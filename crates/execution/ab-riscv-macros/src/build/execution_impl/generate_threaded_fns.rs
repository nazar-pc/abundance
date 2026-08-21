use crate::build::execution_impl::generate_variant_fns::variant_fn_name;
use crate::build::shared::strip_const_where_predicates;
use anyhow::Context;
use heck::ToSnakeCase;
use quote::{ToTokens, format_ident, quote};
use std::env;
use std::rc::Rc;
use syn::{
    Abi, Arm, Attribute, Generics, Ident, Item, Pat, Type, Variant, WhereClause, parse_quote,
};

/// Calling convention used for functions involved in threaded execution, and the target features
/// they need.
///
/// The default Rust calling convention returns nothing larger than a pointer pair in registers, so
/// the outcome of execution would come back through memory, costing an argument register in every
/// handler of the chain. An explicitly pinned convention is what makes the register return
/// possible through `OpaqueThreadedExecutionResult`, which internally is composed of:
/// * 256-bit vector register on x86-64 using `sysv64` ABI, which also gives handlers six argument
///   registers on Windows
/// * homogeneous aggregates on Aarch64
///
/// Everywhere else the outcome comes back through memory, and nothing special is needed for that.
fn handler_abi() -> anyhow::Result<(Option<Abi>, Option<Attribute>)> {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").context(
        "Failed to retrieve `CARGO_CFG_TARGET_ARCH` environment variable, make sure to call \
        `process_instruction_macros` from `build.rs`",
    )?;

    // TODO: `extern "Rust"` returning such a value through memory where the platform's own
    //  convention returns it in registers is a bug rather than something inherent, so this
    //  pinning can go away (except on x86-64 Windows) once it is fixed:
    //  https://github.com/rust-lang/rust/issues/161381
    Ok(match target_arch.as_str() {
        "x86_64" => (
            Some(parse_quote! { extern "sysv64" }),
            // Miri emulates a CPU without AVX and refuses to call a function that requires it, so
            // unless it is running code built for AVX in the first place, the feature is disabled
            Some(parse_quote! {
                #[cfg_attr(
                    any(not(miri), target_feature = "avx"),
                    target_feature(enable = "avx")
                )]
            }),
        ),
        "aarch64" => (Some(parse_quote! { extern "C" }), None),
        _ => (None, None),
    })
}

/// Where clause for the generated threaded items.
///
/// Threaded execution is never `const` (dispatch goes through a table of function pointers, which
/// `const fn` does not allow), so `[const]` bounds are relaxed to ordinary ones, exactly like they
/// are for a non-`const` execution implementation, and the fetching half of the program counter is
/// required on top of what `execute()` needs.
fn threaded_where_clause(generics: &Generics, self_ty: &Type) -> anyhow::Result<WhereClause> {
    let Some(mut where_clause) = generics.where_clause.clone() else {
        return Err(anyhow::anyhow!("Missing where clause"));
    };

    strip_const_where_predicates(&mut where_clause.predicates);

    where_clause.predicates.push(parse_quote! {
        PC: InstructionFetcher<#self_ty, Memory>
    });
    // Applying an `ExecutionResult` is the executor's job rather than the instruction's, so the
    // handlers need the register file even for an extension whose own instructions never touch it
    where_clause.predicates.push(parse_quote! {
        Regs: RegisterFile<Reg>
    });

    Ok(where_clause)
}

/// Generates tail-call-threaded execution alongside the `match`-based `execute()`.
///
/// One handler function per instruction variant is generated. A handler destructures its own
/// instruction (so it reads only the operands that instruction names), calls the per-variant
/// execution function that
/// [`generate_variant_fns`](super::generate_variant_fns::generate_variant_fns) already produced,
/// applies the `ExecutionResult` it returns, and tail-calls the next handler, so the chain never
/// returns until execution stops.
pub(super) fn generate_threaded_fns(
    enum_name: &Ident,
    self_ty: &Type,
    generics: &Generics,
    variants: &[Rc<Variant>],
    match_arms: &[Arm],
) -> anyhow::Result<Vec<Item>> {
    let generic_params = &generics.params;
    let where_clause = threaded_where_clause(generics, self_ty)?;
    let (abi, target_feature) = handler_abi()?;
    let enum_snake_case = enum_name.to_string().to_snake_case();
    let dispatch_fn_name = format_ident!("dispatch_{enum_snake_case}");
    let entry_fn_name = format_ident!("execute_{enum_snake_case}_threaded");
    let dispatch_result_name = format_ident!("{enum_name}ThreadedDispatchResult");
    // What handlers return: the outcome serialized into a shape the target returns in registers
    let handler_result_ty: Type = parse_quote! {
        OpaqueThreadedExecutionResult<#self_ty>
    };
    // What execution as a whole returns, once the chain is over
    let result_ty: Type = parse_quote! {
        ThreadedExecutionResult<#self_ty>
    };

    let mut generated_items = Vec::with_capacity(variants.len() + 3);
    let mut dispatch_arms = Vec::with_capacity(variants.len());

    // `FetchInstructionResult` with the handler that executes the fetched instruction attached, and
    // without the `Continue` variant, which dispatch resolves while fetching rather than passing
    // on. It never escapes the dispatch step it is created in, so it never materializes.
    generated_items.push(parse_quote! {
        enum #dispatch_result_name<I, Handler>
        where
            I: Instruction,
        {
            Next { instruction: I, handler: Handler },
            Break,
            Err(ExecutionError<<I::Reg as Register>::Type>),
        }
    });

    for (variant, arm) in variants.iter().zip(match_arms) {
        let variant_ident = &variant.ident;
        let Pat::Struct(pat_struct) = &arm.pat else {
            return Err(anyhow::anyhow!(
                "Expected a struct pattern for instruction variant `{variant_ident}`, found `{}`",
                arm.pat.to_token_stream()
            ));
        };
        let pat_fields = &pat_struct.fields;

        let variant_fn_name = variant_fn_name(enum_name, variant_ident);
        let handler_fn_name = format_ident!("{variant_fn_name}_threaded");

        let variant_call_args = pat_fields
            .iter()
            .filter_map(|field| match field.pat.as_ref() {
                Pat::Ident(pat_ident) => Some(&pat_ident.ident),
                _ => None,
            });

        generated_items.push(parse_quote! {
            #[expect(
                clippy::undocumented_unsafe_blocks,
                reason = "Comments will be stripped, this will suppress some of the lints that \
                are caused by it"
            )]
            #[expect(clippy::allow_attributes, reason = "Attribute below")]
            #[allow(
                improper_ctypes_definitions,
                reason = "Handlers only ever call each other, within this crate"
            )]
            #target_feature
            unsafe #abi fn #handler_fn_name<#generic_params>(
                instruction: #self_ty,
                mut instruction_fetcher: PC,
                regs: &mut Regs,
                mut ext_state: ExtState,
                memory: &mut Memory,
                mut system_instruction_handler: InstructionHandler,
            ) -> #handler_result_ty
                #where_clause
            {
                let Rs1Rs2Operands { rs1, rs2 } = instruction.get_rs1_rs2_operands();
                let rs1_value = regs.read(rs1);
                let rs2_value = regs.read(rs2);

                let #enum_name::#variant_ident { #pat_fields } = instruction else {
                    // SAFETY: A handler is only ever reached through the dispatch arm for its own
                    // variant
                    unsafe {
                        ::core::hint::unreachable_unchecked();
                    }
                };

                // Dispatch leaves the program counter on the instruction it read, and this is what
                // moves it past it - by a size the compiler folds to a constant, since the variant
                // is known here. Doing it during the fetch instead makes the address of the next
                // instruction depend on decoding the current one, which is a load-to-load
                // dependency in the middle of every dispatch step.
                //
                // SAFETY: Dispatch has just peeked this instruction successfully, and this is the
                // only place that moves past it
                unsafe {
                    instruction_fetcher.advance(Instruction::size(&instruction));
                }

                let execution_result = #variant_fn_name::<#generic_params>(
                    #( #variant_call_args, )*
                    rs1_value,
                    rs2_value,
                    regs,
                    &mut ext_state,
                    memory,
                    &mut instruction_fetcher,
                    &mut system_instruction_handler,
                );

                let control_flow = match execution_result {
                    ExecutionResult::Continue { rd, value } => {
                        regs.write(rd, value);
                        Ok(::core::ops::ControlFlow::Continue(()))
                    }
                    ExecutionResult::ContinueNoWrite => {
                        Ok(::core::ops::ControlFlow::Continue(()))
                    }
                    ExecutionResult::Branch { offset } => {
                        instruction_fetcher.set_pc_relative(
                            memory,
                            Instruction::size(&instruction),
                            offset,
                        )
                    }
                    ExecutionResult::Jump { target } => {
                        instruction_fetcher.set_pc(memory, target)
                    }
                    ExecutionResult::Break => {
                        ::core::hint::cold_path();
                        // SAFETY: Platform support is checked before the chain is entered
                        return unsafe {
                            OpaqueThreadedExecutionResult::new(
                                ThreadedExecutionResult::stopped(instruction_fetcher.get_pc()),
                            )
                        };
                    }
                    ExecutionResult::Err(error) => {
                        ::core::hint::cold_path();
                        // SAFETY: Platform support is checked before the chain is entered
                        return unsafe {
                            OpaqueThreadedExecutionResult::new(
                                ThreadedExecutionResult::failed(
                                    instruction_fetcher.get_pc(),
                                    error,
                                ),
                            )
                        };
                    }
                };

                match control_flow {
                    Ok(::core::ops::ControlFlow::Continue(())) => {}
                    Ok(::core::ops::ControlFlow::Break(())) => {
                        ::core::hint::cold_path();
                        // SAFETY: Platform support is checked before the chain is entered
                        return unsafe {
                            OpaqueThreadedExecutionResult::new(
                                ThreadedExecutionResult::stopped(instruction_fetcher.get_pc()),
                            )
                        };
                    }
                    Err(error) => {
                        ::core::hint::cold_path();
                        // SAFETY: Platform support is checked before the chain is entered
                        return unsafe {
                            OpaqueThreadedExecutionResult::new(
                                ThreadedExecutionResult::failed(
                                    instruction_fetcher.get_pc(),
                                    error,
                                ),
                            )
                        };
                    }
                }

                let (instruction, handler) = match #dispatch_fn_name::<#generic_params>(
                    &mut instruction_fetcher,
                    memory,
                ) {
                    #dispatch_result_name::Next {
                        instruction,
                        handler,
                    } => (instruction, handler),
                    #dispatch_result_name::Break => {
                        ::core::hint::cold_path();
                        // SAFETY: Platform support is checked before the chain is entered
                        return unsafe {
                            OpaqueThreadedExecutionResult::new(
                                ThreadedExecutionResult::stopped(instruction_fetcher.get_pc()),
                            )
                        };
                    }
                    #dispatch_result_name::Err(error) => {
                        ::core::hint::cold_path();
                        // SAFETY: Platform support is checked before the chain is entered
                        return unsafe {
                            OpaqueThreadedExecutionResult::new(
                                ThreadedExecutionResult::failed(
                                    instruction_fetcher.get_pc(),
                                    error,
                                ),
                            )
                        };
                    }
                };

                // SAFETY: Every handler carries the same target features this one does, which is
                // what makes them unsafe to call in the first place
                unsafe {
                    become handler(
                        instruction,
                        instruction_fetcher,
                        regs,
                        ext_state,
                        memory,
                        system_instruction_handler,
                    )
                }
            }
        });

        dispatch_arms.push(quote! {
            #enum_name::#variant_ident { .. } => #handler_fn_name::<#generic_params>,
        });
    }

    generated_items.push(parse_quote! {
        #[expect(
            clippy::type_complexity,
            reason = "`become` requires an exact signature match and a type alias cannot capture \
            the enclosing generics, so the handler type is spelled out here"
        )]
        #[expect(clippy::allow_attributes, reason = "Attribute below")]
        #[allow(
            improper_ctypes_definitions,
            reason = "Handlers only ever call each other, within this crate"
        )]
        #[inline(always)]
        fn #dispatch_fn_name<#generic_params>(
            instruction_fetcher: &mut PC,
            memory: &Memory,
        ) -> #dispatch_result_name<
            #self_ty,
            unsafe #abi fn(
                #self_ty,
                PC,
                &mut Regs,
                ExtState,
                &mut Memory,
                InstructionHandler,
            ) -> #handler_result_ty,
        >
            #where_clause
        {
            let instruction = loop {
                match instruction_fetcher.peek_instruction(memory) {
                    FetchInstructionResult::Instruction(instruction) => {
                        break instruction;
                    }
                    FetchInstructionResult::Continue => {
                        ::core::hint::cold_path();
                    }
                    FetchInstructionResult::Break => {
                        ::core::hint::cold_path();
                        return #dispatch_result_name::Break;
                    }
                    FetchInstructionResult::Err(error) => {
                        ::core::hint::cold_path();
                        return #dispatch_result_name::Err(error);
                    }
                }
            };

            #[expect(
                clippy::rest_pattern_accessible_field,
                reason = "Dispatch selects a handler by variant and never looks at any field"
            )]
            let handler = match instruction {
                #( #dispatch_arms )*
            };

            #dispatch_result_name::Next { instruction, handler }
        }
    });

    // The one ordinary call in the whole chain: everything from here on is reached with `become`
    // and nothing returns until execution stops.
    //
    // This is a separate function rather than the body of the trait method because it is the one
    // that calls handlers, so it has to carry the same target features they do, and a safe trait
    // method cannot carry target features.
    generated_items.push(parse_quote! {
        #[expect(
            clippy::undocumented_unsafe_blocks,
            reason = "Comments will be stripped, this will suppress some of the lints that are \
            caused by it"
        )]
        #[inline]
        #target_feature
        unsafe fn #entry_fn_name<#generic_params>(
            mut instruction_fetcher: PC,
            regs: &mut Regs,
            ext_state: ExtState,
            memory: &mut Memory,
            system_instruction_handler: InstructionHandler,
        ) -> #result_ty
            #where_clause
        {
            let (instruction, handler) = match #dispatch_fn_name::<#generic_params>(
                &mut instruction_fetcher,
                memory,
            ) {
                #dispatch_result_name::Next {
                    instruction,
                    handler,
                } => (instruction, handler),
                #dispatch_result_name::Break => {
                    ::core::hint::cold_path();
                    return ThreadedExecutionResult::stopped(
                        instruction_fetcher.get_pc(),
                    );
                }
                #dispatch_result_name::Err(error) => {
                    ::core::hint::cold_path();
                    return ThreadedExecutionResult::failed(
                        instruction_fetcher.get_pc(),
                        error,
                    );
                }
            };

            // SAFETY: This function carries the same target features every handler does, which is
            // what makes them unsafe to call in the first place
            let outcome = unsafe {
                handler(
                    instruction,
                    instruction_fetcher,
                    regs,
                    ext_state,
                    memory,
                    system_instruction_handler,
                )
            };

            outcome.into_result()
        }
    });

    generated_items.push(parse_quote! {
        impl<#generic_params> ThreadedExecutableInstruction<
            Regs,
            ExtState,
            Memory,
            PC,
            InstructionHandler,
        > for #self_ty
            #where_clause
        {
            #[expect(
                clippy::undocumented_unsafe_blocks,
                reason = "Comments will be stripped, this will suppress some of the lints that \
                are caused by it"
            )]
            #[inline(always)]
            fn execute_threaded(
                instruction_fetcher: PC,
                regs: &mut Regs,
                ext_state: ExtState,
                memory: &mut Memory,
                system_instruction_handler: InstructionHandler,
            ) -> #result_ty {
                if !OpaqueThreadedExecutionResult::<#self_ty>::platform_supported() {
                    ::core::hint::cold_path();
                    return ThreadedExecutionResult::failed(
                        instruction_fetcher.get_pc(),
                        ExecutionError::UnsupportedPlatform,
                    );
                }

                // SAFETY: Platform support for what handlers need was just checked
                unsafe {
                    #entry_fn_name::<#generic_params>(
                        instruction_fetcher,
                        regs,
                        ext_state,
                        memory,
                        system_instruction_handler,
                    )
                }
            }
        }
    });

    Ok(generated_items)
}
