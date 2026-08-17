use nocter_model::BodyNodeId;

use super::OwnershipAnalyzer;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::ownership::{OwnershipState, TemporaryIdentity};
use crate::{CheckedInterpolation, InterpolationPart, ReadonlyOperandPreparation};

impl OwnershipAnalyzer<'_> {
    pub(super) fn visit_interpolation(
        &mut self,
        node: BodyNodeId,
        interpolation: &CheckedInterpolation,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        if !self.visit_allocation(interpolation.allocation(), state)? {
            return Ok(false);
        }
        let in_progress = TemporaryIdentity::InterpolationInProgress(node);
        let partial = self.value_cleanup(node, interpolation.output())?;
        let partial_is_owned = self.temporaries.activate(in_progress, partial, state)?;

        for part in interpolation.parts() {
            match part {
                InterpolationPart::Text(_) => {}
                InterpolationPart::Formatted { operand, .. } => {
                    if !self.visit(operand.value(), state)? {
                        return Ok(false);
                    }
                    if operand.preparation() == ReadonlyOperandPreparation::BorrowTemporary {
                        self.activate_owned_temporary(operand.value(), state)?;
                    }
                }
                InterpolationPart::Diverging(value) => {
                    if !self.visit(*value, state)? {
                        return Ok(false);
                    }
                    return Err(
                        BodyCheckInternalError::UnsupportedOwnershipOperation(*value).into(),
                    );
                }
            }
        }

        if partial_is_owned {
            state
                .consume_temporary(in_progress)
                .map_err(|_| BodyCheckInternalError::OwnershipState)?;
        }
        Ok(true)
    }
}
