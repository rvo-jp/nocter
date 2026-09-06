use std::collections::HashMap;

use nocter_declarations::{BodyOwner, DeclarationGraph};
use nocter_model::{Arena, ArenaBuilder, BodyId, BodyNodeId};
use nocter_source_index::SourceOrigin;

use crate::{
    BodyCheckInternalError, BodyRule, CheckedBody,
    body_relation_error::{BodyRelationError, BodyRelationNote},
};

/// One checked body's complete input to program-wide semantic relation analysis.
///
/// The catalog owns the `BodyId` association. Individual analyses cannot pair a checked body with
/// a sibling's source or origin map, and do not need to rediscover the association by scanning a
/// caller-assembled slice.
pub(crate) struct BodyRelationInput<'program> {
    body_id: BodyId,
    owner: BodyOwner,
    body: &'program CheckedBody,
}

impl<'program> BodyRelationInput<'program> {
    #[must_use]
    pub(crate) const fn body_id(&self) -> BodyId {
        self.body_id
    }

    #[must_use]
    pub(crate) const fn owner(&self) -> BodyOwner {
        self.owner
    }

    #[must_use]
    pub(crate) const fn body(&self) -> &'program CheckedBody {
        self.body
    }

    #[must_use]
    pub(crate) fn reject(&self, rule: BodyRule, primary: BodyNodeId) -> BodyRelationError {
        BodyRelationError::rule(self.body_id, rule, primary, [])
    }

    #[must_use]
    pub(crate) fn reject_with_notes(
        &self,
        rule: BodyRule,
        primary: BodyNodeId,
        notes: impl Into<Box<[BodyRelationNote]>>,
    ) -> BodyRelationError {
        BodyRelationError::rule(self.body_id, rule, primary, notes)
    }
}

/// Canonical, exact-coverage body inputs shared by all program-wide relation analyses.
///
/// Construction proves the checked-body set covers the declaration graph exactly once and pairs
/// every body with its source projection once. Consumers receive O(1) `BodyId` lookup and cannot
/// express independently ordered provenance, effect, or loan input sets.
pub(crate) struct BodyRelationCatalog<'program> {
    inputs: Arena<BodyId, BodyRelationInput<'program>>,
}

impl<'program> BodyRelationCatalog<'program> {
    pub(crate) fn new(
        graph: &DeclarationGraph,
        checked: impl IntoIterator<Item = (BodyId, &'program CheckedBody)>,
    ) -> Result<Self, BodyCheckInternalError> {
        let mut checked_by_body = HashMap::new();
        for (body, checked) in checked {
            if checked_by_body.insert(body, checked).is_some() {
                return Err(BodyCheckInternalError::DuplicateBodyRelation(body));
            }
        }

        let mut inputs = ArenaBuilder::new();
        for (body, declaration) in graph.declarations().bodies().iter() {
            let checked = checked_by_body
                .remove(&body)
                .ok_or(BodyCheckInternalError::MissingBodyRelation(body))?;
            let actual = inputs.insert(BodyRelationInput {
                body_id: body,
                owner: declaration.owner(),
                body: checked,
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
    ) -> Result<&BodyRelationInput<'program>, BodyCheckInternalError> {
        self.inputs
            .get(body)
            .ok_or(BodyCheckInternalError::MissingBodyRelation(body))
    }
}

/// Exact-current source projection for source-neutral relation failures.
pub(crate) struct BodyRelationProjection<'program> {
    origins: Arena<BodyId, &'program HashMap<BodyNodeId, SourceOrigin>>,
}

impl<'program> BodyRelationProjection<'program> {
    pub(crate) fn new(
        graph: &DeclarationGraph,
        origins: impl IntoIterator<Item = (BodyId, &'program HashMap<BodyNodeId, SourceOrigin>)>,
    ) -> Result<Self, BodyCheckInternalError> {
        let mut origins_by_body = HashMap::new();
        for (body, origins) in origins {
            if origins_by_body.insert(body, origins).is_some() {
                return Err(BodyCheckInternalError::DuplicateBodyRelation(body));
            }
        }
        let mut projected = ArenaBuilder::new();
        for (body, _) in graph.declarations().bodies().iter() {
            let origins = origins_by_body
                .remove(&body)
                .ok_or(BodyCheckInternalError::MissingBodyRelation(body))?;
            if projected.insert(origins) != body {
                return Err(BodyCheckInternalError::NonCanonicalBody(body));
            }
        }
        if let Some(body) = origins_by_body.keys().copied().min() {
            return Err(BodyCheckInternalError::UnknownBodyRelation(body));
        }
        Ok(Self {
            origins: projected.finish(),
        })
    }

    pub(crate) fn origin(
        &self,
        body: BodyId,
        node: BodyNodeId,
    ) -> Result<SourceOrigin, BodyCheckInternalError> {
        self.origins
            .get(body)
            .ok_or(BodyCheckInternalError::MissingBodyRelation(body))?
            .get(&node)
            .copied()
            .ok_or(BodyCheckInternalError::MissingNodeOrigin(node))
    }
}
