use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceBinding, SourceRole};

use crate::AnalysisSnapshot;

/// One source-owned semantic navigation target before URI and coordinate projection.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticLocation {
    source: SourceId,
    range: TextRange,
}

impl SemanticLocation {
    #[must_use]
    pub const fn source(self) -> SourceId {
        self.source
    }

    #[must_use]
    pub const fn range(self) -> TextRange {
        self.range
    }
}

impl AnalysisSnapshot {
    /// Finds the authored definition of the exact semantic occurrence at `offset`.
    #[must_use]
    pub fn semantic_definition(
        &self,
        source: SourceId,
        offset: ByteOffset,
    ) -> Box<[SemanticLocation]> {
        let Some(selection) = self.semantic_selection(source, offset) else {
            return Box::new([]);
        };
        let Some(index) = self.source_index() else {
            return Box::new([]);
        };
        let bindings = index.bindings_for(selection.entity());
        let preferred = if bindings
            .iter()
            .any(|binding| binding.role() == SourceRole::Declaration)
        {
            SourceRole::Declaration
        } else {
            SourceRole::Implementation
        };
        locations(
            selection.entity(),
            bindings
                .iter()
                .filter(|binding| binding.role() == preferred),
        )
    }

    /// Finds every reached occurrence of the exact semantic identity at `offset`.
    #[must_use]
    pub fn semantic_references(
        &self,
        source: SourceId,
        offset: ByteOffset,
        include_declarations: bool,
    ) -> Box<[SemanticLocation]> {
        let Some(selection) = self.semantic_selection(source, offset) else {
            return Box::new([]);
        };
        let Some(index) = self.source_index() else {
            return Box::new([]);
        };
        locations(
            selection.entity(),
            index
                .bindings_for(selection.entity())
                .iter()
                .filter(|binding| include_declarations || binding.role() == SourceRole::Reference),
        )
    }
}

fn locations<'a>(
    entity: SemanticEntity,
    bindings: impl Iterator<Item = &'a SourceBinding>,
) -> Box<[SemanticLocation]> {
    let mut locations = bindings
        .map(|binding| SemanticLocation {
            source: binding.origin().source(),
            range: navigation_range(entity, binding),
        })
        .collect::<Vec<_>>();
    locations.sort_unstable();
    locations.dedup();
    locations.into_boxed_slice()
}

fn navigation_range(entity: SemanticEntity, binding: &SourceBinding) -> TextRange {
    let range = binding.origin().span().range();
    if matches!(entity, SemanticEntity::Module(_)) && binding.role() != SourceRole::Reference {
        TextRange::empty(ByteOffset::new(0))
    } else {
        range
    }
}
