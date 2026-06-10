mod add_missing_fields;
mod forbidden_checker;
mod ignored_variants_remover;

use crate::build::enum_impl::add_missing_fields::add_missing_rs_fields;
use crate::build::enum_impl::forbidden_checker::block_contains_forbidden_syntax;
use crate::build::enum_impl::ignored_variants_remover::remove_ignored_variants;
use crate::build::shared::collect_all_dependencies;
use crate::build::state::{PendingEnumDisplayImpl, PendingEnumImpl, State};
use ab_riscv_macros_common::code_utils::{post_process_rust_code, pre_process_rust_code};
use anyhow::Context;
use prettyplease::unparse;
use quote::{ToTokens, format_ident, quote};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::{env, fs, iter, mem};
use syn::token::DotDot;
use syn::{
    Block, Expr, Fields, FnArg, Ident, ImplItem, ItemImpl, Member, Pat, PatRest, Stmt, Type,
    parse_file, parse_quote, parse_str,
};

const ORIGINAL_ENUM_DECODING_IMPL_ENV_VAR_SUFFIX: &str = "__INSTRUCTION_ENUM_ORIGINAL_IMPL_PATH";

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

pub(super) fn collect_original_enum_decoding_impls_from_dependencies()
-> impl Iterator<Item = anyhow::Result<(ItemImpl, Rc<Path>)>> {
    // Collect exported instruction enums from dependencies
    env::vars().filter_map(|(key, value)| {
        if !key.ends_with(ORIGINAL_ENUM_DECODING_IMPL_ENV_VAR_SUFFIX) {
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

pub(super) struct InstructionImplBlocks<'a> {
    try_decode: &'a Block,
    alignment: &'a Block,
    size: &'a Block,
}

pub(super) struct InstructionImplBlocksMut<'a> {
    try_decode: &'a mut Block,
    alignment: &'a mut Block,
    size: &'a mut Block,
}

fn extract_instruction_blocks_from_impl(
    impl_items: &[ImplItem],
) -> Option<InstructionImplBlocks<'_>> {
    let mut try_decode = None;
    let mut alignment = None;
    let mut size = None;

    for item in impl_items {
        if let ImplItem::Fn(impl_item_fn) = item {
            match impl_item_fn.sig.ident.to_string().as_str() {
                "try_decode" => {
                    try_decode.replace(&impl_item_fn.block);
                }
                "alignment" => {
                    alignment.replace(&impl_item_fn.block);
                }
                "size" => {
                    size.replace(&impl_item_fn.block);
                }
                _ => {
                    // Something else
                }
            }
        }
    }

    Some(InstructionImplBlocks {
        try_decode: try_decode?,
        alignment: alignment?,
        size: size?,
    })
}

fn extract_instruction_blocks_from_impl_mut(
    impl_items: &mut [ImplItem],
) -> Option<InstructionImplBlocksMut<'_>> {
    let mut try_decode = None;
    let mut alignment = None;
    let mut size = None;

    for item in impl_items {
        if let ImplItem::Fn(impl_item_fn) = item {
            match impl_item_fn.sig.ident.to_string().as_str() {
                "try_decode" => {
                    try_decode.replace(&mut impl_item_fn.block);
                }
                "alignment" => {
                    alignment.replace(&mut impl_item_fn.block);
                }
                "size" => {
                    size.replace(&mut impl_item_fn.block);
                }
                _ => {
                    // Something else
                }
            }
        }
    }

    Some(InstructionImplBlocksMut {
        try_decode: try_decode?,
        alignment: alignment?,
        size: size?,
    })
}

fn output_processed_enum_decoding_impl(
    enum_name: &Ident,
    original_item_impl: ItemImpl,
    item_impl: ItemImpl,
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    {
        let enum_file_path = out_dir.join(format!("{enum_name}_decoding_impl.rs"));
        let code = item_impl.to_token_stream().to_string();
        // Format
        let mut code = unparse(&parse_file(&code).expect("Generated code is valid; qed"));
        post_process_rust_code(&mut code);

        // Avoid extra file truncation/override if it didn't change
        if fs::read_to_string(&enum_file_path).ok().as_ref() != Some(&code) {
            fs::write(&enum_file_path, code).with_context(|| {
                format!(
                    "Failed to write generated Rust file with instruction decoding implementation \
                    for `{enum_name}`"
                )
            })?;
        }
    }
    {
        let original_enum_file_path =
            out_dir.join(format!("{enum_name}_original_decoding_impl.rs"));
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
                    "Failed to write Rust file with original instruction decoding implementation \
                    for `{enum_name}`"
                )
            })?;
        }
        println!(
            "cargo::metadata={}{ORIGINAL_ENUM_DECODING_IMPL_ENV_VAR_SUFFIX}={}",
            enum_name,
            original_enum_file_path.display()
        );

        state.insert_known_original_enum_decoding_impl(
            original_item_impl,
            Rc::from(original_enum_file_path),
        )
    }
}

fn output_processed_enum_display_impl(
    enum_name: Ident,
    item_impl: ItemImpl,
    out_dir: &Path,
) -> anyhow::Result<()> {
    let enum_file_path = out_dir.join(format!("{enum_name}_display_impl.rs"));
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
            `#[instruction] impl Display for {0}`, but no trait was found",
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
    original_item_impl: ItemImpl,
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    let enum_name = enum_name_from_impl(&original_item_impl);
    let mut item_impl = original_item_impl.clone();

    let Some(blocks) = extract_instruction_blocks_from_impl_mut(&mut item_impl.items) else {
        return Err(anyhow::anyhow!(
            "Expected `#[instruction] impl Instruction for {}` to contain `try_decode`, \
            `alignment`, and `size` methods, but at least one was not found",
            item_impl.self_ty.to_token_stream()
        ));
    };
    let try_decode_block = blocks.try_decode;
    let alignment_block = blocks.alignment;
    let size_block = blocks.size;
    (!block_contains_forbidden_syntax(try_decode_block, &enum_name)).ok_or_else(|| {
        anyhow::anyhow!(
            "Expected `#[instruction] impl Instruction for {enum_name}` must not have `return` or \
            enum construction other than through `Self::` in `try_decode` method"
        )
    })?;

    let Some(enum_definition) = state.get_known_enum_definition(&enum_name) else {
        state.add_pending_enum_impl(PendingEnumImpl { item_impl });
        return Ok(());
    };

    let all_dependencies = match collect_all_dependencies(
        state,
        enum_definition.direct_dependencies.iter().cloned(),
    ) {
        Ok(all_dependencies) => all_dependencies,
        Err(dependency_enum_name) => {
            eprintln!("{enum_name} decoding is waiting on {dependency_enum_name} definition");
            state.add_pending_enum_impl(PendingEnumImpl { item_impl });
            return Ok(());
        }
    };

    let mut all_try_decode_blocks = Vec::new();
    let mut all_dependency_alignment_blocks = Vec::new();
    let mut all_dependency_size_entries = Vec::new();
    let mut all_where_predicates = Vec::new();

    for (dependency_enum_name, dependency_enum_definition) in all_dependencies {
        let Some(dependency_enum_impl) =
            state.get_known_original_enum_decoding_impl(&dependency_enum_name)
        else {
            eprintln!(
                "{enum_name} decoding is waiting on {dependency_enum_name} decoding implementation"
            );
            state.add_pending_enum_impl(PendingEnumImpl { item_impl });
            return Ok(());
        };

        let dependency_blocks =
            extract_instruction_blocks_from_impl(&dependency_enum_impl.item_impl.items)
                .expect("Dependencies are all valid; qed");

        all_try_decode_blocks.push(dependency_blocks.try_decode);
        all_dependency_alignment_blocks.push(dependency_blocks.alignment);

        let variant_idents = dependency_enum_definition
            .instructions
            .iter()
            .map(|v| &v.ident)
            .collect::<Vec<_>>();
        all_dependency_size_entries.push((variant_idents, dependency_blocks.size));

        if let Some(where_clause) = &dependency_enum_impl.item_impl.generics.where_clause {
            all_where_predicates.extend(where_clause.predicates.iter().cloned());
        }
    }

    if !all_where_predicates.is_empty() {
        let where_clause = item_impl
            .generics
            .where_clause
            .get_or_insert_with(|| parse_quote! { where });

        let mut already_inserted = where_clause
            .predicates
            .iter()
            .cloned()
            .collect::<HashSet<_>>();

        for predicate in all_where_predicates {
            if already_inserted.insert(predicate.clone()) {
                where_clause.predicates.push(predicate);
            }
        }
    }

    let allowed_instruction = enum_definition
        .instructions
        .iter()
        .map(|instruction| &instruction.ident)
        .collect::<HashSet<_>>();

    // Process `try_decode()` method
    // TODO: This simply concatenates individual decoding blocks, but it'd be much nicer to combine
    //  multiple `match` statements into one, merging branches with the same opcode. This,
    //  unfortunately, is much more complex, so skipped in the initial implementation.
    let all_try_decode_blocks = all_try_decode_blocks
        .into_iter()
        .chain(iter::once(&*try_decode_block))
        .cloned()
        .map(|mut block| {
            remove_ignored_variants(&mut block, &allowed_instruction);
            block
        });

    *try_decode_block = parse_quote! {{
        #[expect(clippy::allow_attributes, reason = "Attribute below")]
        #[allow(
            clippy::if_same_then_else,
            reason = "In presence of ignored instructions, simple replacement sometimes results in \
            redundant code like `Some(None?)`"
        )]
        #[allow(
            clippy::same_functions_in_if_condition,
            reason = "In presence of ignored instructions, simple replacement sometimes results in \
            redundant code like `Some(None?)`"
        )]
        #( if let Some(decoded) = try { #all_try_decode_blocks? } { Some(decoded) } else )*

        { None }
    }};

    add_missing_rs_fields(try_decode_block);

    // Process `alignment()` method: combine all unique bodies with `.min(...)` calls, keeping
    // the own block last. Bodies that are token-identical are deduplicated to avoid redundant
    // comparisons at runtime.
    if !all_dependency_alignment_blocks.is_empty() {
        let own_tokens = alignment_block.to_token_stream().to_string();

        let mut seen_tokens = HashSet::from([own_tokens]);
        let mut unique_dep_blocks = all_dependency_alignment_blocks
            .into_iter()
            .filter(|block| seen_tokens.insert(block.to_token_stream().to_string()))
            .peekable();

        if unique_dep_blocks.peek().is_some() {
            *alignment_block = parse_quote! {{
                #[expect(clippy::allow_attributes, reason = "Attribute below")]
                #[allow(
                    unused_braces,
                    reason = "Combining blocks often results in `.min({expr})`, where `{expr}` is \
                    very simple, which makes `{}` redundant"
                )]
                { #alignment_block #( .min(#unique_dep_blocks) )* }
            }};
        }
    }

    // Process `size()` method: if all dependency bodies are token-identical to the own body,
    // leave it unchanged (optimization). Otherwise, build a `match self { ... }` where each
    // group of variants is dispatched to its originating dependency's body.
    if !all_dependency_size_entries.is_empty() {
        let own_tokens = size_block.to_token_stream().to_string();

        // Variants covered by at least one dependency entry
        let mut already_covered = HashSet::new();

        let mut all_dependency_size_entries = all_dependency_size_entries;
        // Filter-out extra elements
        all_dependency_size_entries.retain_mut(|(idents, _block)| {
            idents.retain(|ident| {
                allowed_instruction.contains(ident) && already_covered.insert(*ident)
            });

            !idents.is_empty()
        });

        let all_same = all_dependency_size_entries
            .iter()
            .all(move |(_, block)| block.to_token_stream().to_string() == own_tokens);

        if !all_same {
            let mut match_arms = Vec::new();

            for (variant_idents, block) in all_dependency_size_entries {
                match_arms.push(quote! {
                    #( Self::#variant_idents { .. } )|* => #block
                });
            }

            // Remaining variants that belong only to the current enum's own body
            let mut own_only = enum_definition
                .instructions
                .iter()
                .filter(|variant| !already_covered.contains(&variant.ident))
                .peekable();

            if own_only.peek().is_some() {
                match_arms.push(quote! {
                    #( Self::#own_only { .. } )|* => #size_block
                });
            }

            *size_block = parse_quote! {{
                match self {
                    #( #match_arms, )*
                }
            }};
        }
    }

    item_impl
        .attrs
        .push(parse_quote! { #[automatically_derived] });

    output_processed_enum_decoding_impl(&enum_name, original_item_impl, item_impl, out_dir, state)
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

    let all_dependencies = match collect_all_dependencies(
        state,
        enum_definition.direct_dependencies.iter().cloned(),
    ) {
        Ok(all_dependencies) => all_dependencies,
        Err(dependency_enum_name) => {
            eprintln!("{enum_name} display is waiting on {dependency_enum_name} definition");
            state.add_pending_enum_display_impl(PendingEnumDisplayImpl { item_impl });
            return Ok(());
        }
    };

    let mut variants_from_dependencies = HashMap::new();

    for (dependency_enum_name, dependency_enum_definition) in all_dependencies {
        let dependency_enum_name = Rc::new(dependency_enum_name);

        for instruction in &dependency_enum_definition.instructions {
            variants_from_dependencies
                .insert(Rc::clone(instruction), Rc::clone(&dependency_enum_name));
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

    expr_match.arms.retain_mut(|arm| {
        let path = match &arm.pat {
            Pat::Struct(pat_struct) => pat_struct.path.clone(),
            Pat::Path(expr_path) => expr_path.path.clone(),
            _ => {
                return false;
            }
        };

        if !path
            .segments
            .last()
            .map_or_default(|path_segment| allowed_instruction.contains(&path_segment.ident))
        {
            return false;
        }

        let Pat::Struct(pat_struct) = &mut arm.pat else {
            // Must be path otherwise, replace it with a trivial struct
            arm.pat = parse_quote! { #path { .. } };
            return true;
        };

        if pat_struct.rest.is_some() {
            return true;
        }

        let mut has_ignore = false;
        let mut rs1_found = false;
        let mut rs2_found = false;
        for field in &pat_struct.fields {
            if let Pat::Wild(_) = &*field.pat {
                has_ignore = true;
            }

            let Member::Named(ident) = &field.member else {
                return false;
            };

            if ident == "rs1" {
                rs1_found = true;
            } else if ident == "rs2" {
                rs2_found = true;
            }
        }

        if !rs1_found || !rs2_found {
            if has_ignore {
                // This prevents clippy warnings
                pat_struct.fields = mem::take(&mut pat_struct.fields)
                    .into_iter()
                    .filter_map(|field| {
                        if let Pat::Wild(_) = &*field.pat {
                            return None;
                        }

                        Some(field)
                    })
                    .collect();
            }
            pat_struct.rest.replace(PatRest {
                attrs: vec![],
                dot2_token: DotDot::default(),
            });
        }

        true
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
                    "Failed to process `#[instruction]` macro, circular dependency detected, \
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
                    "Failed to process `#[instruction]` macro, circular dependency detected, \
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
