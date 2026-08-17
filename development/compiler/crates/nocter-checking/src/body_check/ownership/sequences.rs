use nocter_model::{BodyNodeId, CallableCapability};

use super::OwnershipAnalyzer;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::ownership::{OwnershipState, TemporaryIdentity};
use crate::{CallTarget, CheckedCall, CheckedOperation, CleanupAction, ReceiverPreparation};

impl OwnershipAnalyzer<'_> {
    pub(super) fn visit_call(
        &mut self,
        call: &CheckedCall,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        let mut staged = Vec::new();
        if let CallTarget::CallableValue {
            value, capability, ..
        }
        | CallTarget::ClosureValue {
            value, capability, ..
        } = call.target()
        {
            if !self.visit(*value, state)? {
                return Ok(false);
            }
            if *capability == CallableCapability::Owned {
                let place = self
                    .body
                    .nodes()
                    .get(*value)
                    .and_then(|node| match node.operation() {
                        CheckedOperation::Place(place) => Some(*place),
                        _ => None,
                    })
                    .ok_or(BodyCheckInternalError::UnsupportedOwnershipOperation(
                        *value,
                    ))?;
                let path = self.move_path(place)?;
                state
                    .move_out(&path)
                    .map_err(|_| BodyCheckInternalError::OwnershipState)?;
                if self.activate_owned_temporary(*value, state)? {
                    staged.push(*value);
                }
            }
        }
        if let Some(receiver) = call.receiver() {
            if !self.visit(receiver.value(), state)? {
                return Ok(false);
            }
            match receiver.preparation() {
                ReceiverPreparation::Owned => {
                    if self.activate_expression_temporary(receiver.value(), state)? {
                        staged.push(receiver.value());
                    }
                }
                ReceiverPreparation::BorrowTemporary(_) => {
                    self.activate_owned_temporary(receiver.value(), state)?;
                }
                ReceiverPreparation::BorrowPlace(_)
                | ReceiverPreparation::PreserveBorrow(_)
                | ReceiverPreparation::WeakenReadwriteBorrow => {}
            }
        }
        for argument in call.arguments() {
            if !self.visit(*argument, state)? {
                return Ok(false);
            }
            if self.activate_expression_temporary(*argument, state)? {
                staged.push(*argument);
            }
        }
        for temporary in staged {
            state
                .consume_temporary(TemporaryIdentity::Value(temporary))
                .map_err(|_| BodyCheckInternalError::OwnershipState)?;
        }
        Ok(true)
    }

    pub(super) fn visit_value_sequence(
        &mut self,
        values: impl IntoIterator<Item = BodyNodeId>,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        let mut staged = Vec::new();
        for value in values {
            if !self.visit(value, state)? {
                return Ok(false);
            }
            if self.activate_expression_temporary(value, state)? {
                staged.push(value);
            }
        }
        for temporary in staged {
            state
                .consume_temporary(TemporaryIdentity::Value(temporary))
                .map_err(|_| BodyCheckInternalError::OwnershipState)?;
        }
        Ok(true)
    }

    pub(super) fn activate_expression_temporary(
        &mut self,
        node: BodyNodeId,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckInternalError> {
        let checked = self
            .body
            .nodes()
            .get(node)
            .ok_or(BodyCheckInternalError::MissingNode(node))?;
        if matches!(
            checked.operation(),
            CheckedOperation::Place(_)
                | CheckedOperation::Copy(_)
                | CheckedOperation::Borrow { .. }
        ) {
            return Ok(false);
        }
        self.activate_owned_temporary(node, state)
    }

    pub(super) fn activate_owned_temporary(
        &mut self,
        node: BodyNodeId,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckInternalError> {
        let checked = self
            .body
            .nodes()
            .get(node)
            .ok_or(BodyCheckInternalError::MissingNode(node))?;
        let action = self.value_cleanup(node, checked.ty())?;
        self.temporaries
            .activate(TemporaryIdentity::Value(node), action, state)
    }

    pub(super) fn temporary_cleanup_actions(
        &self,
        state: &OwnershipState,
        retained: &[TemporaryIdentity],
    ) -> Result<Vec<CleanupAction>, BodyCheckInternalError> {
        self.temporaries.cleanup_actions(state, retained)
    }
}
