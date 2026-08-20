use nocter_checking::StaticSelection;
use nocter_model::{BodyNodeId, BorrowCapability, MirValueId, PlaceId, TypeId, TypeKind};
use nocter_target_program::{ExecutableDispatchPlan, ExecutableDispatchStep};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use super::place::LoweredPlacePath;
use crate::{MirOperationKind, MirPlaceRoot, MirProjectionKind};

impl FunctionLowerer<'_> {
    pub(super) fn lower_coerced_builtin_index(
        &mut self,
        place: PlaceId,
        path: &mut LoweredPlacePath,
        index: BodyNodeId,
        receiver_coercion: &StaticSelection,
        result: TypeId,
    ) -> Result<(), MirLoweringError> {
        let coercion = self.place_invocation_step(place, receiver_coercion)?;
        let (_, target) = self.unary_place_step_types(place, &coercion)?;
        let receiver = self.prepare_index_receiver(place, path, [&coercion], target)?;
        let (capability, container) = self.borrow_shape(place, target)?;
        let index = self.require_value(index)?;

        path.root = MirPlaceRoot::Dereference {
            value: receiver,
            capability,
        };
        path.projections.clear();
        path.ty = container;
        path.push(MirProjectionKind::DynamicIndex(index), result);
        Ok(())
    }

    pub(super) fn lower_selected_index(
        &mut self,
        place: PlaceId,
        path: &mut LoweredPlacePath,
        index: BodyNodeId,
        operation: &StaticSelection,
        checked_receiver_coercion: Option<&StaticSelection>,
        result: TypeId,
    ) -> Result<(), MirLoweringError> {
        let plan = self
            .item
            .body()
            .dispatch(operation)
            .ok_or(MirLoweringError::InvalidPlaceDispatch(place))?;
        let (operation, resolved_receiver_coercion) = match plan {
            ExecutableDispatchPlan::Invocation(operation) => (operation.clone(), None),
            ExecutableDispatchPlan::Index {
                receiver_coercion,
                operation,
            } => (operation.clone(), receiver_coercion.clone()),
            ExecutableDispatchPlan::Comparison { .. }
            | ExecutableDispatchPlan::OpaqueInvocation { .. } => {
                return Err(MirLoweringError::InvalidPlaceDispatch(place));
            }
        };
        if checked_receiver_coercion.is_some() && resolved_receiver_coercion.is_some() {
            return Err(MirLoweringError::InvalidPlaceDispatch(place));
        }
        let checked_receiver_coercion = checked_receiver_coercion
            .map(|selection| self.place_invocation_step(place, selection))
            .transpose()?;
        let signature = self.step_signature(&operation)?;
        let [receiver_type, index_type] = signature.parameters() else {
            return Err(MirLoweringError::InvalidPlaceDispatch(place));
        };
        let coercions = [
            checked_receiver_coercion.as_ref(),
            resolved_receiver_coercion.as_ref(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        let receiver =
            self.prepare_index_receiver(place, path, coercions.iter().copied(), *receiver_type)?;
        let index = self.require_value(index)?;
        if self.builder.value_type(index) != Some(*index_type) {
            return Err(MirLoweringError::InvalidPlaceDispatch(place));
        }
        let borrow = self.emit_place_dispatch_step(
            place,
            signature.result(),
            &operation,
            [receiver, index],
        )?;
        let (capability, referent) = self.borrow_shape(place, signature.result())?;
        if referent != result {
            return Err(MirLoweringError::InvalidPlaceDispatch(place));
        }
        path.root = MirPlaceRoot::Dereference {
            value: borrow,
            capability,
        };
        path.projections.clear();
        path.ty = referent;
        Ok(())
    }

    fn prepare_index_receiver<'step>(
        &mut self,
        place: PlaceId,
        path: &LoweredPlacePath,
        coercions: impl IntoIterator<Item = &'step ExecutableDispatchStep>,
        expected: TypeId,
    ) -> Result<MirValueId, MirLoweringError> {
        let coercions = coercions.into_iter().collect::<Vec<_>>();
        let input = coercions
            .first()
            .map(|step| {
                self.unary_place_step_types(place, step)
                    .map(|types| types.0)
            })
            .transpose()?
            .unwrap_or(expected);
        let (capability, referent) = self.borrow_shape(place, input)?;
        if referent != path.ty {
            return Err(MirLoweringError::InvalidPlaceDispatch(place));
        }
        let receiver_place = self
            .builder
            .add_place(path.root, path.projections.clone(), path.ty);
        let mut receiver = self.append_value(
            input,
            MirOperationKind::Borrow {
                place: receiver_place,
                capability,
            },
        )?;
        for coercion in coercions {
            let (source, target) = self.unary_place_step_types(place, coercion)?;
            if self.builder.value_type(receiver) != Some(source) {
                return Err(MirLoweringError::InvalidPlaceDispatch(place));
            }
            receiver = self.emit_place_dispatch_step(place, target, coercion, [receiver])?;
        }
        if self.builder.value_type(receiver) != Some(expected) {
            return Err(MirLoweringError::InvalidPlaceDispatch(place));
        }
        Ok(receiver)
    }

    fn place_invocation_step(
        &self,
        place: PlaceId,
        selection: &StaticSelection,
    ) -> Result<ExecutableDispatchStep, MirLoweringError> {
        let plan = self
            .item
            .body()
            .dispatch(selection)
            .ok_or(MirLoweringError::InvalidPlaceDispatch(place))?;
        let ExecutableDispatchPlan::Invocation(step) = plan else {
            return Err(MirLoweringError::InvalidPlaceDispatch(place));
        };
        Ok(step.clone())
    }

    fn unary_place_step_types(
        &self,
        place: PlaceId,
        step: &ExecutableDispatchStep,
    ) -> Result<(TypeId, TypeId), MirLoweringError> {
        let signature = self.step_signature(step)?;
        let [input] = signature.parameters() else {
            return Err(MirLoweringError::InvalidPlaceDispatch(place));
        };
        Ok((*input, signature.result()))
    }

    pub(super) fn borrow_shape(
        &self,
        place: PlaceId,
        ty: TypeId,
    ) -> Result<(BorrowCapability, TypeId), MirLoweringError> {
        let Some(TypeKind::Borrow {
            capability,
            referent,
        }) = self.executable.types().get(ty)
        else {
            return Err(MirLoweringError::InvalidPlaceDispatch(place));
        };
        Ok((*capability, *referent))
    }
}
