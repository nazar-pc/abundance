use crate::build::shared::collect_all_dependencies;
use crate::build::state::{PendingEnumDefinition, State};
use ab_riscv_macros_common::code_utils::pre_process_rust_code;
use anyhow::Context;
use prettyplease::unparse;
use quote::{ToTokens, format_ident};
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::{env, fs, mem};
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    Error, Fields, FieldsNamed, Ident, ItemEnum, Meta, Token, Variant, bracketed, parse_file,
    parse_quote, parse_str, parse2,
};

const ORIGINAL_ENUM_DEFINITION_ENV_VAR_SUFFIX: &str = "__INSTRUCTION_ORIGINAL_ENUM_DEFINITION_PATH";
const ENUM_DEFINITION_ENV_VAR_SUFFIX: &str = "__INSTRUCTION_ENUM_DEFINITION_PATH";
const ENUM_DEPENDENCIES_ENV_VAR_SUFFIX: &str = "__INSTRUCTION_ENUM_DEPENDENCIES";
const ENUM_IGNORED_INSTRUCTIONS_ENV_VAR_SUFFIX: &str = "__INSTRUCTION_ENUM_IGNORED_INSTRUCTIONS";
const ENUM_DEPENDENCIES_FOR_ENABLEMENT_ENV_VAR_SUFFIX: &str =
    "__INSTRUCTION_ENUM_DEPENDENCIES_FOR_ENABLEMENT";

/// Attribute for `#[instruction]` macro used on an enum definition
#[derive(Debug, Default)]
struct InstructionDefinition {
    items: Punctuated<InstructionDefinitionItem, Token![,]>,
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
enum InstructionDefinitionItem {
    /// Specifies the order of instruction variants
    Reorder(Vec<Ident>),
    /// Specifies ignored instruction variants or whole enums
    Ignore(Vec<Ident>),
    /// Specifies the inherited instruction enums
    Inherit(Vec<Ident>),
    /// Makes instruction optional depending on the presence of instruction variants or whole enums
    If(Rc<[Ident]>),
}

impl InstructionDefinitionItem {
    fn parse(input: ParseStream<'_>) -> Result<Self, Error> {
        let key = input.call::<Ident>(Ident::parse_any)?;
        input.parse::<Token![=]>()?;

        let content;
        bracketed!(content in input);

        let values = content.parse_terminated(Ident::parse, Token![,])?;

        match key.to_string().as_str() {
            "reorder" => Ok(Self::Reorder(values.into_iter().collect())),
            "ignore" => Ok(Self::Ignore(values.into_iter().collect())),
            "inherit" => Ok(Self::Inherit(values.into_iter().collect())),
            "if" => Ok(Self::If(values.into_iter().collect())),
            _ => Err(Error::new_spanned(key, "unknown instruction attribute key")),
        }
    }
}

/// `#[instruction]` attribute on an enum variant
#[derive(Debug, Default)]
struct InstructionVariant {
    items: Punctuated<InstructionVariantItem, Token![,]>,
}

impl Parse for InstructionVariant {
    fn parse(input: ParseStream<'_>) -> Result<Self, Error> {
        Ok(Self {
            items: input.parse_terminated(InstructionVariantItem::parse, Token![,])?,
        })
    }
}

/// Attribute item for `#[instruction]` macro used on enum definition
#[derive(Debug)]
enum InstructionVariantItem {
    /// Makes instruction optional depending on the presence of instruction variants or whole enums
    If(Rc<[Ident]>),
}

impl InstructionVariantItem {
    fn parse(input: ParseStream<'_>) -> Result<Self, Error> {
        let key = input.call::<Ident>(Ident::parse_any)?;
        input.parse::<Token![=]>()?;

        let content;
        bracketed!(content in input);

        let values = content.parse_terminated(Ident::parse, Token![,])?;

        match key.to_string().as_str() {
            "if" => Ok(Self::If(values.into_iter().collect())),
            _ => Err(Error::new_spanned(key, "unknown instruction attribute key")),
        }
    }
}

#[derive(Debug)]
struct KnownInstruction {
    instruction: Rc<Variant>,
    source: Rc<Path>,
    /// Dependencies for enablement of this instruction, if there are multiple elements in a
    /// vector, any is sufficient to enable the instruction
    dependencies_for_enablement: HashSet<Rc<[Ident]>>,
}

struct ProcessedEnumDefinition {
    item_enum: ItemEnum,
    original_item_enum: ItemEnum,
    ignored_instructions: HashSet<Ident>,
    direct_dependencies: Rc<[Ident]>,
    dependencies_for_enablement: HashSet<Rc<[Ident]>>,
}

// TODO: Data structures for various `collect_*()` return types
#[expect(clippy::type_complexity, reason = "Internal function")]
pub(super) fn collect_enum_definitions_from_dependencies() -> impl Iterator<
    Item = anyhow::Result<(
        ItemEnum,
        ItemEnum,
        HashSet<Ident>,
        Rc<[Ident]>,
        HashSet<Rc<[Ident]>>,
        Rc<Path>,
    )>,
> {
    // Collect exported instruction enums from dependencies
    env::vars().filter_map(|(key, original_source)| {
        if !key.ends_with(ORIGINAL_ENUM_DEFINITION_ENV_VAR_SUFFIX) {
            return None;
        }

        let result = try {
            let definition_key = format!(
                "{}{ENUM_DEFINITION_ENV_VAR_SUFFIX}",
                &key[..key.len() - ORIGINAL_ENUM_DEFINITION_ENV_VAR_SUFFIX.len()]
            );
            let source = env::var(&definition_key).with_context(|| {
                format!(
                    "Failed to read environment variable `{definition_key}` that is expected to \
                    contain instruction enum definition"
                )
            })?;
            let source = Rc::from(Path::new(&source));
            let mut item_enum_contents = fs::read_to_string(&source).with_context(|| {
                format!(
                    "Failed to read Rust file `{}` that is expected to contain instruction enum \
                    definition",
                    source.display()
                )
            })?;
            pre_process_rust_code(&mut item_enum_contents);
            let item_enum = parse_str::<ItemEnum>(&item_enum_contents).with_context(|| {
                format!(
                    "Failed to parse Rust file `{}` that is expected to contain instruction enum \
                    definition",
                    source.display()
                )
            })?;

            let dependencies_key = format!(
                "{}{ENUM_DEPENDENCIES_ENV_VAR_SUFFIX}",
                &key[..key.len() - ORIGINAL_ENUM_DEFINITION_ENV_VAR_SUFFIX.len()]
            );
            let dependencies_string = env::var(&dependencies_key).with_context(|| {
                format!(
                    "Failed to read environment variable `{dependencies_key}` that is expected to \
                    contain instruction enum dependencies"
                )
            })?;
            let dependencies = dependencies_string
                .split(',')
                .filter(|dependency| !dependency.is_empty())
                .map(|dependency| format_ident!("{dependency}"))
                .collect::<Rc<[_]>>();
            let dependencies_for_enablement_key = format!(
                "{}{ENUM_DEPENDENCIES_FOR_ENABLEMENT_ENV_VAR_SUFFIX}",
                &key[..key.len() - ORIGINAL_ENUM_DEFINITION_ENV_VAR_SUFFIX.len()]
            );
            let dependencies_for_enablement_string = env::var(&dependencies_for_enablement_key)
                .with_context(|| {
                    format!(
                        "Failed to read environment variable `{dependencies_for_enablement_key}` \
                        that is expected to contain instruction enum dependencies_for_enablement"
                    )
                })?;
            let dependencies_for_enablement = dependencies_for_enablement_string
                .split(',')
                .map(|dependencies| {
                    dependencies
                        .split('+')
                        .filter(|dependency| !dependency.is_empty())
                        .map(|dependency| format_ident!("{dependency}"))
                        .collect::<Rc<[_]>>()
                })
                .filter(|dependencies| !dependencies.is_empty())
                .collect::<HashSet<_>>();

            let ignored_instructions_key = format!(
                "{}{ENUM_IGNORED_INSTRUCTIONS_ENV_VAR_SUFFIX}",
                &key[..key.len() - ORIGINAL_ENUM_DEFINITION_ENV_VAR_SUFFIX.len()]
            );
            let ignored_instructions_string =
                env::var(&ignored_instructions_key).with_context(|| {
                    format!(
                        "Failed to read environment variable `{ignored_instructions_key}` that is \
                    expected to contain ignored instructions"
                    )
                })?;
            let ignored_instructions = ignored_instructions_string
                .split(',')
                .filter(|dependency| !dependency.is_empty())
                .map(|dependency| format_ident!("{dependency}"))
                .collect();

            let mut original_item_enum_contents = fs::read_to_string(&original_source)
                .with_context(|| {
                    format!(
                        "Failed to read Rust file `{original_source}` that is expected to contain \
                        instruction enum definition"
                    )
                })?;
            pre_process_rust_code(&mut original_item_enum_contents);
            let original_item_enum = parse_str::<ItemEnum>(&original_item_enum_contents)
                .with_context(|| {
                    format!(
                        "Failed to parse Rust file `{original_source}` that is expected to contain \
                        instruction enum definition",
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
                "cargo::metadata={}{ORIGINAL_ENUM_DEFINITION_ENV_VAR_SUFFIX}={original_source}",
                original_item_enum.ident
            );
            println!(
                "cargo::metadata={}{ENUM_IGNORED_INSTRUCTIONS_ENV_VAR_SUFFIX}=\
                {ignored_instructions_string}",
                original_item_enum.ident
            );
            println!(
                "cargo::metadata={}{ENUM_DEPENDENCIES_ENV_VAR_SUFFIX}={dependencies_string}",
                original_item_enum.ident
            );
            println!(
                "cargo::metadata={}{ENUM_DEPENDENCIES_FOR_ENABLEMENT_ENV_VAR_SUFFIX}=\
                {dependencies_for_enablement_string}",
                item_enum.ident
            );

            (
                original_item_enum,
                item_enum,
                ignored_instructions,
                dependencies,
                dependencies_for_enablement,
                source,
            )
        };

        Some(result)
    })
}

fn output_processed_enum_definition(
    processed_enum_definition: ProcessedEnumDefinition,
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    let ProcessedEnumDefinition {
        mut item_enum,
        mut original_item_enum,
        ignored_instructions,
        direct_dependencies,
        dependencies_for_enablement,
    } = processed_enum_definition;

    let enum_name = original_item_enum.ident.clone();

    let enum_file_path = {
        for variant in &mut item_enum.variants {
            let mut sorted_fields: FieldsNamed = parse_quote! {{
                #[
                    doc = "First register operand (placeholder, instruction doesn't have it, \
                    should be set to Reg::ZERO)"
                ]
                #[doc(hidden)]
                rs1: Reg,
                #[
                    doc = "Second register operand (placeholder, instruction doesn't have it, \
                    should be set to Reg::ZERO)"
                ]
                #[doc(hidden)]
                rs2: Reg,
            }};
            match &mut variant.fields {
                Fields::Named(fields) => {
                    for field in mem::take(&mut fields.named) {
                        if let Some(ident) = &field.ident {
                            if ident == "rs1" {
                                sorted_fields.named[0] = field;
                                continue;
                            } else if ident == "rs2" {
                                sorted_fields.named[1] = field;
                                continue;
                            }
                        }
                        sorted_fields.named.push(field);
                    }
                }
                Fields::Unnamed(_) => {
                    return Err(anyhow::anyhow!(
                        "Enum variants must use named fields: {}::{}(..)",
                        item_enum.ident,
                        variant.ident
                    ));
                }
                Fields::Unit => {
                    // Nothing to do here
                }
            }

            variant.fields = Fields::Named(sorted_fields);
        }

        let enum_file_path = out_dir.join(format!("{enum_name}_definition.rs"));
        let code = item_enum.to_token_stream().to_string();
        // Format
        let code = unparse(&parse_file(&code).expect("Generated code is valid; qed"));
        // Normalize source
        item_enum = parse_str(&code).expect("Generated code is valid; qed");

        // Avoid extra file truncation/override if it didn't change
        if fs::read_to_string(&enum_file_path).ok().as_ref() != Some(&code) {
            fs::write(&enum_file_path, code).with_context(|| {
                format!("Failed to write generated Rust file with instruction enum `{enum_name}`")
            })?;
        }
        println!(
            "cargo::metadata={enum_name}{ENUM_DEFINITION_ENV_VAR_SUFFIX}={}",
            enum_file_path.display()
        );

        enum_file_path
    };
    {
        let original_enum_file_path = out_dir.join(format!("{enum_name}_original_definition.rs"));
        let code = original_item_enum.to_token_stream().to_string();
        // Format
        let code = unparse(&parse_file(&code).expect("Original code is valid; qed"));
        // Normalize source
        original_item_enum = parse_str(&code).expect("Original code is valid; qed");

        // Avoid extra file truncation/override if it didn't change
        if fs::read_to_string(&original_enum_file_path).ok().as_ref() != Some(&code) {
            fs::write(&original_enum_file_path, code).with_context(|| {
                format!("Failed to write original Rust file with instruction enum `{enum_name}`")
            })?;
        }
        println!(
            "cargo::metadata={enum_name}{ORIGINAL_ENUM_DEFINITION_ENV_VAR_SUFFIX}={}",
            original_enum_file_path.display()
        );
        println!(
            "cargo::metadata={enum_name}{ENUM_IGNORED_INSTRUCTIONS_ENV_VAR_SUFFIX}={}",
            ignored_instructions
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
        println!(
            "cargo::metadata={enum_name}{ENUM_DEPENDENCIES_ENV_VAR_SUFFIX}={}",
            direct_dependencies
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
    }

    println!(
        "cargo::metadata={enum_name}{ENUM_DEPENDENCIES_FOR_ENABLEMENT_ENV_VAR_SUFFIX}={}",
        dependencies_for_enablement
            .iter()
            .map(|idents| {
                idents
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("+")
            })
            .collect::<Vec<_>>()
            .join(","),
    );

    state.insert_known_enum_definition(
        original_item_enum,
        item_enum,
        ignored_instructions,
        direct_dependencies,
        dependencies_for_enablement,
        Rc::from(enum_file_path),
    )
}

/// Identify enums with an `#[instruction]` attribute and generate Rust files with the enum
/// definition after processing attribute contents and propagate the environment variable with the
/// path do dependent crates
pub(super) fn process_enum_definition(
    mut item_enum: ItemEnum,
    out_dir: &Path,
    state: &mut State,
) -> anyhow::Result<()> {
    let original_item_enum = item_enum.clone();
    let Some(attribute_index) = item_enum
        .attrs
        .iter()
        .enumerate()
        .find_map(|(index, attr)| attr.path().is_ident("instruction").then_some(index))
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

    for removed_attribute in removed_attributes.collect::<Vec<_>>() {
        if removed_attribute.path().is_ident("derive") {
            return Err(anyhow::anyhow!(
                "All `#[derive(...)]` attributes must be after `#[instruction(...)]` attribute, found: {}",
                removed_attribute.to_token_stream()
            ));
        } else if removed_attribute.path().is_ident("doc") {
            // Restore documentation attributes
            item_enum.attrs.push(removed_attribute);
        }
    }

    let has_repr_align = item_enum.attrs.iter().any(|attr| {
        if !attr.path().is_ident("repr") {
            return false;
        }
        // Parse the repr(...) token stream and look for align(...)
        attr.parse_args_with(|input: ParseStream<'_>| {
            let nested = input.parse_terminated(Meta::parse, Token![,])?;
            Ok(nested.iter().any(|meta| meta.path().is_ident("align")))
        })
        .unwrap_or_default()
    });

    if !has_repr_align {
        // Alignment improves performance significantly during execution
        item_enum.attrs.push(parse_quote! { #[repr(align(4))] });
    }

    let has_repr_c_ux = item_enum.attrs.iter().any(|attr| {
        if !attr.path().is_ident("repr") {
            return false;
        }
        // Parse the repr(...) token stream and look for align(...)
        attr.parse_args_with(|input: ParseStream<'_>| {
            let nested = input.parse_terminated(Meta::parse, Token![,])?;
            Ok(nested.iter().any(|meta| {
                meta.path().is_ident("C")
                    || meta.path().is_ident("u8")
                    || meta.path().is_ident("u16")
            }))
        })
        .unwrap_or_default()
    });

    if !has_repr_c_ux {
        // `u16` is a bit larger than necessary in the simplest case, but it is also consistently a
        // bit faster for some reason
        item_enum.attrs.push(parse_quote! { #[repr(u16)] });
    }

    let instruction_definition = match attribute.meta {
        Meta::Path(_) => InstructionDefinition::default(),
        Meta::List(meta_list) => parse2::<InstructionDefinition>(meta_list.tokens)
            .context("Failed to parse `#[instruction(...)]` attribute")?,
        Meta::NameValue(meta_name_value) => {
            return Err(anyhow::anyhow!(
                "Unexpected `#[instruction = {}]` attribute",
                Meta::NameValue(meta_name_value).to_token_stream()
            ));
        }
    };

    let Some(processed_enum_definition) = process_enum_definition_inherited(
        instruction_definition,
        original_item_enum,
        item_enum,
        state,
    )?
    else {
        return Ok(());
    };

    output_processed_enum_definition(processed_enum_definition, out_dir, state)
}

enum GetAllInheritedInstructionsError {
    MissingDependency(Ident),
    Error(anyhow::Error),
}

impl From<Ident> for GetAllInheritedInstructionsError {
    fn from(value: Ident) -> Self {
        Self::MissingDependency(value)
    }
}

impl From<anyhow::Error> for GetAllInheritedInstructionsError {
    fn from(value: anyhow::Error) -> Self {
        Self::Error(value)
    }
}

/// Collects all inherited instructions from dependencies recursively alongside their dependencies
/// for enablement, while taking into consideration ignored instructions
fn get_all_inherited_instructions(
    ignored_instructions: &HashSet<Ident>,
    direct_dependencies: &[Ident],
    dependencies_for_enablement: &HashSet<Rc<[Ident]>>,
    state: &mut State,
) -> anyhow::Result<HashMap<Ident, KnownInstruction>, GetAllInheritedInstructionsError> {
    let mut all_inherited_instructions = HashMap::<Ident, KnownInstruction>::new();

    for dependency_enum_name in direct_dependencies {
        let Some(dependency_enum_definition) =
            state.get_known_enum_definition(dependency_enum_name)
        else {
            return Err(GetAllInheritedInstructionsError::MissingDependency(
                dependency_enum_name.clone(),
            ));
        };

        for instruction in &dependency_enum_definition.own_instructions {
            let instruction_variant = instruction
                .attrs
                .iter()
                .find_map(|attribute| {
                    if !attribute.path().is_ident("instruction") {
                        return None;
                    }

                    Some(match attribute.meta.clone() {
                        Meta::Path(_) => {
                            return None;
                        }
                        Meta::List(meta_list) => parse2::<InstructionVariant>(meta_list.tokens)
                            .with_context(|| {
                                format!(
                                    "Failed to parse `#[instruction(...)]` attribute on {} variant",
                                    instruction.ident
                                )
                            }),
                        Meta::NameValue(meta_name_value) => Err(anyhow::anyhow!(
                            "Unexpected `#[instruction = {}]` attribute on {} variant",
                            Meta::NameValue(meta_name_value).to_token_stream(),
                            instruction.ident,
                        )),
                    })
                })
                .transpose()?
                .unwrap_or_default();
            let instruction_dependencies_for_enablement = instruction_variant
                .items
                .iter()
                .map(|item| match item {
                    InstructionVariantItem::If(if_dependency) => Rc::clone(if_dependency),
                })
                .collect();

            match all_inherited_instructions.entry(instruction.ident.clone()) {
                Entry::Occupied(mut entry) => {
                    let existing_known_instruction = entry.get_mut();
                    if &existing_known_instruction.instruction != instruction {
                        return Err(GetAllInheritedInstructionsError::Error(anyhow::anyhow!(
                            "Duplicate instruction definition that is not identical to an existing \
                            one: `{}` ({}) != `{}` ({})",
                            existing_known_instruction.instruction.to_token_stream(),
                            existing_known_instruction.source.display(),
                            instruction.to_token_stream(),
                            dependency_enum_definition.source.display(),
                        )));
                    }

                    // Combine dependencies that come from different sources, so any of them can
                    // satisfy the requirements in the end
                    existing_known_instruction
                        .dependencies_for_enablement
                        .extend(instruction_dependencies_for_enablement);
                }
                Entry::Vacant(entry) => {
                    entry.insert(KnownInstruction {
                        instruction: Rc::clone(instruction),
                        source: Rc::clone(&dependency_enum_definition.source),
                        dependencies_for_enablement: instruction_dependencies_for_enablement,
                    });
                }
            }
        }

        let dependency_ignored_instructions =
            Rc::clone(&dependency_enum_definition.ignored_instructions);
        let dependency_direct_dependencies =
            Rc::clone(&dependency_enum_definition.direct_dependencies);
        let dependency_dependencies_for_enablement = dependency_enum_definition
            .dependencies_for_enablement
            .clone();
        let source = Rc::clone(&dependency_enum_definition.source);

        for (ident, known_instruction) in get_all_inherited_instructions(
            &dependency_ignored_instructions,
            &dependency_direct_dependencies,
            &dependency_dependencies_for_enablement,
            state,
        )? {
            match all_inherited_instructions.entry(ident.clone()) {
                Entry::Occupied(mut entry) => {
                    let existing_known_instruction = entry.get_mut();
                    if existing_known_instruction.instruction != known_instruction.instruction {
                        return Err(GetAllInheritedInstructionsError::Error(anyhow::anyhow!(
                            "Duplicate instruction definition that is not identical to an existing \
                            one: `{}` ({}) != `{}` ({})",
                            existing_known_instruction.instruction.to_token_stream(),
                            existing_known_instruction.source.display(),
                            known_instruction.instruction.to_token_stream(),
                            source.display(),
                        )));
                    }

                    // Combine dependencies that come from different sources, so any of them can
                    // satisfy the requirements in the end
                    existing_known_instruction
                        .dependencies_for_enablement
                        .extend(
                            known_instruction
                                .dependencies_for_enablement
                                .iter()
                                .cloned(),
                        );
                }
                Entry::Vacant(entry) => {
                    entry.insert(known_instruction);
                }
            }
        }
    }

    if !ignored_instructions.is_empty() {
        all_inherited_instructions.retain(|instruction_name, _inherited_instruction| {
            !ignored_instructions.contains(instruction_name)
        });
    }

    // Extend dependencies of instructions
    if !dependencies_for_enablement.is_empty() {
        let dependencies_for_enablement_cache = &RefCell::new(HashMap::new());

        for inherited_instruction in all_inherited_instructions.values_mut() {
            if inherited_instruction.dependencies_for_enablement.is_empty() {
                inherited_instruction
                    .dependencies_for_enablement
                    .clone_from(dependencies_for_enablement);
            } else {
                let instruction_dependencies_for_enablement =
                    &mem::take(&mut inherited_instruction.dependencies_for_enablement);
                inherited_instruction.dependencies_for_enablement = dependencies_for_enablement
                    .iter()
                    .flat_map(move |dependencies_for_enablement| {
                        instruction_dependencies_for_enablement.iter().map(
                            move |instruction_dependencies_for_enablement| {
                                // Use cache to share allocations
                                let cache_key = (
                                    Rc::clone(instruction_dependencies_for_enablement),
                                    dependencies_for_enablement,
                                );

                                Rc::clone(
                                    dependencies_for_enablement_cache
                                        .borrow_mut()
                                        .entry(cache_key)
                                        .or_insert_with(|| {
                                            // Combine instruction's and enum's dependencies
                                            instruction_dependencies_for_enablement
                                                .iter()
                                                .chain(dependencies_for_enablement.as_ref())
                                                .cloned()
                                                .collect::<Rc<[_]>>()
                                        }),
                                )
                            },
                        )
                    })
                    .collect();
            }
        }
    }

    Ok(all_inherited_instructions)
}

/// Resolve dependencies and clean up unsatisfied dependencies from the map produced by
/// [`get_all_inherited_instructions()`]
fn process_dependencies_for_enablement(
    all_inherited_instructions: &mut HashMap<Ident, KnownInstruction>,
    state: &State,
) {
    let mut last_fully_available_instructions = usize::MAX;
    let mut fully_available_instructions = HashSet::with_capacity(all_inherited_instructions.len());
    let mut enum_dependencies_for_enablement_cache = HashMap::new();

    while last_fully_available_instructions != fully_available_instructions.len() {
        last_fully_available_instructions = fully_available_instructions.len();
        enum_dependencies_for_enablement_cache.clear();

        for (instruction_name, inherited_instruction) in all_inherited_instructions.iter() {
            if fully_available_instructions.contains(instruction_name) {
                continue;
            }

            if inherited_instruction.dependencies_for_enablement.is_empty() {
                // No dependencies
                fully_available_instructions.insert(instruction_name.clone());
                continue;
            }

            if inherited_instruction
                .dependencies_for_enablement
                .iter()
                .any(|instruction_dependencies_for_enablement| {
                    instruction_dependencies_for_enablement.iter().all(
                        |instruction_dependency_for_enablement| {
                            if fully_available_instructions
                                .contains(instruction_dependency_for_enablement)
                            {
                                // Depends on another fully available instruction variant
                                return true;
                            }

                            // Potentially depends on the whole enum, in which case all its variants
                            // must be available
                            *enum_dependencies_for_enablement_cache
                                .entry(instruction_dependency_for_enablement)
                                .or_insert_with(|| {
                                    state
                                        .get_known_enum_definition(
                                            instruction_dependency_for_enablement,
                                        )
                                        .is_some_and(|enum_definition| {
                                            enum_definition.instructions.iter().all(|variant| {
                                                fully_available_instructions
                                                    .contains(&variant.ident)
                                            })
                                        })
                                })
                        },
                    )
                })
            {
                fully_available_instructions.insert(instruction_name.clone());
            }
        }
    }

    all_inherited_instructions.retain(|instruction_name, _inherited_instruction| {
        fully_available_instructions.contains(instruction_name)
    });
}

fn process_enum_definition_inherited(
    instruction_definition: InstructionDefinition,
    original_item_enum: ItemEnum,
    mut item_enum: ItemEnum,
    state: &mut State,
) -> anyhow::Result<Option<ProcessedEnumDefinition>> {
    let enum_name = &original_item_enum.ident;

    let mut all_reordered_instructions = HashSet::new();
    let mut direct_dependencies = Vec::new();
    let mut dependencies_for_enablement = HashSet::new();

    for item in &instruction_definition.items {
        match item {
            InstructionDefinitionItem::Reorder(reorder_variants) => {
                // Collect reordered instructions to make sure they are later placed at the correct
                // location and not ignored
                all_reordered_instructions.extend(reorder_variants);
            }
            InstructionDefinitionItem::Ignore(_) => {
                // Ignore for now
            }
            InstructionDefinitionItem::Inherit(inherit_enums) => {
                direct_dependencies.extend(inherit_enums.iter().cloned());
            }
            InstructionDefinitionItem::If(if_dependency) => {
                dependencies_for_enablement.insert(Rc::clone(if_dependency));
            }
        }
    }

    let direct_dependencies = direct_dependencies.into_iter().collect::<Rc<[_]>>();
    let dependencies_for_enablement = dependencies_for_enablement.into_iter().collect();

    let mut all_inherited_instructions = match get_all_inherited_instructions(
        &HashSet::new(),
        &direct_dependencies,
        &HashSet::new(),
        state,
    ) {
        Ok(all_inherited_instructions) => all_inherited_instructions,
        Err(GetAllInheritedInstructionsError::MissingDependency(dependency_enum_name)) => {
            eprintln!("{enum_name} definition is waiting on {dependency_enum_name} definition");
            state.add_pending_enum_definition(PendingEnumDefinition { original_item_enum });
            return Ok(None);
        }
        Err(GetAllInheritedInstructionsError::Error(error)) => {
            return Err(error);
        }
    };
    process_dependencies_for_enablement(&mut all_inherited_instructions, state);

    let mut own_instructions = item_enum
        .variants
        .iter()
        .map(|variant| (variant.ident.clone(), Rc::new(variant.clone())))
        .collect::<HashMap<_, _>>();

    let mut processed_instructions = HashSet::new();
    let mut ignored_instructions = HashSet::new();
    let mut instructions = Vec::new();

    for item in &instruction_definition.items {
        match item {
            InstructionDefinitionItem::Reorder(reorder_variants) => {
                for reorder_variant in reorder_variants {
                    if processed_instructions.contains(reorder_variant) {
                        return Err(anyhow::anyhow!(
                            "Instruction `{reorder_variant}` in `#[instruction(...)]`'s `reorder` \
                            attribute is already included earlier"
                        ));
                    }
                    processed_instructions.insert(reorder_variant.clone());

                    let instruction =
                        if let Some(instruction) = own_instructions.remove(reorder_variant) {
                            instruction
                        } else if let Some(instruction) =
                            all_inherited_instructions.get(reorder_variant)
                        {
                            Rc::clone(&instruction.instruction)
                        } else {
                            return Err(anyhow::anyhow!(
                                "Unknown reorder instruction `{reorder_variant}` in \
                                `#[instruction(...)]` attribute"
                            ));
                        };

                    instructions.push(instruction);
                }
            }
            InstructionDefinitionItem::Ignore(ignore_items) => {
                for ignore_item in ignore_items {
                    if own_instructions.contains_key(ignore_item)
                        || all_inherited_instructions.contains_key(ignore_item)
                    {
                        if !all_reordered_instructions.contains(ignore_item) {
                            processed_instructions.insert(ignore_item.clone());
                            ignored_instructions.insert(ignore_item.clone());
                        }
                    } else {
                        let Some(known_enum) = state.get_known_enum_definition(ignore_item) else {
                            eprintln!(
                                "{enum_name} definition is waiting on {ignore_item} ignore item"
                            );
                            state.add_pending_enum_definition(PendingEnumDefinition {
                                original_item_enum,
                            });
                            return Ok(None);
                        };

                        // All instructions, including recursive dependencies
                        let all_instructions = known_enum.instructions.iter().chain(
                            collect_all_dependencies(
                                state,
                                known_enum.direct_dependencies.iter().cloned(),
                            )
                            .expect(
                                "Available parent definition means all dependencies are already \
                                resolved; qed",
                            )
                            .into_iter()
                            .flat_map(|(_, known_enum)| known_enum.instructions.iter()),
                        );

                        for known_instruction in all_instructions {
                            if !all_reordered_instructions.contains(&known_instruction.ident) {
                                processed_instructions.insert(known_instruction.ident.clone());
                                ignored_instructions.insert(known_instruction.ident.clone());
                            }
                        }
                    }
                }
            }
            InstructionDefinitionItem::Inherit(inherit_enums) => {
                for inherit_enum in inherit_enums {
                    let Some(known_enum) = state.get_known_enum_definition(inherit_enum) else {
                        eprintln!(
                            "{enum_name} definition is waiting on {inherit_enum} inherit enum"
                        );
                        state.add_pending_enum_definition(PendingEnumDefinition {
                            original_item_enum,
                        });
                        return Ok(None);
                    };

                    // All instructions, including recursive dependencies
                    let all_instructions = known_enum.instructions.iter().chain(
                        collect_all_dependencies(
                            state,
                            known_enum.direct_dependencies.iter().cloned(),
                        )
                        .expect(
                            "Available parent definition means all dependencies are already \
                            resolved; qed",
                        )
                        .into_iter()
                        .flat_map(|(_, known_enum)| known_enum.instructions.iter()),
                    );

                    for known_instruction in all_instructions {
                        if !(all_reordered_instructions.contains(&known_instruction.ident)
                            || processed_instructions.contains(&known_instruction.ident))
                        {
                            processed_instructions.insert(known_instruction.ident.clone());
                            if all_inherited_instructions.contains_key(&known_instruction.ident) {
                                instructions.push(Rc::clone(known_instruction));
                            }
                        }
                    }
                }
            }
            InstructionDefinitionItem::If(_) => {
                // Already processed above
            }
        }
    }

    for own_instruction in &item_enum.variants {
        if !processed_instructions.contains(&own_instruction.ident) {
            instructions.push(Rc::new(own_instruction.clone()));
        }
    }

    item_enum.variants = Punctuated::from_iter(instructions.into_iter().map(|instruction| {
        let mut instruction = instruction.as_ref().clone();
        instruction
            .attrs
            .retain(|attribute| !attribute.path().is_ident("instruction"));
        instruction
    }));

    Ok(Some(ProcessedEnumDefinition {
        item_enum,
        original_item_enum,
        ignored_instructions,
        direct_dependencies,
        dependencies_for_enablement,
    }))
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
                    .map(|pending_enum| &pending_enum.original_item_enum.ident)
                    .collect::<Vec<_>>()
            ));
        }
        last_pending_enums_count = pending_enums.len();

        for PendingEnumDefinition { original_item_enum } in pending_enums {
            process_enum_definition(original_item_enum, out_dir, state)?
        }
    }

    Ok(())
}
