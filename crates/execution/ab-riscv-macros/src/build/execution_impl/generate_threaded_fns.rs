use crate::build::execution_impl::generate_variant_fns::variant_fn_name;
use crate::build::shared::strip_const_where_predicates;
use anyhow::Context;
use heck::ToSnakeCase;
use quote::{ToTokens, format_ident, quote};
use std::env;
use std::rc::Rc;
use syn::{Abi, Arm, Generics, Ident, Item, Pat, Type, Variant, WhereClause, parse_quote};

/// Calling convention used for functions involved in threaded execution.
///
/// Windows x86-64 passes only four arguments in registers, where the System V convention every
/// other x86-64 target uses passes six, and it is critical for performance that a threaded
/// interpreter has six registers available for arguments. `sysv64` ABI is supported on Windows too
/// and solves this particular issue.
fn handler_abi() -> anyhow::Result<Option<Abi>> {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").context(
        "Failed to retrieve `CARGO_CFG_TARGET_ARCH` environment variable, make sure to call \
        `process_instruction_macros` from `build.rs`",
    )?;
    let target_os = env::var("CARGO_CFG_TARGET_OS").context(
        "Failed to retrieve `CARGO_CFG_TARGET_OS` environment variable, make sure to call \
        `process_instruction_macros` from `build.rs`",
    )?;

    Ok((target_arch == "x86_64" && target_os == "windows")
        .then(|| parse_quote! { extern "sysv64" }))
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
    let abi = handler_abi()?;
    let dispatch_fn_name = format_ident!("dispatch_{}", enum_name.to_string().to_snake_case());
    let dispatch_result_name = format_ident!("{enum_name}ThreadedDispatchResult");
    let result_ty: Type = parse_quote! {
        ThreadedExecutionResult<PC, #self_ty>
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
            #[cfg_attr(
                all(target_arch = "x86_64", target_os = "windows"),
                expect(
                    improper_ctypes_definitions,
                    reason = "Handlers only ever call each other, within this crate"
                )
            )]
            #abi fn #handler_fn_name<#generic_params>(
                instruction: #self_ty,
                mut instruction_fetcher: PC,
                regs: &mut Regs,
                mut ext_state: ExtState,
                memory: &mut Memory,
                mut system_instruction_handler: InstructionHandler,
            ) -> #result_ty
                #where_clause
            {
                let Rs1Rs2Operands { rs1, rs2 } = instruction.get_rs1_rs2_operands();
                let rs1_value = regs.read(rs1);
                let rs2_value = regs.read(rs2);

                #[expect(
                    clippy::undocumented_unsafe_blocks,
                    reason = "Comments will be stripped, this will suppress some of the lints that \
                    are caused by it"
                )]
                let #enum_name::#variant_ident { #pat_fields } = instruction else {
                    // SAFETY: A handler is only ever reached through the dispatch arm for its own
                    // variant
                    unsafe {
                        ::core::hint::unreachable_unchecked();
                    }
                };

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
                        return ThreadedExecutionResult::stopped(
                            instruction_fetcher,
                        );
                    }
                    ExecutionResult::Err(error) => {
                        ::core::hint::cold_path();
                        return ThreadedExecutionResult::failed(
                            instruction_fetcher,
                            error,
                        );
                    }
                };

                match control_flow {
                    Ok(::core::ops::ControlFlow::Continue(())) => {}
                    Ok(::core::ops::ControlFlow::Break(())) => {
                        ::core::hint::cold_path();
                        return ThreadedExecutionResult::stopped(
                            instruction_fetcher,
                        );
                    }
                    Err(error) => {
                        ::core::hint::cold_path();
                        return ThreadedExecutionResult::failed(
                            instruction_fetcher,
                            error,
                        );
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
                        return ThreadedExecutionResult::stopped(
                            instruction_fetcher,
                        );
                    }
                    #dispatch_result_name::Err(error) => {
                        ::core::hint::cold_path();
                        return ThreadedExecutionResult::failed(
                            instruction_fetcher,
                            error,
                        );
                    }
                };

                become handler(
                    instruction,
                    instruction_fetcher,
                    regs,
                    ext_state,
                    memory,
                    system_instruction_handler,
                )
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
        #[cfg_attr(
            all(target_arch = "x86_64", target_os = "windows"),
            expect(
                improper_ctypes_definitions,
                reason = "Handlers only ever call each other, within this crate"
            )
        )]
        #[inline(always)]
        fn #dispatch_fn_name<#generic_params>(
            instruction_fetcher: &mut PC,
            memory: &Memory,
        ) -> #dispatch_result_name<
            #self_ty,
            #abi fn(
                #self_ty,
                PC,
                &mut Regs,
                ExtState,
                &mut Memory,
                InstructionHandler,
            ) -> #result_ty,
        >
            #where_clause
        {
            let instruction = loop {
                match instruction_fetcher.fetch_instruction(memory) {
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
            #[inline(always)]
            fn execute_threaded(
                mut instruction_fetcher: PC,
                regs: &mut Regs,
                ext_state: ExtState,
                memory: &mut Memory,
                system_instruction_handler: InstructionHandler,
            ) -> #result_ty {
                // This is the one ordinary call in the whole chain: everything from here on is
                // reached with `become` and nothing returns until execution stops
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
                            instruction_fetcher,
                        );
                    }
                    #dispatch_result_name::Err(error) => {
                        ::core::hint::cold_path();
                        return ThreadedExecutionResult::failed(
                            instruction_fetcher,
                            error,
                        );
                    }
                };

                handler(
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

    Ok(generated_items)
}
