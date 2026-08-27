use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceBinding, SourceRole};

use crate::AnalysisSnapshot;
use crate::query::evidence::{
    EvidenceIntegrityError, SemanticCoverage, SemanticQuerySet, SemanticSetUnavailability,
};

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
    ///
    /// # Errors
    ///
    /// Returns an integrity error when the selected occurrence has no semantic domain.
    pub fn semantic_definition(
        &self,
        source: SourceId,
        offset: ByteOffset,
    ) -> Result<SemanticQuerySet<SemanticLocation>, EvidenceIntegrityError> {
        let Some(authority) = self.semantic_query()? else {
            return Ok(SemanticQuerySet::new(
                Box::new([]),
                SemanticCoverage::Unavailable(SemanticSetUnavailability::NoSemanticEvidence),
            ));
        };
        let index = authority.source_index();
        let Some(selection) = crate::query::semantic_selection_from(index, source, offset) else {
            return Ok(SemanticQuerySet::new(
                Box::new([]),
                SemanticCoverage::Complete,
            ));
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
        Ok(SemanticQuerySet::new(
            locations(
                selection.entity(),
                bindings
                    .iter()
                    .filter(|binding| binding.role() == preferred),
            ),
            SemanticCoverage::Complete,
        ))
    }

    /// Finds the authored implementation of the exact semantic occurrence at `offset`.
    ///
    /// # Errors
    ///
    /// Returns an integrity error when the selected occurrence has no semantic domain.
    pub fn semantic_implementation(
        &self,
        source: SourceId,
        offset: ByteOffset,
    ) -> Result<SemanticQuerySet<SemanticLocation>, EvidenceIntegrityError> {
        let Some(authority) = self.semantic_query()? else {
            return Ok(SemanticQuerySet::new(
                Box::new([]),
                SemanticCoverage::Unavailable(SemanticSetUnavailability::NoSemanticEvidence),
            ));
        };
        let index = authority.source_index();
        let Some(selection) = crate::query::semantic_selection_from(index, source, offset) else {
            return Ok(SemanticQuerySet::new(
                Box::new([]),
                SemanticCoverage::Complete,
            ));
        };
        let bindings = index.bindings_for(selection.entity());
        let preferred = if bindings
            .iter()
            .any(|binding| binding.role() == SourceRole::Implementation)
        {
            SourceRole::Implementation
        } else {
            SourceRole::Declaration
        };
        Ok(SemanticQuerySet::new(
            locations(
                selection.entity(),
                bindings
                    .iter()
                    .filter(|binding| binding.role() == preferred),
            ),
            SemanticCoverage::Complete,
        ))
    }

    /// Finds every reached occurrence of the exact semantic identity at `offset`.
    ///
    /// # Errors
    ///
    /// Returns an integrity error when a source occurrence names a semantic domain absent from the
    /// immutable evidence result.
    pub fn semantic_references(
        &self,
        source: SourceId,
        offset: ByteOffset,
        include_declarations: bool,
    ) -> Result<SemanticQuerySet<SemanticLocation>, EvidenceIntegrityError> {
        let Some(authority) = self.semantic_query()? else {
            return Ok(SemanticQuerySet::new(
                Box::new([]),
                SemanticCoverage::Unavailable(SemanticSetUnavailability::NoSemanticEvidence),
            ));
        };
        let Some(selection) =
            crate::query::semantic_selection_from(authority.source_index(), source, offset)
        else {
            return Ok(SemanticQuerySet::new(
                Box::new([]),
                SemanticCoverage::Complete,
            ));
        };
        let coverage = authority.typed_body_coverage()?;
        Ok(SemanticQuerySet::new(
            locations(
                selection.entity(),
                authority
                    .source_index()
                    .bindings_for(selection.entity())
                    .iter()
                    .filter(|binding| {
                        include_declarations || binding.role() == SourceRole::Reference
                    }),
            ),
            coverage,
        ))
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
