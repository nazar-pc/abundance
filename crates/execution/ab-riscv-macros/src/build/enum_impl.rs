mod forbidden_checker;
mod ignored_variants_remover;

use crate::build::enum_impl::forbidden_checker::block_contains_forbidden_syntax;
use crate::build::enum_impl::ignored_variants_remover::remove_ignored_variants;
use crate::build::state::{PendingEnumDisplayImpl, PendingEnumImpl, State};
use ab_riscv_macros_common::code_utils::{post_process_rust_code, pre_process_rust_code};
use anyhow::Context;
use prettyplease::unparse;
use quote::{ToTokens, format_ident, quote};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::{env, fs, iter, mem};
use syn::{
    Expr, Fields, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, Pat, Stmt, Type, parse_file,
    parse_quote, parse_str, parse2,
};

const ENUM_IMPL_ENV_VAR_SUFFIX: &str = "__INSTRUCTION_ENUM_IMPL_PATH";

pub(super) fn enum_name_from_impl(item_impl: &ItemImpl) -> Ident {
    let Type::Path(path) = item_impl.self_ty.as_ref() else {
        panic!(
            "Expected `impl` for `{}`, `#[instruction]` attribute must be added to a simple \
            instruction enum implementation",
            item_impl.self_ty.to_token_stream()
        );
    };
    path.path
        .segments
        .last()
        .expect("Path is never empty; qed")
        .ident
        .clone()
}

pub(super) fn collect_enum_impls_from_dependencies()
-> impl Iterator<Item = anyhow::Result<(ItemImpl, Rc<Path>)>> {
    // Collect exported instruction enums from dependencies
    env::vars().filter_map(|(key, value)| {
        if !key.ends_with(ENUM_IMPL_ENV_VAR_SUFFIX) {
            return None;
        }

        let result = try {
            let mut item_enum_contents = fs::read_to_string(&value).with_context(|| {
                format!(
                    "Failed to read Rust file `{value}` that is expected to contain instruction \
                    enum implementation"
                )
            })?;
            pre_process_rust_code(&mut item_enum_contents);
            let item_impl = parse_str::<ItemImpl>(&item_enum_contents).with_context(|| {
                format!(
                    "Failed to parse Rust file `{value}` that is expected to contain instruction \
                    enum implementation"
                )
            })?;

            (item_impl, Rc::from(Path::new(&value)))
        };

        Some(result)
    })
}

fn extract_try_decode_block_from_impl(item_impl: &ItemImpl) -> Option<&ImplItemFn> {
    for item in &item_impl.items {
        if let ImplItem::Fn(impl_item_fn) = item
            && impl_item_fn.sig.ident == "try_decode"
        {
            return Some(impl_item_fn);
        }
    }

    None
}

fn extract_try_decode_fn_from_impl_mut(item_impl: &mut ItemImpl) -> Option<&mut ImplItemFn> {
    for item in &mut item_impl.items {
        if let ImplItem::Fn(impl_item_fn) = item
            && impl_item_fn.sig.ident == "try_decode"
        {
            return Some(impl_item_fn);
        }
    }

    None
}

fn output_processed_enum_decoding_impl(
    enum_name: Ident,
    item_impl: ItemImpl,
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    let enum_file_path = out_dir.join(format!("{}_decoding_impl.rs", enum_name));
    let code = item_impl.to_token_stream().to_string();
    // Format
    let mut code = unparse(&parse_file(&code).expect("Generated code is valid; qed"));
    // Normalize source
    let item_impl = parse_str(&code).expect("Generated code is valid; qed");
    post_process_rust_code(&mut code);

    // Avoid extra file truncation/override if it didn't change
    if fs::read_to_string(&enum_file_path).ok().as_ref() != Some(&code) {
        fs::write(&enum_file_path, code).with_context(|| {
            format!(
                "Failed to write generated Rust file with instruction decoding implementation for \
                `{enum_name}`"
            )
        })?;
    }
    println!(
        "cargo::metadata={}{ENUM_IMPL_ENV_VAR_SUFFIX}={}",
        enum_name,
        enum_file_path.display()
    );

    state.insert_known_enum_impl(item_impl, Rc::from(enum_file_path))
}

fn output_processed_enum_display_impl(
    enum_name: Ident,
    item_impl: ItemImpl,
    out_dir: &Path,
) -> anyhow::Result<()> {
    let enum_file_path = out_dir.join(format!("{}_display_impl.rs", enum_name));
    let code = item_impl.to_token_stream().to_string();
    // Format
    let mut code = unparse(&parse_file(&code).expect("Generated code is valid; qed"));
    post_process_rust_code(&mut code);

    // Avoid extra file truncation/override if it didn't change
    if fs::read_to_string(&enum_file_path).ok().as_ref() != Some(&code) {
        fs::write(&enum_file_path, code).with_context(|| {
            format!(
                "Failed to write generated Rust file with instruction display implementation for \
                `{enum_name}`"
            )
        })?;
    }

    Ok(())
}

pub(super) fn process_enum_impl(
    mut item_impl: ItemImpl,
    out_dir: &Path,
    state: &mut State,
) -> Option<anyhow::Result<()>> {
    let attribute_index = item_impl
        .attrs
        .iter()
        .enumerate()
        .find_map(|(index, attr)| attr.meta.path().is_ident("instruction").then_some(index))?;
    item_impl.attrs.remove(attribute_index);

    let Some((_, trait_path, _)) = &item_impl.trait_ else {
        return Some(Err(anyhow::anyhow!(
            "Expected `#[instruction] impl Instruction for {0}` or \
            `#[instruction] impl Display for {0}`, but no trait was not found",
            item_impl.self_ty.to_token_stream()
        )));
    };

    let last_trait_segment_path = trait_path
        .segments
        .last()
        .expect("Path is never empty; qed");

    Some(if last_trait_segment_path.ident == "Instruction" {
        process_enum_decoding_impl(item_impl, out_dir, state)
    } else if last_trait_segment_path.ident == "Display" {
        process_enum_display_impl(item_impl, out_dir, state)
    } else {
        Err(anyhow::anyhow!(
            "Expected `impl` for `{}`, `#[instruction]` attribute must be added to a trait \
            implementation, but trait `{}` is not supported",
            item_impl.self_ty.to_token_stream(),
            last_trait_segment_path.ident
        ))
    })
}

pub(super) fn process_enum_decoding_impl(
    mut item_impl: ItemImpl,
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    let enum_name = enum_name_from_impl(&item_impl);

    let Some(try_decode_fn) = extract_try_decode_fn_from_impl_mut(&mut item_impl) else {
        Err(anyhow::anyhow!(
            "Expected `#[instruction] impl Instruction for {}`, but no `try_decode` method was \
            not found",
            item_impl.self_ty.to_token_stream()
        ))?
    };
    let try_decode_block = &mut try_decode_fn.block;
    (!block_contains_forbidden_syntax(try_decode_block, &enum_name)).ok_or_else(|| {
        anyhow::anyhow!(
            "Expected `#[instruction] impl Instruction for {}` must not have `return` or enum \
            construction other than through `Self::` in `try_decode` method",
            enum_name
        )
    })?;

    let Some(enum_definition) = state.get_known_enum_definition(&enum_name) else {
        state.add_pending_enum_impl(PendingEnumImpl { item_impl });
        return Ok(());
    };

    // TODO: This can probably be refactored as an iterator without collecting into a vector first
    let mut all_blocks = Vec::new();
    {
        let mut all_dependencies = HashSet::new();
        all_dependencies.insert(enum_name.clone());
        let mut new_dependencies = enum_definition.dependencies.clone();

        while !new_dependencies.is_empty() {
            for dependency_enum_name in mem::take(&mut new_dependencies) {
                let Some(dependency_enum_definition) =
                    state.get_known_enum_definition(&dependency_enum_name)
                else {
                    state.add_pending_enum_impl(PendingEnumImpl { item_impl });
                    return Ok(());
                };

                if !all_dependencies.insert(dependency_enum_name.clone()) {
                    continue;
                }

                let Some(dependency_enum_impl) = state.get_known_enum_impl(&dependency_enum_name)
                else {
                    state.add_pending_enum_impl(PendingEnumImpl { item_impl });
                    return Ok(());
                };

                let block = &extract_try_decode_block_from_impl(&dependency_enum_impl.item_impl)
                    .expect("Dependencies are all valid; qed")
                    .block;

                all_blocks.push(block);
                new_dependencies.extend(dependency_enum_definition.dependencies.iter().cloned());
            }
        }
    }

    let allowed_instruction = enum_definition
        .instructions
        .iter()
        .map(|instruction| instruction.ident.clone())
        .collect::<HashSet<_>>();
    // TODO: This simply concatenates individual decoding blocks, but it'd be much nicer to combine
    //  multiple `match` statements into one, merging branches with the same opcode. This,
    //  unfortunately, is much more complex, so skipped in the initial implementation.
    let all_blocks = all_blocks
        .into_iter()
        .chain(iter::once(&*try_decode_block))
        .cloned()
        .map(|mut block| {
            remove_ignored_variants(&mut block, &allowed_instruction);
            block
        });

    *try_decode_block = parse2(quote! {{
        #[expect(clippy::allow_attributes, reason = "Attribute below")]
        #[allow(
            clippy::if_same_then_else,
            reason = "In presence of ignored instructions, simple replacement sometimes results in \
            redundant code like `Some(None?)`"
        )]
        #( if let Some(decoded) = try { #all_blocks? } { Some(decoded) } else )*

        { None }
    }})
    .expect("Generated code is valid; qed");

    item_impl
        .attrs
        .push(parse_quote! { #[automatically_derived] });

    output_processed_enum_decoding_impl(enum_name, item_impl, out_dir, state)
}

pub(super) fn process_enum_display_impl(
    mut item_impl: ItemImpl,
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    let enum_name = enum_name_from_impl(&item_impl);

    let Some(enum_definition) = state.get_known_enum_definition(&enum_name) else {
        state.add_pending_enum_display_impl(PendingEnumDisplayImpl { item_impl });
        return Ok(());
    };

    let mut variants_from_dependencies = HashMap::new();
    {
        let mut new_dependencies = enum_definition.dependencies.clone();

        while !new_dependencies.is_empty() {
            for dependency_enum_name in mem::take(&mut new_dependencies) {
                let Some(dependency_enum_definition) =
                    state.get_known_enum_definition(&dependency_enum_name)
                else {
                    state.add_pending_enum_display_impl(PendingEnumDisplayImpl { item_impl });
                    return Ok(());
                };

                let dependency_enum_name = Rc::new(dependency_enum_name);
                for instruction in &dependency_enum_definition.instructions {
                    variants_from_dependencies
                        .insert(Rc::clone(instruction), Rc::clone(&dependency_enum_name));
                }

                new_dependencies.extend(dependency_enum_definition.dependencies.iter().cloned());
            }
        }
    }

    let (formatter_arg, expr_match) = if item_impl.items.len() == 1
        && let Some(ImplItem::Fn(impl_item_fn)) = item_impl.items.first_mut()
        && impl_item_fn.sig.ident == "fmt"
        && let Some(FnArg::Typed(formatter_arg_pat_type)) = impl_item_fn.sig.inputs.last()
        && let Pat::Ident(formatter_arg) = formatter_arg_pat_type.pat.as_ref()
        && impl_item_fn.block.stmts.len() == 1
        && let Some(Stmt::Expr(Expr::Match(expr_match), None)) =
            impl_item_fn.block.stmts.first_mut()
    {
        (&formatter_arg.ident, expr_match)
    } else {
        return Err(anyhow::anyhow!(
            "Expected `#[instruction] impl Display for {}` to contain a single `match` statement \
            in `fmt` method, but found: {}",
            item_impl.self_ty.to_token_stream(),
            item_impl.to_token_stream(),
        ));
    };

    let allowed_instruction = enum_definition
        .instructions
        .iter()
        .map(|instruction| instruction.ident.clone())
        .collect::<HashSet<_>>();

    expr_match.arms.retain(|arm| {
        let path = match &arm.pat {
            Pat::Struct(pat_struct) => &pat_struct.path,
            Pat::TupleStruct(pat_tuple_struct) => &pat_tuple_struct.path,
            Pat::Path(expr_path) => &expr_path.path,
            _ => {
                return false;
            }
        };

        path.segments
            .last()
            .map_or_default(|path_segment| allowed_instruction.contains(&path_segment.ident))
    });

    // The order of variants is not identical to the enum definition for simplicity. It should not
    // be performance-sensitive to justify the complexity.
    expr_match
        .arms
        .extend(
            variants_from_dependencies
                .into_iter()
                .filter_map(|(variant, source_enum)| {
                    let variant_name = &variant.ident;
                    if !allowed_instruction.contains(variant_name) {
                        return None;
                    }

                    Some(match &variant.fields {
                        Fields::Named(fields_named) => {
                            let field_names = fields_named
                                .named
                                .iter()
                                .map(|field| &field.ident)
                                .collect::<Vec<_>>();

                            parse_quote! {
                                Self::#variant_name {
                                    #( #field_names, )*
                                } => ::core::fmt::Display::fmt(
                                    &#source_enum::<Reg>::#variant_name {
                                        #( #field_names: *#field_names, )*
                                    },
                                    #formatter_arg,
                                )
                            }
                        }
                        Fields::Unnamed(fields_unnamed) => {
                            let fields = (0..fields_unnamed.unnamed.len())
                                .map(|index| format_ident!("field_{}", index))
                                .collect::<Vec<_>>();

                            parse_quote! {
                                Self::#variant_name(
                                    #( #fields, )*
                                ) => ::core::fmt::Display::fmt(
                                    &#source_enum::<Reg>::#variant_name(
                                        #( *#fields, )*
                                    ),
                                    #formatter_arg,
                                )
                            }
                        }
                        Fields::Unit => parse_quote! {
                            Self::#variant_name => ::core::fmt::Display::fmt(
                                &#source_enum::<Reg>::#variant_name,
                                #formatter_arg,
                            )
                        },
                    })
                }),
        );

    output_processed_enum_display_impl(enum_name, item_impl, out_dir)
}

/// Process remaining enums that were waiting for dependencies
pub(super) fn process_pending_enum_impls(out_dir: &Path, state: &mut State) -> anyhow::Result<()> {
    {
        let mut last_pending_enums_count = 0;
        loop {
            let pending_enums = state.take_pending_enum_impls();

            if pending_enums.is_empty() {
                break;
            }

            if pending_enums.len() == last_pending_enums_count {
                return Err(anyhow::anyhow!(
                    "Failed to process instruction macros, circular dependency detected, \
                    pending_enums: {:?}",
                    pending_enums
                        .iter()
                        .map(|pending_enum| enum_name_from_impl(&pending_enum.item_impl))
                        .collect::<Vec<_>>()
                ));
            }
            last_pending_enums_count = pending_enums.len();

            for PendingEnumImpl { item_impl } in pending_enums {
                process_enum_decoding_impl(item_impl, out_dir, state)?;
            }
        }
    }
    {
        let mut last_pending_enums_count = 0;
        loop {
            let pending_enums = state.take_pending_enum_display_impls();

            if pending_enums.is_empty() {
                break;
            }

            if pending_enums.len() == last_pending_enums_count {
                return Err(anyhow::anyhow!(
                    "Failed to process instruction macros, circular dependency detected, \
                    pending_enums: {:?}",
                    pending_enums
                        .iter()
                        .map(|pending_enum| enum_name_from_impl(&pending_enum.item_impl))
                        .collect::<Vec<_>>()
                ));
            }
            last_pending_enums_count = pending_enums.len();

            for PendingEnumDisplayImpl { item_impl } in pending_enums {
                process_enum_display_impl(item_impl, out_dir, state)?;
            }
        }
    }

    Ok(())
}
