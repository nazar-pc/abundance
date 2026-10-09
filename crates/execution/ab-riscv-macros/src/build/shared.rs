use crate::build::state::{KnownEnumDefinition, State};
use quote::ToTokens;
use std::collections::{HashSet, VecDeque};
use std::{iter, mem};
use syn::punctuated::Punctuated;
use syn::{Attribute, Ident, Meta, Token, WherePredicate, parse_quote};

pub(super) const INSTRUCTION_ATTRIBUTE: &str = "instruction";
pub(super) const INSTRUCTION_EXECUTION_ATTRIBUTE: &str = "instruction_execution";

/// Whether an attribute has a given name.
///
/// Only the last path segment is compared, so path-qualified attributes like
/// `#[ab_riscv_macros::instruction]` are recognized too.
pub(super) fn is_named_attribute(attribute: &Attribute, name: &str) -> bool {
    attribute
        .path()
        .segments
        .last()
        .is_some_and(|segment| segment.ident == name)
}

/// Arguments of an attribute, if there are any.
///
/// Empty parentheses like `#[instruction()]` are treated as no arguments, the same way attribute
/// macros see them.
pub(super) fn attribute_arguments(attribute: &Attribute) -> Option<String> {
    match &attribute.meta {
        Meta::Path(_) => None,
        Meta::List(meta_list) => {
            (!meta_list.tokens.is_empty()).then(|| meta_list.tokens.to_string())
        }
        Meta::NameValue(meta_name_value) => {
            Some(meta_name_value.value.to_token_stream().to_string())
        }
    }
}

pub(super) fn collect_all_dependencies<InitialDependencies>(
    state: &State,
    initial_dependencies: InitialDependencies,
) -> Result<Vec<(Ident, &KnownEnumDefinition)>, Ident>
where
    InitialDependencies: Iterator<Item = Ident>,
{
    let mut already_inserted = HashSet::new();
    let mut all_dependencies = Vec::new();
    let mut new_dependencies = VecDeque::from_iter(initial_dependencies);

    while let Some(dependency_enum_name) = new_dependencies.pop_front() {
        let Some(dependency_enum_definition) =
            state.get_known_enum_definition(&dependency_enum_name)
        else {
            return Err(dependency_enum_name);
        };

        if !already_inserted.insert(dependency_enum_name.clone()) {
            continue;
        }

        all_dependencies.push((dependency_enum_name, dependency_enum_definition));
        new_dependencies.extend(
            dependency_enum_definition
                .direct_dependencies
                .iter()
                .cloned(),
        );
    }

    Ok(all_dependencies)
}

/// Whether an instruction enum is implemented by an instruction set, with `is_present` checking
/// whether an individual instruction is present in it.
///
/// An enum is implemented when at least one of its own instructions is present (otherwise it was
/// ignored in one way or another) and the same is true for all enums it inherits. Enums without
/// own instructions (like `B`) are only implemented when all enums they inherit are.
pub(super) fn is_enum_implemented<IsPresent>(
    state: &State,
    enum_name: &Ident,
    is_present: IsPresent,
) -> bool
where
    IsPresent: Fn(&Ident) -> bool,
{
    collect_all_dependencies(state, iter::once(enum_name.clone())).is_ok_and(|all_enums| {
        all_enums.into_iter().all(|(_enum_name, enum_definition)| {
            enum_definition.own_instructions.is_empty()
                || enum_definition
                    .own_instructions
                    .iter()
                    .any(|instruction| is_present(&instruction.ident))
        })
    })
}

/// Strips `[const]` from `where` predicates.
///
/// Used both for a non-`const` implementation that inherits arms from `const` ones and for the
/// threaded implementation, which is never `const`.
pub(super) fn strip_const_where_predicates(predicates: &mut Punctuated<WherePredicate, Token![,]>) {
    for predicate in predicates {
        if let WherePredicate::Type(predicate_type) = predicate {
            // TODO: `BRCONST` is a hack that allows `syn` to parse unstable Rust syntax
            //  around const traits and such. It will change to a proper modifier once
            //  stabilized
            if predicate_type.bounds.first() == Some(&parse_quote! { BRCONST }) {
                predicate_type.bounds = mem::take(&mut predicate_type.bounds)
                    .into_iter()
                    .skip(1)
                    .collect();
            }
        }
    }
}
