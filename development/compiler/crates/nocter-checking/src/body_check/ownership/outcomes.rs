use nocter_model::{BodyNodeId, LocalBindingId};

use super::OwnershipAnalyzer;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::ownership::{MovePath, OwnershipState};
use crate::{CleanupTiming, PlaceRoot};

impl OwnershipAnalyzer<'_> {
    pub(super) fn visit_propagate(
        &mut self,
        node: BodyNodeId,
        operand: BodyNodeId,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        if !self.visit(operand, state)? {
            return Ok(false);
        }
        let mut failure_state = state.clone();
        let mut actions = self.temporary_cleanup_actions(&failure_state, &[])?;
        actions.extend(self.all_scope_cleanup(&mut failure_state)?);
        self.record_cleanup(node, CleanupTiming::OnOutcomePropagation, actions);
        Ok(true)
    }

    pub(super) fn visit_recover(
        &mut self,
        operand: BodyNodeId,
        binding: Option<LocalBindingId>,
        fallback: BodyNodeId,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        if !self.visit(operand, state)? {
            return Ok(false);
        }
        let entry = state.clone();
        let mut incoming = vec![entry.clone()];
        let mut fallback_state = entry.clone();
        if let Some(binding) = binding {
            fallback_state
                .declare_initialized(MovePath::root(PlaceRoot::Local(binding)))
                .map_err(|_| BodyCheckInternalError::OwnershipState)?;
        }
        if self.visit(fallback, &mut fallback_state)? {
            incoming.push(fallback_state);
        }
        *state = entry
            .join_reachable(&incoming)
            .map_err(|_| BodyCheckInternalError::OwnershipState)?;
        Ok(true)
    }
}
