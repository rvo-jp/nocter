use crate::{SemanticEntity, SourceIndex, SourceOrigin, SourceRole};

/// Read-only projection capability for attaching semantic failures to authored source.
///
/// This capability deliberately exposes no binding iteration, reverse lookup, or visible-name
/// state. A semantic stage may use it to present a decision, but cannot use source projection to
/// make that decision.
#[derive(Clone, Copy, Debug)]
pub struct DiagnosticOrigins<'index> {
    index: &'index SourceIndex,
}

impl<'index> DiagnosticOrigins<'index> {
    pub(crate) const fn new(index: &'index SourceIndex) -> Self {
        Self { index }
    }

    /// Selects the authored definition origin used to present a semantic rule failure.
    ///
    /// A public contract is preferred over its separate implementation regardless of index order.
    #[must_use]
    pub fn declaration(self, entity: SemanticEntity) -> Option<SourceOrigin> {
        [SourceRole::Declaration, SourceRole::Implementation]
            .into_iter()
            .find_map(|role| {
                self.index
                    .bindings_for(entity)
                    .iter()
                    .find(|binding| binding.role() == role)
                    .map(|binding| binding.origin())
            })
    }
}
