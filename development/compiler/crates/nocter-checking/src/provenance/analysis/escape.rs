use nocter_model::{BodyNodeId, BodyScopeId, LocalBindingId, PlaceId};

use super::Analyzer;
use crate::provenance::state::ProvenanceState;
use crate::{
    BodyCheckInternalError, BodyRelationError, BodyRule, PlaceProjection, PlaceRoot,
    ProvenanceSource, ValueProvenance,
};

#[derive(Clone, Copy)]
enum DestinationLifetime {
    Scope(BodyScopeId),
    External,
}

impl Analyzer<'_, '_> {
    pub(super) fn validate_binding_storage(
        &self,
        node: BodyNodeId,
        binding: LocalBindingId,
        value: &ValueProvenance,
    ) -> Result<(), BodyRelationError> {
        let scope = self.local_scope(binding)?;
        self.validate_destination(node, DestinationLifetime::Scope(scope), value)
    }

    pub(super) fn validate_assignment_storage(
        &self,
        node: BodyNodeId,
        target: PlaceId,
        value: &ValueProvenance,
    ) -> Result<(), BodyRelationError> {
        let place = self
            .body
            .places()
            .get(target)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(target))?;
        let destination = if place
            .projections()
            .iter()
            .any(|projection| matches!(projection, PlaceProjection::BorrowDeref { .. }))
        {
            DestinationLifetime::External
        } else {
            match place.root() {
                PlaceRoot::Local(local) => DestinationLifetime::Scope(self.local_scope(local)?),
                PlaceRoot::Parameter(_)
                | PlaceRoot::Capture(_)
                | PlaceRoot::Value(_)
                | PlaceRoot::Static(_) => DestinationLifetime::External,
            }
        };
        self.validate_destination(node, destination, value)
    }

    pub(super) fn validate_statement_storage(
        &self,
        node: BodyNodeId,
        state: &ProvenanceState,
    ) -> Result<(), BodyRelationError> {
        let escapes = state.values().any(|(_, value)| {
            value
                .all_sources()
                .iter()
                .any(|source| matches!(source, ProvenanceSource::StatementTemporary(_)))
        });
        self.reject_escape(node, escapes)
    }

    pub(super) fn validate_scope_result(
        &self,
        node: BodyNodeId,
        scope: BodyScopeId,
        value: &ValueProvenance,
    ) -> Result<(), BodyRelationError> {
        // The callable-result boundary owns root-body diagnostics and its declared provenance
        // contract. Nested blocks instead cross a lexical storage boundary here.
        if node == self.body.root() {
            return Ok(());
        }
        let escapes = value.all_sources().iter().any(|source| match source {
            ProvenanceSource::Local(local) => self
                .local_scope(*local)
                .is_ok_and(|source_scope| self.scope_contains(scope, source_scope)),
            ProvenanceSource::Region(region) => self
                .local_scope(*region)
                .is_ok_and(|source_scope| self.scope_contains(scope, source_scope)),
            ProvenanceSource::ScopedTemporary {
                scope: source_scope,
                ..
            } => self.scope_contains(scope, *source_scope),
            _ => false,
        });
        self.reject_escape(node, escapes)
    }

    pub(super) fn validate_region_exit(
        &self,
        node: BodyNodeId,
        region: LocalBindingId,
        result: &ValueProvenance,
        state: &ProvenanceState,
    ) -> Result<(), BodyRelationError> {
        let carries_region = |value: &ValueProvenance| {
            value
                .all_sources()
                .contains(&ProvenanceSource::Region(region))
        };
        let escapes =
            carries_region(result) || state.values().any(|(_, value)| carries_region(value));
        self.reject_escape(node, escapes)
    }

    fn validate_destination(
        &self,
        node: BodyNodeId,
        destination: DestinationLifetime,
        value: &ValueProvenance,
    ) -> Result<(), BodyRelationError> {
        let escapes = value
            .all_sources()
            .iter()
            .any(|source| !self.source_outlives(*source, destination));
        self.reject_escape(node, escapes)
    }

    fn source_outlives(&self, source: ProvenanceSource, destination: DestinationLifetime) -> bool {
        match destination {
            DestinationLifetime::Scope(destination) => match source {
                ProvenanceSource::Local(local) => self
                    .local_scope(local)
                    .is_ok_and(|source| self.scope_contains(source, destination)),
                ProvenanceSource::StatementTemporary(_) => false,
                ProvenanceSource::ScopedTemporary { scope: source, .. } => {
                    self.scope_contains(source, destination)
                }
                ProvenanceSource::Callable(_)
                | ProvenanceSource::CurrentAllocation
                | ProvenanceSource::OwnedParameter(_)
                | ProvenanceSource::Region(_)
                | ProvenanceSource::ClosureParameter { .. }
                | ProvenanceSource::ClosureCaptureValue { .. }
                | ProvenanceSource::ClosureEnvironment(_)
                | ProvenanceSource::Unknown => true,
            },
            DestinationLifetime::External => matches!(
                source,
                ProvenanceSource::Callable(_) | ProvenanceSource::CurrentAllocation
            ),
        }
    }

    fn local_scope(&self, local: LocalBindingId) -> Result<BodyScopeId, BodyCheckInternalError> {
        self.body
            .locals()
            .get(local)
            .map(|local| local.declaration().scope())
            .ok_or(BodyCheckInternalError::ProvenanceAnalysis)
    }

    fn scope_contains(&self, ancestor: BodyScopeId, mut scope: BodyScopeId) -> bool {
        loop {
            if scope == ancestor {
                return true;
            }
            let Some(parent) = self
                .body
                .scopes()
                .get(scope)
                .and_then(crate::BodyScope::parent)
            else {
                return false;
            };
            scope = parent;
        }
    }

    fn reject_escape(&self, node: BodyNodeId, escapes: bool) -> Result<(), BodyRelationError> {
        if !escapes {
            return Ok(());
        }
        let rule = BodyRule::InvalidStorageEscape;
        Err(BodyRelationError::rule(self.body_id, rule, node, []))
    }
}
