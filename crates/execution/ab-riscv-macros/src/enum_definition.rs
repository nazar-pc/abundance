use crate::state::{PendingEnumDefinition, State};
use ab_riscv_macros_common::code_utils::pre_process_rust_code;
use anyhow::Context;
use quote::{ToTokens, format_ident};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::{env, fs};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Error, Ident, ItemEnum, Meta, Token, Variant, bracketed, parse_str, parse2};

const ENUM_DEFINITION_ENV_VAR_SUFFIX: &str = "__INSTRUCTION_ENUM_DEFINITION_PATH";
const ENUM_DEPENDENCIES_ENV_VAR_SUFFIX: &str = "__INSTRUCTION_ENUM_DEPENDENCIES";

/// Attribute for `#[instruction]` macro used on enum definition
#[derive(Debug, Default)]
pub(super) struct InstructionDefinition {
    pub(super) items: Punctuated<InstructionDefinitionItem, Token![,]>,
}

impl Parse for InstructionDefinition {
    fn parse(input: ParseStream<'_>) -> Result<Self, Error> {
        Ok(Self {
            items: input.parse_terminated(InstructionDefinitionItem::parse, Token![,])?,
        })
    }
}

/// Attribute item for `#[instruction]` macro used on enum definition
#[derive(Debug)]
pub(super) enum InstructionDefinitionItem {
    /// Specifies the order of instruction variants
    Reorder(Vec<Ident>),
    /// Specifies ignored instruction variants or whole enums
    Ignore(Vec<Ident>),
    /// Specifies the inherited instruction variants
    Inherit(Vec<Ident>),
}

impl InstructionDefinitionItem {
    fn parse(input: ParseStream<'_>) -> Result<Self, Error> {
        let key: Ident = input.parse()?;
        input.parse::<Token![=]>()?;

        let content;
        bracketed!(content in input);

        let values: Punctuated<Ident, Token![,]> =
            content.parse_terminated(Ident::parse, Token![,])?;

        match key.to_string().as_str() {
            "reorder" => Ok(Self::Reorder(values.into_iter().collect())),
            "ignore" => Ok(Self::Ignore(values.into_iter().collect())),
            "inherit" => Ok(Self::Inherit(values.into_iter().collect())),
            _ => Err(Error::new_spanned(key, "unknown instruction attribute key")),
        }
    }
}

struct KnownInstruction {
    instruction: Rc<Variant>,
    source: Rc<Path>,
}

pub(super) fn collect_enum_definitions_from_dependencies()
-> impl Iterator<Item = anyhow::Result<(ItemEnum, Vec<Ident>, Rc<Path>)>> {
    // Collect exported instruction enums from dependencies
    env::vars().filter_map(|(key, source)| {
        if !key.ends_with(ENUM_DEFINITION_ENV_VAR_SUFFIX) {
            return None;
        }

        let result = try {
            let dependencies_key = format!(
                "{}{ENUM_DEPENDENCIES_ENV_VAR_SUFFIX}",
                &key[..key.len() - ENUM_DEFINITION_ENV_VAR_SUFFIX.len()]
            );
            let dependencies_string = env::var(&dependencies_key).with_context(|| {
                format!(
                    "Failed to read environment variable `{}` that is expected to contain \
                    instruction enum dependencies",
                    dependencies_key
                )
            })?;
            let dependencies = dependencies_string
                .split('=')
                .nth(1)
                .with_context(|| {
                    format!(
                        "Dependency must follow the pattern \
                        `Instruction=InstructionA,InstructionB`: {dependencies_string}"
                    )
                })?
                .split(',')
                .filter(|dependency| !dependency.is_empty())
                .map(|dependency| format_ident!("{dependency}"))
                .collect::<Vec<_>>();

            let source = Rc::from(Path::new(&source));

            let mut item_enum_contents = fs::read_to_string(&source).with_context(|| {
                format!(
                    "Failed to read Rust file `{}` that is expected to contain instruction \
                    enum definition",
                    source.display()
                )
            })?;
            pre_process_rust_code(&mut item_enum_contents);
            let item_enum = parse_str::<ItemEnum>(&item_enum_contents).with_context(|| {
                format!(
                    "Failed to parse Rust file `{}` that is expected to contain instruction \
                    enum definition",
                    source.display()
                )
            })?;

            // Re-export metadata so it is available for downstream crates even if they don't depend
            // directly on a crate where upstream instruction is defined
            println!(
                "cargo::metadata={}{ENUM_DEFINITION_ENV_VAR_SUFFIX}={}",
                item_enum.ident,
                source.display()
            );
            println!(
                "cargo::metadata={}{ENUM_DEPENDENCIES_ENV_VAR_SUFFIX}={dependencies_string}",
                item_enum.ident
            );

            (item_enum, dependencies, source)
        };

        Some(result)
    })
}

fn output_processed_enum_definition(
    item_enum: ItemEnum,
    dependencies: Vec<Ident>,
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    let enum_file_path = out_dir.join(format!("{}_definition.rs", item_enum.ident));
    let code = item_enum.to_token_stream().to_string();

    // Avoid extra file truncation/override if it didn't change
    if fs::read_to_string(&enum_file_path).ok().as_ref() != Some(&code) {
        fs::write(&enum_file_path, code).with_context(|| {
            format!(
                "Failed to write generated Rust file with instruction enum `{}`",
                item_enum.ident,
            )
        })?;
    }
    println!(
        "cargo::metadata={}{ENUM_DEFINITION_ENV_VAR_SUFFIX}={}",
        item_enum.ident,
        enum_file_path.display()
    );
    println!(
        "cargo::metadata={}{ENUM_DEPENDENCIES_ENV_VAR_SUFFIX}={}={}",
        item_enum.ident,
        item_enum.ident,
        dependencies
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(","),
    );

    state.insert_known_enum_definition(item_enum, dependencies, Rc::from(enum_file_path))
}

/// Identify enums with an `#[instruction]` attribute and generate Rust files with the enum
/// definition after processing attribute contents and propagate the environment variable with the
/// path do dependent crates
pub(super) fn process_enum_definition(
    mut item_enum: ItemEnum,
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    let Some(attribute_index) = item_enum
        .attrs
        .iter()
        .enumerate()
        .find_map(|(index, attr)| attr.meta.path().is_ident("instruction").then_some(index))
    else {
        // Enum without `#[instruction]` attribute, skip it
        return Ok(());
    };

    // We'll store the whole enum in the Rust file, so remove any attributes before `#[instruction]`
    // attribute to avoid conflicts
    let mut removed_attributes = item_enum.attrs.drain(..=attribute_index);
    let attribute = removed_attributes
        .next_back()
        .expect("`#[instruction]` attribute is found; qed");

    for removed_attribute in removed_attributes {
        if removed_attribute.path().is_ident("derive") {
            return Err(anyhow::anyhow!(
                "All `#[derive(...)]` attributes must be after `#[instruction(...)]` attribute, found: {}",
                removed_attribute.to_token_stream()
            ));
        }
    }

    let dependencies = match attribute.meta {
        Meta::Path(_) => {
            process_enum_definition_with_variants(&mut item_enum, state)?;
            Vec::new()
        }
        Meta::List(meta_list) => {
            let instruction_definition = parse2::<InstructionDefinition>(meta_list.tokens)
                .context("Failed to parse `#[instruction(...)]` attribute")?;

            if instruction_definition.items.iter().any(|item| match item {
                InstructionDefinitionItem::Reorder(_) | InstructionDefinitionItem::Ignore(_) => {
                    false
                }
                InstructionDefinitionItem::Inherit(enums) => enums
                    .iter()
                    .any(|enum_name| state.get_known_enum_definition(enum_name).is_none()),
            }) {
                state.add_pending_enum_definition(PendingEnumDefinition {
                    instruction_definition,
                    item_enum,
                });
                return Ok(());
            }

            let Some(result) =
                process_enum_definition_inherited(instruction_definition, item_enum, state)?
            else {
                return Ok(());
            };
            let dependencies;
            (item_enum, dependencies) = result;
            dependencies
        }
        Meta::NameValue(meta_name_value) => {
            return Err(anyhow::anyhow!(
                "Unexpected `#[instruction = {}]` attribute",
                Meta::NameValue(meta_name_value).to_token_stream()
            ));
        }
    };

    output_processed_enum_definition(item_enum, dependencies, out_dir, state)
}

fn process_enum_definition_with_variants(
    _item_enum: &mut ItemEnum,
    _state: &mut State,
) -> anyhow::Result<()> {
    // No special processing needed, at least not yet
    Ok(())
}

fn process_enum_definition_inherited(
    instruction_definition: InstructionDefinition,
    mut item_enum: ItemEnum,
    state: &mut State,
) -> anyhow::Result<Option<(ItemEnum, Vec<Ident>)>> {
    let mut all_known_instructions = HashMap::<Ident, KnownInstruction>::new();
    let mut dependencies = Vec::new();

    for item in &instruction_definition.items {
        match item {
            InstructionDefinitionItem::Reorder(_) | InstructionDefinitionItem::Ignore(_) => {
                // Ignore for now
            }
            InstructionDefinitionItem::Inherit(inherit_enums) => {
                for inherit_enum in inherit_enums {
                    dependencies.push(inherit_enum.clone());
                    let Some(known_enum) = state.get_known_enum_definition(inherit_enum) else {
                        return Err(anyhow::anyhow!(
                            "Unknown inherit enum `{}` in `#[instruction(...)]` attribute",
                            inherit_enum
                        ));
                    };

                    for known_instruction in &known_enum.instructions {
                        match all_known_instructions.entry(known_instruction.ident.clone()) {
                            Entry::Occupied(entry) => {
                                let existing_known_instruction = entry.get();
                                if &existing_known_instruction.instruction != known_instruction {
                                    return Err(anyhow::anyhow!(
                                        "Duplicate instruction definition that is not identical to \
                                        an existing one: `{}` ({}) != `{}` ({})",
                                        existing_known_instruction.instruction.to_token_stream(),
                                        existing_known_instruction.source.display(),
                                        known_instruction.to_token_stream(),
                                        known_enum.source.display(),
                                    ));
                                }
                            }
                            Entry::Vacant(entry) => {
                                let instruction = Rc::new(known_instruction);

                                entry.insert(KnownInstruction {
                                    instruction: Rc::clone(&instruction),
                                    source: Rc::clone(&known_enum.source),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    let mut included_instructions = HashSet::new();
    let mut instructions = Vec::new();

    let mut own_instructions = item_enum
        .variants
        .iter()
        .map(|variant| (variant.ident.clone(), Rc::new(variant.clone())))
        .collect::<HashMap<_, _>>();

    for item in &instruction_definition.items {
        match item {
            InstructionDefinitionItem::Reorder(reorder_variants) => {
                for reorder_variant in reorder_variants {
                    if included_instructions.contains(reorder_variant) {
                        return Err(anyhow::anyhow!(
                            "Instruction `{}` in `#[instruction(...)]`'s `reorder` attribute is \
                            already included earlier",
                            reorder_variant
                        ));
                    }
                    included_instructions.insert(reorder_variant.clone());

                    let instruction = if let Some(instruction) =
                        own_instructions.remove(reorder_variant)
                    {
                        instruction
                    } else if let Some(instruction) = all_known_instructions.get(reorder_variant) {
                        Rc::clone(&instruction.instruction)
                    } else {
                        return Err(anyhow::anyhow!(
                            "Unknown reorder instruction `{}` in `#[instruction(...)]` attribute",
                            reorder_variant
                        ));
                    };

                    instructions.push(instruction);
                }
            }
            InstructionDefinitionItem::Ignore(ignore_items) => {
                for ignore_item in ignore_items {
                    if own_instructions.contains_key(ignore_item)
                        || all_known_instructions.contains_key(ignore_item)
                    {
                        included_instructions.insert(ignore_item.clone());
                    } else if let Some(known_enum) = state.get_known_enum_definition(ignore_item) {
                        for known_instruction in &known_enum.instructions {
                            included_instructions.insert(known_instruction.ident.clone());
                        }
                    } else {
                        state.add_pending_enum_definition(PendingEnumDefinition {
                            instruction_definition,
                            item_enum,
                        });
                        return Ok(None);
                    }
                }
            }
            InstructionDefinitionItem::Inherit(inherit_enums) => {
                for inherit_enum in inherit_enums {
                    let Some(known_enum) = state.get_known_enum_definition(inherit_enum) else {
                        return Err(anyhow::anyhow!(
                            "Unknown inherit enum `{}` in `#[instruction(...)]` attribute",
                            inherit_enum
                        ));
                    };

                    for known_instruction in &known_enum.instructions {
                        if !included_instructions.contains(&known_instruction.ident) {
                            included_instructions.insert(known_instruction.ident.clone());
                            instructions.push(Rc::clone(known_instruction));
                        }
                    }
                }
            }
        }
    }

    for own_instruction in &item_enum.variants {
        if !included_instructions.contains(&own_instruction.ident) {
            instructions.push(Rc::new(own_instruction.clone()));
        }
    }

    item_enum.variants = Punctuated::from_iter(
        instructions
            .into_iter()
            .map(|instruction| instruction.as_ref().clone()),
    );

    Ok(Some((item_enum, dependencies)))
}

/// Process remaining enums that were waiting for dependencies
pub(super) fn process_pending_enum_definitions(
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    let mut last_pending_enums_count = 0;
    loop {
        let pending_enums = state.take_pending_enum_definitions();

        if pending_enums.is_empty() {
            break;
        }

        if pending_enums.len() == last_pending_enums_count {
            return Err(anyhow::anyhow!(
                "Failed to process instruction macros, circular dependency detected, \
                pending_enums: {:?}",
                pending_enums
                    .iter()
                    .map(|pending_enum| &pending_enum.item_enum.ident)
                    .collect::<Vec<_>>()
            ));
        }
        last_pending_enums_count = pending_enums.len();

        for PendingEnumDefinition {
            instruction_definition,
            item_enum,
        } in pending_enums
        {
            if let Some((item_enum, dependencies)) =
                process_enum_definition_inherited(instruction_definition, item_enum, state)?
            {
                output_processed_enum_definition(item_enum, dependencies, out_dir, state)?;
            }
        }
    }

    Ok(())
}
