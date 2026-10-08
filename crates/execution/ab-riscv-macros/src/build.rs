mod enum_definition;
mod enum_impl;
mod execution_impl;
mod shared;
mod state;
#[cfg(test)]
mod tests;

use crate::build::enum_definition::{
    collect_enum_definitions_from_dependencies, process_enum_definition,
    process_pending_enum_definitions,
};
use crate::build::enum_impl::{
    collect_original_enum_decoding_impls_from_dependencies, process_enum_impl,
    process_pending_enum_impls,
};
use crate::build::execution_impl::{
    collect_enum_csr_impls_from_dependencies,
    collect_original_enum_execution_impls_from_dependencies, process_execution_impl,
    process_pending_enum_execution_impls,
};
use crate::build::shared::{
    INSTRUCTION_ATTRIBUTE, INSTRUCTION_EXECUTION_ATTRIBUTE, is_named_attribute,
};
use crate::build::state::State;
use ab_riscv_macros_common::code_utils::pre_process_rust_code;
use anyhow::Context;
use quote::ToTokens;
use std::path::{Path, PathBuf};
use std::{env, fs, io, iter};
use syn::visit::{Visit, visit_item_enum, visit_item_impl};
use syn::{ItemEnum, ItemImpl};

/// Processes all instruction macros in the crate when called from `build.rs`.
///
/// Items annotated with `#[instruction]` or `#[instruction_execution]` are found anywhere in the
/// crate's Rust files, including inline modules and function bodies, and attributes may be
/// path-qualified like `#[ab_riscv_macros::instruction]`. Attributes are recognized by the last
/// segment of their path in the source code, there is no name resolution.
///
/// # Limitations
///
/// The following usages are skipped silently, and the corresponding macro then fails to include a
/// file that was never generated:
/// * renamed imports of the macros, like `#[foo]` after `use ab_riscv_macros::instruction as foo;`
/// * items inside macro invocations (like `macro_rules!` or a function-like macro call), whose
///   contents are not parsed
/// * attributes produced by other macros (like `cfg_attr`)
/// * files where no such attribute is written at the start of a line (indentation is fine)
pub fn process_instruction_macros() -> anyhow::Result<()> {
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR").context(
        "Failed to retrieve `CARGO_MANIFEST_DIR` environment variable, make sure to call \
        `process_instruction_macros` from `build.rs`",
    )?;
    let out_dir = env::var_os("OUT_DIR").context(
        "Failed to retrieve `OUT_DIR` environment variable, make sure to call \
        `process_instruction_macros` from `build.rs`",
    )?;
    let out_dir = Path::new(&out_dir);

    let mut state = State::new();

    for maybe_enum_definition in collect_enum_definitions_from_dependencies() {
        let (
            original_item_enum,
            item_enum,
            ignored_instructions,
            direct_dependencies,
            dependencies_for_enablement,
            source,
        ) = maybe_enum_definition?;

        state.insert_known_enum_definition(
            original_item_enum,
            item_enum,
            ignored_instructions,
            direct_dependencies,
            dependencies_for_enablement,
            source,
        )?;
    }
    for maybe_enum_impl in collect_original_enum_decoding_impls_from_dependencies() {
        let (item_impl, source) = maybe_enum_impl?;
        state.insert_known_original_enum_decoding_impl(item_impl, source)?;
    }
    for maybe_enum_csr_impl in collect_enum_csr_impls_from_dependencies() {
        let (item_impl, source) = maybe_enum_csr_impl?;
        state.insert_known_enum_csr_impl(item_impl, source)?;
    }
    for maybe_enum_execution_impl in collect_original_enum_execution_impls_from_dependencies() {
        let (item_impl, source) = maybe_enum_execution_impl?;
        state.insert_known_original_enum_execution_impl(item_impl, source)?;
    }

    for maybe_rust_file in rust_files_in(PathBuf::from(&manifest_dir)) {
        let rust_file = maybe_rust_file.context("Failed to collect Rust files")?;
        process_rust_file(&rust_file, out_dir, &mut state)
            .with_context(|| format!("Failed to process Rust file `{}`", rust_file.display()))?;
    }

    process_pending_enum_definitions(out_dir, &mut state)?;
    process_pending_enum_impls(out_dir, &mut state)?;
    process_pending_enum_execution_impls(out_dir, &mut state)
}

fn rust_files_in(dir: PathBuf) -> Box<dyn Iterator<Item = io::Result<PathBuf>>> {
    fn walk(dir: PathBuf) -> Box<dyn Iterator<Item = io::Result<PathBuf>>> {
        let read_dir = match fs::read_dir(dir) {
            Ok(iter) => iter,
            Err(error) => {
                return Box::new(iter::once(Err(error))) as Box<_>;
            }
        };

        Box::new(read_dir.flat_map(move |entry_res| {
            let entry = match entry_res {
                Ok(entry) => entry,
                Err(error) => {
                    return Box::new(iter::once(Err(error))) as Box<_>;
                }
            };

            let path = entry.path();

            if path.is_dir() {
                walk(path)
            } else if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == "rs")
            {
                Box::new(iter::once(Ok(path))) as Box<_>
            } else {
                Box::new(iter::empty::<io::Result<PathBuf>>()) as Box<_>
            }
        }))
    }

    walk(dir)
}

/// Whether a Rust file may contain `#[instruction]` or `#[instruction_execution]` attributes.
///
/// This helps to quickly skip files that may use Rust nightly syntax not supported by `syn`, which
/// is limited to stable Rust. Any line that starts with such an attribute (path-qualified or not,
/// regardless of indentation) is a match.
fn may_contain_instruction_macros(file_contents: &str) -> bool {
    file_contents.lines().any(|line| {
        let Some(attribute) = line.trim_start().strip_prefix("#[") else {
            return false;
        };
        // Arguments may be delimited by any kind of brackets, and the path may contain whitespace
        let path = attribute
            .split_once(|c: char| {
                !(c.is_alphanumeric() || c.is_whitespace() || matches!(c, '_' | ':'))
            })
            .map_or(attribute, |(path, _)| path);
        let name = path.rsplit_once("::").map_or(path, |(_, name)| name);

        matches!(
            name.trim(),
            INSTRUCTION_ATTRIBUTE | INSTRUCTION_EXECUTION_ATTRIBUTE
        )
    })
}

/// An item annotated with `#[instruction]` or `#[instruction_execution]`
enum InstructionItem {
    Enum(ItemEnum),
    Impl(ItemImpl),
}

/// Collects enums and impls annotated with `#[instruction]` or `#[instruction_execution]`
/// anywhere in a file, including inline modules and function bodies
#[derive(Default)]
struct InstructionItemsCollector {
    items: Vec<InstructionItem>,
}

impl Visit<'_> for InstructionItemsCollector {
    fn visit_item_enum(&mut self, i: &ItemEnum) {
        if i.attrs
            .iter()
            .any(|attribute| is_named_attribute(attribute, INSTRUCTION_ATTRIBUTE))
        {
            self.items.push(InstructionItem::Enum(i.clone()));
        }

        visit_item_enum(self, i);
    }

    fn visit_item_impl(&mut self, i: &ItemImpl) {
        if i.attrs.iter().any(|attribute| {
            is_named_attribute(attribute, INSTRUCTION_ATTRIBUTE)
                || is_named_attribute(attribute, INSTRUCTION_EXECUTION_ATTRIBUTE)
        }) {
            self.items.push(InstructionItem::Impl(i.clone()));
        }

        visit_item_impl(self, i);
    }
}

fn process_rust_file(source: &Path, out_dir: &Path, state: &mut State) -> anyhow::Result<()> {
    let mut file_contents = fs::read_to_string(source).context("Failed to read Rust file")?;
    if !may_contain_instruction_macros(&file_contents) {
        return Ok(());
    }

    pre_process_rust_code(&mut file_contents);

    let file = syn::parse_file(&file_contents).context("Failed to parse Rust file")?;

    let mut collector = InstructionItemsCollector::default();
    collector.visit_file(&file);

    for item in collector.items {
        match item {
            InstructionItem::Enum(item_enum) => {
                let enum_name = item_enum.ident.clone();
                process_enum_definition(item_enum, out_dir, state).with_context(|| {
                    format!(
                        "Failed to process enum `{enum_name}` in file `{}`",
                        source.display()
                    )
                })?;
            }
            InstructionItem::Impl(item_impl) => {
                let trait_name = item_impl.trait_.as_ref().map(|(path, _)| {
                    path.segments
                        .last()
                        .expect("Path is never empty; qed")
                        .ident
                        .clone()
                });
                let type_name = item_impl.self_ty.clone();
                if let Some(result) = process_enum_impl(item_impl.clone(), out_dir, state) {
                    result.with_context(|| {
                        format!(
                            "Failed to process impl block (`{:?}` for `{}`) in file `{}`",
                            trait_name.to_token_stream(),
                            type_name.to_token_stream(),
                            source.display()
                        )
                    })?;
                } else if let Some(result) = process_execution_impl(item_impl, out_dir, state) {
                    result.with_context(|| {
                        format!(
                            "Failed to process impl block (`{:?}` for `{}`) in file `{}`",
                            trait_name.to_token_stream(),
                            type_name.to_token_stream(),
                            source.display()
                        )
                    })?;
                }
            }
        }
    }

    Ok(())
}
