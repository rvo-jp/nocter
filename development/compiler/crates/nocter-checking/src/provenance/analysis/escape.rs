use nocter_model::{BodyNodeId, BodyScopeId, LocalBindingId, PlaceId};

use super::Analyzer;
use crate::provenance::state::ProvenanceState;
use crate::{
    BodyCheckError, BodyCheckInternalError, BodyRule, PlaceProjection, PlaceRoot, ProvenanceSource,
    ValueProvenance,
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
    ) -> Result<(), BodyCheckError> {
        let scope = self.local_scope(binding)?;
        self.validate_destination(node, DestinationLifetime::Scope(scope), value)
    }

    pub(super) fn validate_assignment_storage(
        &self,
        node: BodyNodeId,
        target: PlaceId,
        value: &ValueProvenance,
    ) -> Result<(), BodyCheckError> {
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
                PlaceRoot::Parameter(_) | PlaceRoot::Capture(_) => DestinationLifetime::External,
            }
        };
        self.validate_destination(node, destination, value)
    }

    pub(super) fn validate_statement_storage(
        &self,
        node: BodyNodeId,
        state: &ProvenanceState,
    ) -> Result<(), BodyCheckError> {
        let escapes = state.values().any(|(_, value)| {
            value
                .all_sources()
                .iter()
                .any(|source| matches!(source, ProvenanceSource::Temporary(_)))
        });
        self.reject_escape(node, escapes)
    }

    pub(super) fn validate_scope_result(
        &self,
        node: BodyNodeId,
        scope: BodyScopeId,
        value: &ValueProvenance,
    ) -> Result<(), BodyCheckError> {
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
    ) -> Result<(), BodyCheckError> {
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
    ) -> Result<(), BodyCheckError> {
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
                ProvenanceSource::Temporary(_) => false,
                ProvenanceSource::Callable(_)
                | ProvenanceSource::CurrentAllocation
                | ProvenanceSource::OwnedParameter(_)
                | ProvenanceSource::Region(_)
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
                .and_then(|scope| scope.parent())
            else {
                return false;
            };
            scope = parent;
        }
    }

    fn reject_escape(&self, node: BodyNodeId, escapes: bool) -> Result<(), BodyCheckError> {
        if !escapes {
            return Ok(());
        }
        let origin = self
            .origins
            .get(&node)
            .copied()
            .ok_or(BodyCheckInternalError::MissingNodeOrigin(node))?;
        let rule = BodyRule::InvalidStorageEscape;
        Err(BodyCheckError::from_rule(rule, rule.diagnostic(origin)))
    }
}
