use crate::build::state::{KnownEnumDefinition, State};
use quote::ToTokens;
use std::collections::{HashSet, VecDeque};
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

/// Strips `[const]` from `where` predicates, dropping `[const] Destruct` ones altogether.
///
/// Used both for a non-`const` implementation that inherits arms from `const` ones and for the
/// threaded implementation, which is never `const`.
pub(super) fn strip_const_where_predicates<Predicates>(
    predicates: Predicates,
) -> Punctuated<WherePredicate, Token![,]>
where
    Predicates: IntoIterator<Item = WherePredicate>,
{
    predicates
        .into_iter()
        .filter_map(|mut predicate| match &mut predicate {
            WherePredicate::Type(predicate_type) => {
                // TODO: Remove `Destruct` hack once stabilized, removing it from non-const
                //  implementations improves downstream user experience:
                //  https://github.com/rust-lang/rust/issues/133214
                if predicate_type.bounds.to_token_stream().to_string() == "BRCONST + Destruct" {
                    return None;
                }

                // TODO: `BRCONST` is a hack that allows `syn` to parse unstable Rust syntax
                //  around const traits and such. It will change to a proper modifier once
                //  stabilized
                if predicate_type.bounds.first() == Some(&parse_quote! { BRCONST }) {
                    predicate_type.bounds =
                        predicate_type.bounds.clone().into_iter().skip(1).collect();
                }

                Some(predicate)
            }
            _ => Some(predicate),
        })
        .collect()
}
