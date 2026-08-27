use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceBinding, SourceRole};

use crate::AnalysisSnapshot;
use crate::evidence::{
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
    #[must_use]
    pub fn semantic_definition(
        &self,
        source: SourceId,
        offset: ByteOffset,
    ) -> SemanticQuerySet<SemanticLocation> {
        let Some(selection) = self.semantic_selection(source, offset) else {
            return SemanticQuerySet::new(Box::new([]), authority_coverage(self));
        };
        let Some(index) = self
            .semantic_authority()
            .map(|authority| authority.source_index())
        else {
            return SemanticQuerySet::new(
                Box::new([]),
                SemanticCoverage::Unavailable(SemanticSetUnavailability::NoSemanticAuthority),
            );
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
        SemanticQuerySet::new(
            locations(
                selection.entity(),
                bindings
                    .iter()
                    .filter(|binding| binding.role() == preferred),
            ),
            SemanticCoverage::Complete,
        )
    }

    /// Finds the authored implementation of the exact semantic occurrence at `offset`.
    #[must_use]
    pub fn semantic_implementation(
        &self,
        source: SourceId,
        offset: ByteOffset,
    ) -> SemanticQuerySet<SemanticLocation> {
        let Some(selection) = self.semantic_selection(source, offset) else {
            return SemanticQuerySet::new(Box::new([]), authority_coverage(self));
        };
        let Some(index) = self
            .semantic_authority()
            .map(|authority| authority.source_index())
        else {
            return SemanticQuerySet::new(
                Box::new([]),
                SemanticCoverage::Unavailable(SemanticSetUnavailability::NoSemanticAuthority),
            );
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
        SemanticQuerySet::new(
            locations(
                selection.entity(),
                bindings
                    .iter()
                    .filter(|binding| binding.role() == preferred),
            ),
            SemanticCoverage::Complete,
        )
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
        let Some(selection) = self.semantic_selection(source, offset) else {
            return Ok(SemanticQuerySet::new(
                Box::new([]),
                authority_coverage(self),
            ));
        };
        let Some(authority) = self.semantic_authority() else {
            return Ok(SemanticQuerySet::new(
                Box::new([]),
                SemanticCoverage::Unavailable(SemanticSetUnavailability::NoSemanticAuthority),
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

fn authority_coverage(snapshot: &AnalysisSnapshot) -> SemanticCoverage {
    if snapshot.semantic_authority().is_some() {
        SemanticCoverage::Complete
    } else {
        SemanticCoverage::Unavailable(SemanticSetUnavailability::NoSemanticAuthority)
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
