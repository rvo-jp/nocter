use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin, SourceRole};

/// Selects the authored definition origin used to present a semantic rule failure.
///
/// A public contract is preferred over its separate implementation regardless of index ordering.
/// This projection occurs only after semantic rule selection and cannot choose semantic identity.
pub(crate) fn declaration_origin(
    source_index: &SourceIndex,
    entity: SemanticEntity,
) -> Option<SourceOrigin> {
    [SourceRole::Declaration, SourceRole::Implementation]
        .into_iter()
        .find_map(|role| {
            source_index
                .bindings_for(entity)
                .iter()
                .find(|binding| binding.role() == role)
                .map(|binding| binding.origin())
        })
}
