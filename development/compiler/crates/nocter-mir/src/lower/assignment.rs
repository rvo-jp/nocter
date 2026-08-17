use nocter_model::{BodyNodeId, PlaceId};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{MirOperationKind, MirReadMode};

impl FunctionLowerer<'_> {
    pub(super) fn lower_assignment(
        &mut self,
        node: BodyNodeId,
        target: PlaceId,
        value: BodyNodeId,
    ) -> Result<(), MirLoweringError> {
        let value = self.require_value(value)?;
        let destination = self.lower_place(target)?;
        self.lower_cleanup(node, nocter_checking::CleanupTiming::BeforeStore)?;
        self.append_effect(MirOperationKind::Store { destination, value })?;
        self.mark_place_initialized(target, true)
    }

    pub(super) fn lower_compound_assignment(
        &mut self,
        target: PlaceId,
        value: BodyNodeId,
        operation: nocter_checking::PrimitiveBinary,
    ) -> Result<(), MirLoweringError> {
        let value = self.require_value(value)?;
        let destination = self.lower_place(target)?;
        let ty = self
            .builder
            .place(destination)
            .map(crate::MirPlace::ty)
            .ok_or(MirLoweringError::UnknownPlace(target))?;
        let current = self.append_value(
            ty,
            MirOperationKind::Read {
                place: destination,
                mode: MirReadMode::Copy,
            },
        )?;
        let result = self.append_value(
            ty,
            MirOperationKind::Binary {
                operation: super::function::mir_binary_operation(operation),
                left: current,
                right: value,
            },
        )?;
        self.append_effect(MirOperationKind::Store {
            destination,
            value: result,
        })?;
        Ok(())
    }
}
