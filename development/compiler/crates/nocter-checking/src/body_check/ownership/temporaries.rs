use std::collections::HashMap;

use nocter_model::BodyNodeId;

use crate::body_check::error::BodyCheckInternalError;
use crate::ownership::{InitializationState, OwnershipState};
use crate::{CleanupAction, CleanupCondition};

/// Owns the identity, creation order, and flow-dependent liveness of evaluated temporaries.
///
/// The catalog is body-wide and immutable after an identity is first observed. Liveness remains in
/// `OwnershipState`, so ordinary branch joins also produce conditional temporary cleanup without
/// a parallel control-flow analysis.
#[derive(Default)]
pub(super) struct TemporaryPlanner {
    actions: HashMap<BodyNodeId, CleanupAction>,
    order: Vec<BodyNodeId>,
}

impl TemporaryPlanner {
    pub(super) fn activate(
        &mut self,
        node: BodyNodeId,
        action: Option<CleanupAction>,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckInternalError> {
        let Some(action) = action else {
            return Ok(false);
        };
        if let Some(existing) = self.actions.get(&node) {
            if existing != &action {
                return Err(BodyCheckInternalError::CleanupPlanning);
            }
        } else {
            self.actions.insert(node, action);
            self.order.push(node);
        }
        state
            .declare_temporary(node)
            .map_err(|_| BodyCheckInternalError::OwnershipState)?;
        Ok(true)
    }

    pub(super) fn cleanup_actions(
        &self,
        state: &OwnershipState,
        retained: &[BodyNodeId],
    ) -> Result<Vec<CleanupAction>, BodyCheckInternalError> {
        self.order
            .iter()
            .rev()
            .filter(|temporary| retained.binary_search(temporary).is_err())
            .filter_map(|temporary| {
                let condition = match state.temporary_initialization(*temporary) {
                    InitializationState::Initialized => CleanupCondition::Always,
                    InitializationState::MaybeInitialized => CleanupCondition::IfInitialized,
                    InitializationState::Uninitialized => return None,
                };
                Some(
                    self.actions
                        .get(temporary)
                        .map(|action| action.with_condition(condition))
                        .ok_or(BodyCheckInternalError::CleanupPlanning),
                )
            })
            .collect()
    }
}
