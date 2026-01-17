//! Macros for RISC-V primitives

#![feature(bool_to_result, map_try_insert, try_blocks)]

mod enum_definition;
mod enum_impl;
mod state;

use crate::enum_definition::{
    collect_enum_definitions_from_dependencies, process_enum_definition,
    process_pending_enum_definitions,
};
use crate::enum_impl::{
    collect_enum_impls_from_dependencies, process_enum_impl, process_pending_enum_impls,
};
use crate::state::State;
use ab_riscv_macros_common::code_utils::pre_process_rust_code;
#[cfg(feature = "proc-macro")]
pub use ab_riscv_macros_impl::instruction;
use anyhow::Context;
use quote::ToTokens;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{env, fs, io, iter};
use syn::Item;

/// Processes all instruction macros in the crate when called from `build.rs`
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
        let (item_enum, dependencies, source) = maybe_enum_definition?;

        state.insert_known_enum_definition(item_enum, dependencies, source)?;
    }
    for maybe_enum_impl in collect_enum_impls_from_dependencies() {
        let (item_impl, source) = maybe_enum_impl?;
        state.insert_known_enum_impl(item_impl, source)?;
    }

    for maybe_rust_file in rust_files_in(Path::new(&manifest_dir).join("src")) {
        let rust_file = maybe_rust_file.context("Failed to collect Rust files")?;
        let rust_file = Rc::<Path>::from(rust_file.into_boxed_path());
        process_rust_file(rust_file.clone(), out_dir, &mut state)
            .with_context(|| format!("Failed to process Rust file `{}`", rust_file.display()))?;
    }

    process_pending_enum_definitions(out_dir, &mut state)?;
    process_pending_enum_impls(out_dir, &mut state)?;
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

fn process_rust_file(source: Rc<Path>, out_dir: &Path, state: &mut State) -> anyhow::Result<()> {
    let mut file_contents = fs::read_to_string(&source).context("Failed to read Rust file")?;
    if !file_contents.contains("#[instruction") {
        // Quickly skip files without instruction macro calls. This helps to ignore the files that
        // may use Rust nightly syntax features not supported by `syn`, which is limited to stable
        // Rust.
        return Ok(());
    }

    pre_process_rust_code(&mut file_contents);

    let file = syn::parse_file(&file_contents).context("Failed to parse Rust file")?;

    for item in file.items {
        match item {
            Item::Enum(item_enum) => {
                let enum_name = item_enum.ident.clone();
                process_enum_definition(item_enum, out_dir, state).with_context(|| {
                    format!(
                        "Failed to process enum `{enum_name}` in file `{}`",
                        source.display()
                    )
                })?;
            }
            Item::Impl(item_impl) => {
                let trait_name = item_impl.trait_.as_ref().map(|(_, path, _)| {
                    path.segments
                        .last()
                        .expect("Path is never empty; qed")
                        .ident
                        .clone()
                });
                let type_name = item_impl.self_ty.clone();
                if let Some(result) = process_enum_impl(item_impl, out_dir, state) {
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
            _ => {
                // Ignore
            }
        }
    }

    Ok(())
}
