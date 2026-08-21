mod extract_matches;
mod forbidden_checker;
mod generate_threaded_fns;
mod generate_variant_fns;

use crate::build::enum_impl::enum_name_from_impl;
use crate::build::execution_impl::extract_matches::extract_variant_arms;
use crate::build::execution_impl::forbidden_checker::block_contains_forbidden_syntax;
use crate::build::execution_impl::generate_threaded_fns::generate_threaded_fns;
use crate::build::execution_impl::generate_variant_fns::generate_variant_fns;
use crate::build::shared::{collect_all_dependencies, strip_const_where_predicates};
use crate::build::state::{
    PendingEnumCsrImpl, PendingEnumExecutionImpl, PendingEnumOperandsImpl, State,
};
use ab_riscv_macros_common::code_utils::{post_process_rust_code, pre_process_rust_code};
use anyhow::Context;
use prettyplease::unparse;
use quote::{ToTokens, quote};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::{env, fs, iter};
use syn::{
    FnArg, Ident, ImplItem, ImplItemFn, Item, ItemFn, ItemImpl, Member, Pat, PatType, Token,
    parse_file, parse_quote, parse_str,
};

const ORIGINAL_ENUM_EXECUTION_IMPL_ENV_VAR_SUFFIX: &str =
    "__INSTRUCTION_ENUM_ORIGINAL_EXECUTION_IMPL_PATH";
const ENUM_CSR_IMPL_ENV_VAR_SUFFIX: &str = "__INSTRUCTION_ENUM_CSR_IMPL_PATH";

pub(super) fn collect_enum_csr_impls_from_dependencies()
-> impl Iterator<Item = anyhow::Result<(ItemImpl, Rc<Path>)>> {
    // Collect exported instruction enums from dependencies
    env::vars().filter_map(|(key, value)| {
        if !key.ends_with(ENUM_CSR_IMPL_ENV_VAR_SUFFIX) {
            return None;
        }

        let result = try {
            let mut item_enum_contents = fs::read_to_string(&value).with_context(|| {
                format!(
                    "Failed to read Rust file `{value}` that is expected to contain instruction \
                    enum csr implementation"
                )
            })?;
            pre_process_rust_code(&mut item_enum_contents);
            let item_impl = parse_str::<ItemImpl>(&item_enum_contents).with_context(|| {
                format!(
                    "Failed to parse Rust file `{value}` that is expected to contain instruction \
                    enum csr implementation"
                )
            })?;

            (item_impl, Rc::from(Path::new(&value)))
        };

        Some(result)
    })
}

pub(super) fn collect_original_enum_execution_impls_from_dependencies()
-> impl Iterator<Item = anyhow::Result<(ItemImpl, Rc<Path>)>> {
    // Collect exported instruction enums from dependencies
    env::vars().filter_map(|(key, value)| {
        if !key.ends_with(ORIGINAL_ENUM_EXECUTION_IMPL_ENV_VAR_SUFFIX) {
            return None;
        }

        let result = try {
            let mut item_enum_contents = fs::read_to_string(&value).with_context(|| {
                format!(
                    "Failed to read Rust file `{value}` that is expected to contain original \
                    instruction enum execution implementation"
                )
            })?;
            pre_process_rust_code(&mut item_enum_contents);
            let item_impl = parse_str::<ItemImpl>(&item_enum_contents).with_context(|| {
                format!(
                    "Failed to parse Rust file `{value}` that is expected to contain original \
                    instruction enum execution implementation"
                )
            })?;

            (item_impl, Rc::from(Path::new(&value)))
        };

        Some(result)
    })
}

fn extract_prepare_csr_read_fn(item_impl: &ItemImpl) -> Option<&ImplItemFn> {
    for item in &item_impl.items {
        if let ImplItem::Fn(impl_item_fn) = item
            && impl_item_fn.sig.ident == "prepare_csr_read"
        {
            return Some(impl_item_fn);
        }
    }

    None
}

fn extract_prepare_csr_write_fn(item_impl: &ItemImpl) -> Option<&ImplItemFn> {
    for item in &item_impl.items {
        if let ImplItem::Fn(impl_item_fn) = item
            && impl_item_fn.sig.ident == "prepare_csr_write"
        {
            return Some(impl_item_fn);
        }
    }

    None
}

fn extract_prepare_csr_read_fn_mut(item_impl: &mut ItemImpl) -> Option<&mut ImplItemFn> {
    for item in &mut item_impl.items {
        if let ImplItem::Fn(impl_item_fn) = item
            && impl_item_fn.sig.ident == "prepare_csr_read"
        {
            return Some(impl_item_fn);
        }
    }

    None
}

fn extract_prepare_csr_write_fn_mut(item_impl: &mut ItemImpl) -> Option<&mut ImplItemFn> {
    for item in &mut item_impl.items {
        if let ImplItem::Fn(impl_item_fn) = item
            && impl_item_fn.sig.ident == "prepare_csr_write"
        {
            return Some(impl_item_fn);
        }
    }

    None
}

fn extract_execute_fn(item_impl: &ItemImpl) -> Option<&ImplItemFn> {
    for item in &item_impl.items {
        if let ImplItem::Fn(impl_item_fn) = item
            && impl_item_fn.sig.ident == "execute"
        {
            return Some(impl_item_fn);
        }
    }

    None
}

fn extract_execute_fn_from_impl_mut(impl_item: &mut [ImplItem]) -> Option<&mut ImplItemFn> {
    for item in impl_item {
        if let ImplItem::Fn(impl_item_fn) = item
            && impl_item_fn.sig.ident == "execute"
        {
            return Some(impl_item_fn);
        }
    }

    None
}

fn output_processed_enum_operands_impl(
    enum_name: &Ident,
    item_impl: ItemImpl,
    out_dir: &Path,
) -> anyhow::Result<()> {
    let enum_file_path = out_dir.join(format!("{enum_name}_operands_impl.rs"));
    let code = item_impl.to_token_stream().to_string();
    // Format
    let mut code = unparse(&parse_file(&code).expect("Generated code is valid; qed"));
    post_process_rust_code(&mut code);
    // This is a hack for pulling generics from decoding impl, which is `const`
    let code = code.replace("[const] ", "");

    // Avoid extra file truncation/override if it didn't change
    if fs::read_to_string(&enum_file_path).ok().as_ref() != Some(&code) {
        fs::write(&enum_file_path, code).with_context(|| {
            format!(
                "Failed to write generated Rust file with instruction operands implementation for \
                `{enum_name}`",
            )
        })?;
    }

    Ok(())
}

fn output_processed_enum_csr_impl(
    enum_name: &Ident,
    item_impl: ItemImpl,
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    let enum_file_path = out_dir.join(format!("{enum_name}_csr_impl.rs"));
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
                "Failed to write generated Rust file with instruction csr implementation for \
                `{enum_name}`",
            )
        })?;
    }
    println!(
        "cargo::metadata={}{ENUM_CSR_IMPL_ENV_VAR_SUFFIX}={}",
        enum_name,
        enum_file_path.display()
    );

    state.insert_known_enum_csr_impl(item_impl, Rc::from(enum_file_path))
}

fn output_processed_enum_execution_impl(
    enum_name: &Ident,
    original_item_impl: ItemImpl,
    item_impl: ItemImpl,
    variant_fns: Vec<ItemFn>,
    threaded_items: Vec<Item>,
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    {
        let enum_file_path = out_dir.join(format!("{enum_name}_execution_impl.rs"));
        let code = quote! { #( #variant_fns )* #item_impl #( #threaded_items )* }.to_string();
        // Format
        let mut code = unparse(&parse_file(&code).expect("Generated code is valid; qed"));
        post_process_rust_code(&mut code);

        // Avoid extra file truncation/override if it didn't change
        if fs::read_to_string(&enum_file_path).ok().as_ref() != Some(&code) {
            fs::write(&enum_file_path, code).with_context(|| {
                format!(
                    "Failed to write generated Rust file with instruction execution \
                    implementation for `{enum_name}`",
                )
            })?;
        }
    }
    {
        let original_enum_file_path =
            out_dir.join(format!("{enum_name}_original_execution_impl.rs"));
        let code = original_item_impl.to_token_stream().to_string();
        // Format
        let mut code = unparse(&parse_file(&code).expect("Original code is valid; qed"));
        // Normalize source
        let original_item_impl = parse_str(&code).expect("Original code is valid; qed");
        post_process_rust_code(&mut code);

        // Avoid extra file truncation/override if it didn't change
        if fs::read_to_string(&original_enum_file_path).ok().as_ref() != Some(&code) {
            fs::write(&original_enum_file_path, code).with_context(|| {
                format!(
                    "Failed to write Rust file with original instruction execution \
                    implementation for `{enum_name}`",
                )
            })?;
        }
        println!(
            "cargo::metadata={}{ORIGINAL_ENUM_EXECUTION_IMPL_ENV_VAR_SUFFIX}={}",
            enum_name,
            original_enum_file_path.display()
        );

        state.insert_known_original_enum_execution_impl(
            original_item_impl,
            Rc::from(original_enum_file_path),
        )
    }
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
    item_impl.attrs.remove(attribute_index);

    let Some((trait_path, _)) = &item_impl.trait_ else {
        return Some(Err(anyhow::anyhow!(
            "Expected  `#[instruction_execution] impl ExecutableInstructionOperands for {0}` or \
            `#[instruction_execution] impl ExecutableInstructionCsr for {0}` or \
            `#[instruction_execution] impl ExecutableInstruction for {0}`, but no trait was found",
            item_impl.self_ty.to_token_stream()
        )));
    };

    let last_trait_segment_path = trait_path
        .segments
        .last()
        .expect("Path is never empty; qed");

    Some(
        if last_trait_segment_path.ident == "ExecutableInstructionOperands" {
            process_enum_operands_impl(item_impl, out_dir, state)
        } else if last_trait_segment_path.ident == "ExecutableInstructionCsr" {
            process_enum_csr_impl(item_impl, out_dir, state)
        } else if last_trait_segment_path.ident == "ExecutableInstruction" {
            process_enum_execution_impl(item_impl, out_dir, state)
        } else {
            Err(anyhow::anyhow!(
                "Expected `impl` for `{}`, `#[instruction_execution]` attribute must be added to a \
                trait implementation, but trait `{}` is not supported",
                item_impl.self_ty.to_token_stream(),
                last_trait_segment_path.ident
            ))
        },
    )
}

pub(super) fn process_enum_operands_impl(
    mut item_impl: ItemImpl,
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    let enum_name = enum_name_from_impl(&item_impl);

    let Some(enum_definition) = state.get_known_enum_definition(&enum_name) else {
        eprintln!("{enum_name} operands is waiting on its definition");
        state.add_pending_enum_operands_impl(PendingEnumOperandsImpl { item_impl });
        return Ok(());
    };

    let all_dependencies = match collect_all_dependencies(
        state,
        enum_definition.direct_dependencies.iter().cloned(),
    ) {
        Ok(all_dependencies) => all_dependencies,
        Err(dependency_enum_name) => {
            eprintln!("{enum_name} operands is waiting on {dependency_enum_name} definition");
            state.add_pending_enum_operands_impl(PendingEnumOperandsImpl { item_impl });
            return Ok(());
        }
    };

    let mut all_where_predicates = Vec::new();

    for (dependency_enum_name, _dependency_enum_definition) in all_dependencies {
        let Some(dependency_enum_operands_impl) =
            state.get_known_original_enum_decoding_impl(&dependency_enum_name)
        else {
            eprintln!(
                "{enum_name} operands is waiting on {dependency_enum_name} decoding implementation"
            );
            state.add_pending_enum_operands_impl(PendingEnumOperandsImpl { item_impl });
            return Ok(());
        };

        if let Some(where_clause) = &dependency_enum_operands_impl
            .item_impl
            .generics
            .where_clause
        {
            all_where_predicates.extend(where_clause.predicates.iter().cloned());
        }
    }

    {
        let where_clause = item_impl
            .generics
            .where_clause
            .get_or_insert(parse_quote! { where });
        let mut already_inserted = where_clause
            .predicates
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        for predicate in all_where_predicates {
            if already_inserted.contains(&predicate) {
                continue;
            }
            already_inserted.insert(predicate.clone());
            where_clause.predicates.push(predicate);
        }
    }

    item_impl.items.push({
        let all_instructions = enum_definition
            .instructions
            .iter()
            .map(|variant| &variant.ident);

        parse_quote! {
            #[inline(always)]
            #[expect(clippy::allow_attributes, reason = "Attribute below")]
            #[allow(clippy::rest_pattern_accessible_field, reason = "Generated code")]
            #[allow(clippy::unnecessary_rest_pattern, reason = "Generated code")]
            fn get_rs1_rs2_operands(self) -> Rs1Rs2Operands<Self::Reg> {
                match self {
                    #( Self::#all_instructions { rs1, rs2, .. } => Rs1Rs2Operands { rs1, rs2 }, )*
                }
            }
        }
    });

    output_processed_enum_operands_impl(&enum_name, item_impl, out_dir)
}

pub(super) fn process_enum_csr_impl(
    mut item_impl: ItemImpl,
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    let enum_name = enum_name_from_impl(&item_impl);

    let Some(enum_definition) = state.get_known_enum_definition(&enum_name) else {
        eprintln!("{enum_name} CSR is waiting on its definition");
        state.add_pending_enum_csr_impl(PendingEnumCsrImpl { item_impl });
        return Ok(());
    };

    let all_dependencies = match collect_all_dependencies(
        state,
        enum_definition.direct_dependencies.iter().cloned(),
    ) {
        Ok(all_dependencies) => all_dependencies,
        Err(dependency_enum_name) => {
            eprintln!("{enum_name} CSR is waiting on {dependency_enum_name} definition");
            state.add_pending_enum_csr_impl(PendingEnumCsrImpl { item_impl });
            return Ok(());
        }
    };

    let mut all_prepare_csr_read_fns = Vec::new();
    let mut all_prepare_csr_write_fns = Vec::new();
    let mut all_where_predicates = Vec::new();

    // TODO: This should be changed to recursively combine all fundamental extensions instead of
    //  combined ones instead, such that extension D that depends two extensions B and C that both
    //  depend on the same extension A will get A included in D once rather than twice. This is
    //  caused by `get_known_enum_csr_impl` returning final extension implementation and not
    //  its original state as defined in the source code. Essentially, in addition to the final
    //  version, the original body of the implementation needs to be retained and used.
    for (dependency_enum_name, _dependency_enum_definition) in all_dependencies {
        let Some(dependency_enum_csr_impl) = state.get_known_enum_csr_impl(&dependency_enum_name)
        else {
            eprintln!("{enum_name} CSR is waiting on {dependency_enum_name} CSR implementation");
            state.add_pending_enum_csr_impl(PendingEnumCsrImpl { item_impl });
            return Ok(());
        };

        all_prepare_csr_read_fns.extend(extract_prepare_csr_read_fn(
            &dependency_enum_csr_impl.item_impl,
        ));
        all_prepare_csr_write_fns.extend(extract_prepare_csr_write_fn(
            &dependency_enum_csr_impl.item_impl,
        ));

        if let Some(where_clause) = &dependency_enum_csr_impl.item_impl.generics.where_clause {
            all_where_predicates.extend(where_clause.predicates.iter().cloned());
        }
    }

    let is_const = item_impl.attrs.last() == Some(&parse_quote! { #[cst] });

    {
        let where_clause = item_impl
            .generics
            .where_clause
            .get_or_insert(parse_quote! { where });
        let mut already_inserted = where_clause
            .predicates
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        for predicate in all_where_predicates {
            if already_inserted.contains(&predicate) {
                continue;
            }
            already_inserted.insert(predicate.clone());
            where_clause.predicates.push(predicate);
        }

        // Non-const implementations that inherit arms from const ones must not keep `[const]`
        if !is_const {
            strip_const_where_predicates(&mut where_clause.predicates);
        }
    }

    // Composition for `prepare_csr_read` method
    let maybe_prepare_csr_read_fn = extract_prepare_csr_read_fn_mut(&mut item_impl);
    if let Some(prepare_csr_read_fn) = maybe_prepare_csr_read_fn {
        (!block_contains_forbidden_syntax(&prepare_csr_read_fn.block)).ok_or_else(|| {
            anyhow::anyhow!(
                "Expected `#[instruction_execution] impl ExecutableInstructionCsr for {enum_name}` \
                must not have `return` in `prepare_csr_read` method"
            )
        })?;

        let all_prepare_csr_read_blocks = all_prepare_csr_read_fns
            .iter()
            .map(|impl_item_fn| &impl_item_fn.block)
            .chain(iter::once(&prepare_csr_read_fn.block));
        prepare_csr_read_fn.attrs.push(parse_quote! {
            #[expect(clippy::allow_attributes, reason = "Attribute below")]
        });
        prepare_csr_read_fn.attrs.push(parse_quote! {
            #[allow(clippy::try_err, reason = "Macro requirement")]
        });
        prepare_csr_read_fn.block = parse_quote! {{
            let mut accepted_by_at_least_one = false;
            #( if #all_prepare_csr_read_blocks? { accepted_by_at_least_one = true; } )*

            Ok(accepted_by_at_least_one)
        }};
    } else if let Some(&first_prepare_csr_read_fn) = all_prepare_csr_read_fns.first() {
        let all_prepare_csr_read_blocks = all_prepare_csr_read_fns
            .iter()
            .map(|impl_item_fn| &impl_item_fn.block);
        let mut base_fn = first_prepare_csr_read_fn.clone();
        base_fn.block = parse_quote! {{
            let mut accepted_by_at_least_one = false;
            #( if #all_prepare_csr_read_blocks? { accepted_by_at_least_one = true; } )*

            Ok(accepted_by_at_least_one)
        }};
        item_impl.items.push(ImplItem::Fn(base_fn));
    }

    // Composition for `prepare_csr_write` method
    let maybe_prepare_csr_write_fn = extract_prepare_csr_write_fn_mut(&mut item_impl);
    if let Some(prepare_csr_write_fn) = maybe_prepare_csr_write_fn {
        (!block_contains_forbidden_syntax(&prepare_csr_write_fn.block)).ok_or_else(|| {
            anyhow::anyhow!(
                "Expected `#[instruction_execution] impl ExecutableInstructionCsr for {enum_name}` \
                must not have `return` in `prepare_csr_write` method",
            )
        })?;

        let all_prepare_csr_write_blocks = all_prepare_csr_write_fns
            .iter()
            .map(|impl_item_fn| &impl_item_fn.block)
            .chain(iter::once(&prepare_csr_write_fn.block));
        prepare_csr_write_fn.attrs.push(parse_quote! {
            #[expect(clippy::allow_attributes, reason = "Attribute below")]
        });
        prepare_csr_write_fn.attrs.push(parse_quote! {
            #[allow(clippy::try_err, reason = "Macro requirement")]
        });
        prepare_csr_write_fn.block = parse_quote! {{
            let mut accepted_by_at_least_one = false;
            #( if #all_prepare_csr_write_blocks? { accepted_by_at_least_one = true; } )*

            Ok(accepted_by_at_least_one)
        }};
    } else if let Some(&first_prepare_csr_write_fn) = all_prepare_csr_write_fns.first() {
        let all_prepare_csr_write_blocks = all_prepare_csr_write_fns
            .iter()
            .map(|impl_item_fn| &impl_item_fn.block);
        let mut base_fn = first_prepare_csr_write_fn.clone();
        base_fn.block = parse_quote! {{
            let mut accepted_by_at_least_one = false;
            #( if #all_prepare_csr_write_blocks? { accepted_by_at_least_one = true; } )*

            Ok(accepted_by_at_least_one)
        }};
        item_impl.items.push(ImplItem::Fn(base_fn));
    }

    // Comments will be stripped, this will suppress some of the lints that are caused by it
    item_impl.attrs.insert(
        0,
        parse_quote! { #[expect(clippy::allow_attributes, reason = "Attribute below")] },
    );
    item_impl.attrs.insert(
        1,
        parse_quote! { #[allow(
            clippy::undocumented_unsafe_blocks,
            reason = "Comments will be stripped, this will suppress some of the lints that are \
            caused by it"
        )] },
    );

    output_processed_enum_csr_impl(&enum_name, item_impl, out_dir, state)
}

pub(super) fn process_enum_execution_impl(
    original_item_impl: ItemImpl,
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    let enum_name = enum_name_from_impl(&original_item_impl);
    let mut item_impl = original_item_impl.clone();

    let Some(execute_fn) = extract_execute_fn_from_impl_mut(&mut item_impl.items) else {
        return Err(anyhow::anyhow!(
            "Unexpected `impl` for `{}`, `#[instruction_execution]` attribute must be added to a \
            trait implementation, but no `execute` method was found",
            item_impl.self_ty.to_token_stream()
        ));
    };

    // Every argument is normalized to not start with `_` in the name, so the later code can rely on
    // canonical names
    for input in &mut execute_fn.sig.inputs {
        let FnArg::Typed(PatType {
            pat,
            attrs: _,
            colon_token: _,
            ty: _,
        }) = input
        else {
            continue;
        };

        match pat.as_mut() {
            Pat::Ident(pat_ident) => {
                let ident = pat_ident.ident.to_string();
                let Some(name) = ident.strip_prefix('_') else {
                    continue;
                };

                pat_ident.ident = Ident::new(name, pat_ident.ident.span());
            }
            Pat::Struct(pat_struct) => {
                for field in &mut pat_struct.fields {
                    let Member::Named(member) = &field.member else {
                        continue;
                    };

                    *field.pat = parse_quote! { #member };
                    // The binding and the field are now spelled the same, so the field name is
                    // redundant
                    field.colon_token = None;
                }
            }
            _ => {}
        }
    }

    // Support for `no_panic` macro, which is moved from the method to each extracted instruction
    // execution function
    let no_panic_attr_index = execute_fn.attrs.iter().position(|attr| {
        attr.path().is_ident("cfg_attr") && attr.to_token_stream().to_string().contains("no_panic")
    });
    let no_panic_attr = no_panic_attr_index.map(|index| execute_fn.attrs.remove(index));

    let Some(enum_definition) = state.get_known_enum_definition(&enum_name) else {
        eprintln!("{enum_name} execution is waiting on its definition");
        state.add_pending_enum_execution_impl(PendingEnumExecutionImpl { original_item_impl });
        return Ok(());
    };

    let all_dependencies = match collect_all_dependencies(
        state,
        enum_definition.direct_dependencies.iter().cloned(),
    ) {
        Ok(all_dependencies) => all_dependencies,
        Err(dependency_enum_name) => {
            eprintln!("{enum_name} execution is waiting on {dependency_enum_name} definition");
            state.add_pending_enum_execution_impl(PendingEnumExecutionImpl { original_item_impl });
            return Ok(());
        }
    };

    let mut all_execute_blocks = Vec::new();
    let mut all_where_predicates = Vec::new();

    for (dependency_enum_name, _dependency_enum_definition) in all_dependencies {
        let Some(dependency_enum_execution_impl) =
            state.get_known_original_enum_execution_impl(&dependency_enum_name)
        else {
            eprintln!(
                "{enum_name} execution is waiting on {dependency_enum_name} execution \
                implementation"
            );
            state.add_pending_enum_execution_impl(PendingEnumExecutionImpl { original_item_impl });
            return Ok(());
        };

        all_execute_blocks.push(
            &extract_execute_fn(&dependency_enum_execution_impl.item_impl)
                .expect("Dependencies are all valid; qed")
                .block,
        );
        if let Some(where_clause) = &dependency_enum_execution_impl
            .item_impl
            .generics
            .where_clause
        {
            all_where_predicates.extend(where_clause.predicates.iter().cloned());
        }
    }

    let is_const = item_impl.attrs.last() == Some(&parse_quote! { #[cst] });

    if let Some(where_clause) = &mut item_impl.generics.where_clause {
        let mut already_inserted = where_clause
            .predicates
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        for predicate in all_where_predicates {
            if already_inserted.contains(&predicate) {
                continue;
            }
            already_inserted.insert(predicate.clone());
            where_clause.predicates.push(predicate);
        }

        // Non-const implementations that inherit arms from const ones must not keep `[const]`
        if !is_const {
            strip_const_where_predicates(&mut where_clause.predicates);
        }
    } else {
        return Err(anyhow::anyhow!(
            "Missing where clause on `#[instruction_execution] impl ExecutableInstruction for \
            {enum_name}`"
        ));
    }

    let all_execute_blocks = all_execute_blocks
        .into_iter()
        .chain(iter::once(&execute_fn.block));

    let mut all_instructions = HashMap::new();
    for block in all_execute_blocks {
        for maybe_instruction in extract_variant_arms(block)? {
            let (variant_name, instruction_arm) = maybe_instruction?;
            all_instructions.insert(variant_name, instruction_arm);
        }
    }

    let mut match_arms = enum_definition
        .instructions
        .iter()
        .map(|variant| {
            let ident = &variant.ident;
            all_instructions.remove(ident).with_context(|| {
                format!(
                    "Instruction `{ident}` not found in all_instructions for enum `{enum_name}`"
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let variant_fns = generate_variant_fns(
        &enum_name,
        &item_impl.self_ty,
        &item_impl.generics,
        is_const.then(<Token![const]>::default),
        no_panic_attr.as_ref(),
        &execute_fn.sig,
        &enum_definition.instructions,
        &mut match_arms,
    )?;

    // The tail-call-threaded implementation is generated from the very same arms, alongside the
    // `match`-based `execute()` rather than instead of it
    let threaded_items = generate_threaded_fns(
        &enum_name,
        &item_impl.self_ty,
        &item_impl.generics,
        &enum_definition.instructions,
        &match_arms,
    )?;

    execute_fn.block = parse_quote! {{
        match self {
            #( #match_arms )*
        }
    }};

    output_processed_enum_execution_impl(
        &enum_name,
        original_item_impl,
        item_impl,
        variant_fns,
        threaded_items,
        out_dir,
        state,
    )
}

/// Process remaining enums that were waiting for dependencies
pub(super) fn process_pending_enum_execution_impls(
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    {
        let mut last_pending_enums_count = 0;
        loop {
            let pending_enums = state.take_pending_enum_operands_impls();

            if pending_enums.is_empty() {
                break;
            }

            if pending_enums.len() == last_pending_enums_count {
                return Err(anyhow::anyhow!(
                    "Failed to process instruction `#[instruction_execution]` macro on \
                    `ExecutableInstructionOperands`, circular dependency detected, pending_enums: \
                    {:?}",
                    pending_enums
                        .iter()
                        .map(|pending_enum| enum_name_from_impl(&pending_enum.item_impl))
                        .collect::<Vec<_>>()
                ));
            }
            last_pending_enums_count = pending_enums.len();

            for PendingEnumOperandsImpl { item_impl } in pending_enums {
                process_enum_operands_impl(item_impl, out_dir, state)?;
            }
        }
    }
    {
        let mut last_pending_enums_count = 0;
        loop {
            let pending_enums = state.take_pending_enum_csr_impls();

            if pending_enums.is_empty() {
                break;
            }

            if pending_enums.len() == last_pending_enums_count {
                return Err(anyhow::anyhow!(
                    "Failed to process instruction `#[instruction_execution]` macro on \
                    `ExecutableInstructionCsr`, circular dependency detected, pending_enums: {:?}",
                    pending_enums
                        .iter()
                        .map(|pending_enum| enum_name_from_impl(&pending_enum.item_impl))
                        .collect::<Vec<_>>()
                ));
            }
            last_pending_enums_count = pending_enums.len();

            for PendingEnumCsrImpl { item_impl } in pending_enums {
                process_enum_csr_impl(item_impl, out_dir, state)?;
            }
        }
    }
    {
        let mut last_pending_enums_count = 0;
        loop {
            let pending_enums = state.take_pending_enum_execution_impls();

            if pending_enums.is_empty() {
                break;
            }

            if pending_enums.len() == last_pending_enums_count {
                return Err(anyhow::anyhow!(
                    "Failed to process instruction `#[instruction_execution]` macro on \
                    `ExecutableInstruction`, circular dependency detected, pending_enums: {:?}",
                    pending_enums
                        .iter()
                        .map(|pending_enum| enum_name_from_impl(&pending_enum.original_item_impl))
                        .collect::<Vec<_>>()
                ));
            }
            last_pending_enums_count = pending_enums.len();

            for PendingEnumExecutionImpl { original_item_impl } in pending_enums {
                process_enum_execution_impl(original_item_impl, out_dir, state)?;
            }
        }
    }

    Ok(())
}
