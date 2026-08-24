use crate::build::state::{KnownEnumDefinition, State};
use std::collections::{HashSet, VecDeque};
use std::mem;
use syn::punctuated::Punctuated;
use syn::{Ident, Token, WherePredicate, parse_quote};

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
