use crate::build::state::{KnownEnumDefinition, State};
use std::collections::VecDeque;
use syn::Ident;

pub(super) fn collect_all_dependencies(
    state: &State,
    initial_dependencies: Vec<Ident>,
) -> Result<Vec<(Ident, &KnownEnumDefinition)>, Ident> {
    let mut all_dependencies = Vec::new();
    let mut new_dependencies = VecDeque::from(initial_dependencies);

    while let Some(dependency_enum_name) = new_dependencies.pop_front() {
        let Some(dependency_enum_definition) =
            state.get_known_enum_definition(&dependency_enum_name)
        else {
            return Err(dependency_enum_name);
        };

        all_dependencies.push((dependency_enum_name, dependency_enum_definition));
        new_dependencies.extend(dependency_enum_definition.dependencies.iter().cloned());
    }

    Ok(all_dependencies)
}
