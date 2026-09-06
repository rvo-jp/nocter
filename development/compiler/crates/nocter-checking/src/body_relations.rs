use std::collections::HashMap;

use nocter_declarations::DeclarationGraph;
use nocter_model::{Arena, ArenaBuilder, BodyId, BodyNodeId};
use nocter_source_index::SourceOrigin;

use crate::{BodyCheckInternalError, BodySource, BodySourceCatalog, CheckedBody};

/// One checked body's complete input to program-wide semantic relation analysis.
///
/// The catalog owns the `BodyId` association. Individual analyses cannot pair a checked body with
/// a sibling's source or origin map, and do not need to rediscover the association by scanning a
/// caller-assembled slice.
pub(crate) struct BodyRelationInput<'program, 'syntax> {
    source: BodySource<'syntax>,
    body: &'program CheckedBody,
    origins: &'program HashMap<BodyNodeId, SourceOrigin>,
}

impl<'program, 'syntax> BodyRelationInput<'program, 'syntax> {
    #[must_use]
    pub(crate) const fn source(&self) -> BodySource<'syntax> {
        self.source
    }

    #[must_use]
    pub(crate) const fn body(&self) -> &'program CheckedBody {
        self.body
    }

    #[must_use]
    pub(crate) const fn origins(&self) -> &'program HashMap<BodyNodeId, SourceOrigin> {
        self.origins
    }
}

/// Canonical, exact-coverage body inputs shared by all program-wide relation analyses.
///
/// Construction proves the checked-body set covers the declaration graph exactly once and pairs
/// every body with its source projection once. Consumers receive O(1) `BodyId` lookup and cannot
/// express independently ordered provenance, effect, or loan input sets.
pub(crate) struct BodyRelationCatalog<'program, 'syntax> {
    inputs: Arena<BodyId, BodyRelationInput<'program, 'syntax>>,
}

impl<'program, 'syntax> BodyRelationCatalog<'program, 'syntax> {
    pub(crate) fn new(
        graph: &DeclarationGraph,
        sources: &BodySourceCatalog<'syntax>,
        checked: impl IntoIterator<
            Item = (
                BodyId,
                &'program CheckedBody,
                &'program HashMap<BodyNodeId, SourceOrigin>,
            ),
        >,
    ) -> Result<Self, BodyCheckInternalError> {
        let mut checked_by_body = HashMap::new();
        for (body, checked, origins) in checked {
            if checked_by_body.insert(body, (checked, origins)).is_some() {
                return Err(BodyCheckInternalError::DuplicateBodyRelation(body));
            }
        }

        let mut inputs = ArenaBuilder::new();
        for (body, _) in graph.declarations().bodies().iter() {
            let (checked, origins) = checked_by_body
                .remove(&body)
                .ok_or(BodyCheckInternalError::MissingBodyRelation(body))?;
            let source = sources
                .get(body)
                .ok_or(BodyCheckInternalError::MissingBodySource(body))?;
            let actual = inputs.insert(BodyRelationInput {
                source,
                body: checked,
                origins,
            });
            if actual != body {
                return Err(BodyCheckInternalError::NonCanonicalBody(body));
            }
        }
        if let Some(body) = checked_by_body.keys().copied().min() {
            return Err(BodyCheckInternalError::UnknownBodyRelation(body));
        }
        Ok(Self {
            inputs: inputs.finish(),
        })
    }

    pub(crate) fn get(
        &self,
        body: BodyId,
    ) -> Result<&BodyRelationInput<'program, 'syntax>, BodyCheckInternalError> {
        self.inputs
            .get(body)
            .ok_or(BodyCheckInternalError::MissingBodyRelation(body))
    }
}
