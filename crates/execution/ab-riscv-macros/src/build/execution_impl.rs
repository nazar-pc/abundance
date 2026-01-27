mod extract_matches;

use crate::build::enum_impl::enum_name_from_impl;
use crate::build::execution_impl::extract_matches::extract_variant_arms;
use crate::build::state::{PendingEnumExecutionImpl, State};
use ab_riscv_macros_common::code_utils::{post_process_rust_code, pre_process_rust_code};
use anyhow::Context;
use prettyplease::unparse;
use quote::{ToTokens, quote};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::{env, fs, iter, mem};
use syn::{Ident, ImplItem, ImplItemFn, ItemImpl, parse_file, parse_quote, parse_str, parse2};

const ENUM_EXECUTION_IMPL_ENV_VAR_SUFFIX: &str = "__INSTRUCTION_ENUM_EXECUTION_IMPL_PATH";

pub(super) fn collect_enum_execution_impls_from_dependencies()
-> impl Iterator<Item = anyhow::Result<(ItemImpl, Rc<Path>)>> {
    // Collect exported instruction enums from dependencies
    env::vars().filter_map(|(key, value)| {
        if !key.ends_with(ENUM_EXECUTION_IMPL_ENV_VAR_SUFFIX) {
            return None;
        }

        let result = try {
            let mut item_enum_contents = fs::read_to_string(&value).with_context(|| {
                format!(
                    "Failed to read Rust file `{value}` that is expected to contain instruction \
                    enum execution implementation"
                )
            })?;
            pre_process_rust_code(&mut item_enum_contents);
            let item_impl = parse_str::<ItemImpl>(&item_enum_contents).with_context(|| {
                format!(
                    "Failed to parse Rust file `{value}` that is expected to contain instruction \
                    enum execution implementation"
                )
            })?;

            (item_impl, Rc::from(Path::new(&value)))
        };

        Some(result)
    })
}

fn extract_execute_block_from_impl(item_impl: &ItemImpl) -> Option<&ImplItemFn> {
    for item in &item_impl.items {
        if let ImplItem::Fn(impl_item_fn) = item
            && impl_item_fn.sig.ident == "execute"
        {
            return Some(impl_item_fn);
        }
    }

    None
}

fn extract_execute_fn_from_impl_mut(item_impl: &mut ItemImpl) -> Option<&mut ImplItemFn> {
    for item in &mut item_impl.items {
        if let ImplItem::Fn(impl_item_fn) = item
            && impl_item_fn.sig.ident == "execute"
        {
            return Some(impl_item_fn);
        }
    }

    None
}

fn output_processed_enum_execution_impl(
    enum_name: Ident,
    item_impl: ItemImpl,
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    let enum_file_path = out_dir.join(format!("{}_execution_impl.rs", enum_name));
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
                "Failed to write generated Rust file with instruction execution implementation for \
                `{enum_name}`",
            )
        })?;
    }
    println!(
        "cargo::metadata={}{ENUM_EXECUTION_IMPL_ENV_VAR_SUFFIX}={}",
        enum_name,
        enum_file_path.display()
    );

    state.insert_known_enum_execution_impl(item_impl, Rc::from(enum_file_path))
}

pub(super) fn process_execution_impl(
    mut item_impl: ItemImpl,
    out_dir: &Path,
    state: &mut State,
) -> Option<anyhow::Result<()>> {
    let attribute_index = item_impl
        .attrs
        .iter()
        .enumerate()
        .find_map(|(index, attr)| {
            attr.meta
                .path()
                .is_ident("instruction_execution")
                .then_some(index)
        })?;

    let result = try {
        let enum_name = enum_name_from_impl(&item_impl);

        let Some(execute_fn) = extract_execute_fn_from_impl_mut(&mut item_impl) else {
            Err(anyhow::anyhow!(
                "Expected `impl` for `{}`, `#[instruction_execution]` attribute must be added to a \
                simple instruction enum implementation, but no `execute` method was not found",
                item_impl.self_ty.to_token_stream()
            ))?
        };
        let execute_block = &mut execute_fn.block;

        let Some(enum_definition) = state.get_known_enum_definition(&enum_name) else {
            state.add_pending_enum_execution_impl(PendingEnumExecutionImpl { item_impl });
            return Some(Ok(()));
        };

        // TODO: This can probably be refactored as an iterator without collecting into a vector
        //  first
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
                        state.add_pending_enum_execution_impl(PendingEnumExecutionImpl {
                            item_impl,
                        });
                        return Some(Ok(()));
                    };

                    if !all_dependencies.insert(dependency_enum_name.clone()) {
                        continue;
                    }

                    let Some(dependency_enum_execution_impl) =
                        state.get_known_enum_execution_impl(&dependency_enum_name)
                    else {
                        state.add_pending_enum_execution_impl(PendingEnumExecutionImpl {
                            item_impl,
                        });
                        return Some(Ok(()));
                    };

                    let block =
                        &extract_execute_block_from_impl(&dependency_enum_execution_impl.item_impl)
                            .expect("Dependencies are all valid; qed")
                            .block;

                    all_blocks.push(block);
                    new_dependencies
                        .extend(dependency_enum_definition.dependencies.iter().cloned());
                }
            }
        }

        let all_blocks = all_blocks.into_iter().chain(iter::once(&*execute_block));

        let mut all_instructions = HashMap::new();
        for block in all_blocks {
            for maybe_instruction in extract_variant_arms(block)? {
                let (variant_name, instruction_arm) = maybe_instruction?;
                all_instructions.insert(variant_name, instruction_arm);
            }
        }

        let all_instructions = enum_definition
            .instructions
            .iter()
            .map(|variant| {
                all_instructions.remove(&variant.ident).with_context(|| {
                    format!(
                        "Instruction `{}` not found in all_instructions for enum `{enum_name}`",
                        variant.ident
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        *execute_block = parse2(quote! {{
            match self {
                #( #all_instructions )*
            }
        }})
        .expect("Generated code is valid; qed");

        // Only remove after successful processing, so that the function can be called repeatedly
        // with the same input if the implementation is still pending
        item_impl.attrs.remove(attribute_index);
        // Comments will be stripped, this will suppress some of the lints that are caused by it
        item_impl.attrs.extend([
            parse_quote! { #[expect(clippy::allow_attributes, reason = "clippy::undocumented_unsafe_blocks")]},
            parse_quote! { #[allow(clippy::undocumented_unsafe_blocks)]},
        ]);

        output_processed_enum_execution_impl(enum_name, item_impl, out_dir, state)?
    };

    Some(result)
}

/// Process remaining enums that were waiting for dependencies
pub(super) fn process_pending_enum_execution_impls(
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    let mut last_pending_enums_count = 0;
    loop {
        let pending_enums = state.take_pending_enum_execution_impls();

        if pending_enums.is_empty() {
            break;
        }

        if pending_enums.len() == last_pending_enums_count {
            return Err(anyhow::anyhow!(
                "Failed to process instruction execution macros, circular dependency detected, \
                pending_enums: {:?}",
                pending_enums
                    .iter()
                    .map(|pending_enum| enum_name_from_impl(&pending_enum.item_impl))
                    .collect::<Vec<_>>()
            ));
        }
        last_pending_enums_count = pending_enums.len();

        for PendingEnumExecutionImpl { item_impl } in pending_enums {
            if let Some(result) = process_execution_impl(item_impl, out_dir, state) {
                result?;
            }
        }
    }

    Ok(())
}
