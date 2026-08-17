use nocter_checking::{
    CheckedOperation, CheckedReceiver, CoercedReceiverPreparation, ReadonlyOperandPreparation,
    ReceiverPreparation,
};
use nocter_model::{BodyNodeId, BorrowCapability, MirPlaceId, MirValueId};
use nocter_target_program::ExecutableDispatchPlan;

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{MirOperationKind, MirReadMode, MirStructuralCall};

impl FunctionLowerer<'_> {
    pub(super) fn lower_receiver(
        &mut self,
        node: BodyNodeId,
        receiver: &CheckedReceiver,
        expected: nocter_model::TypeId,
    ) -> Result<MirValueId, MirLoweringError> {
        let Some(coercion) = receiver.coercion() else {
            return self.lower_receiver_base(node, receiver, expected);
        };
        let step = {
            let plan = self
                .item
                .body()
                .dispatch(coercion.selection())
                .ok_or(MirLoweringError::InvalidDispatch(node))?;
            let ExecutableDispatchPlan::Invocation(step) = plan else {
                return Err(MirLoweringError::UnsupportedOperation(node));
            };
            step.clone()
        };
        let signature = self.step_signature(&step)?;
        let [input] = signature.parameters() else {
            return Err(MirLoweringError::InvalidDispatch(node));
        };
        let prepared = self.lower_receiver_base(node, receiver, *input)?;
        let converted = self.emit_dispatch_step(node, signature.result(), &step, [prepared])?;
        match coercion.result_preparation() {
            CoercedReceiverPreparation::PreserveReadonly
            | CoercedReceiverPreparation::PreserveReadwrite => {
                self.require_value_type(converted, expected, node)
            }
            CoercedReceiverPreparation::WeakenReadwrite => self.weaken_borrow(converted, expected),
        }
    }

    fn lower_receiver_base(
        &mut self,
        node: BodyNodeId,
        receiver: &CheckedReceiver,
        expected: nocter_model::TypeId,
    ) -> Result<MirValueId, MirLoweringError> {
        match receiver.preparation() {
            ReceiverPreparation::Owned => self.require_value(receiver.value()),
            ReceiverPreparation::BorrowPlace(capability) => {
                let place = self.lower_place_node(receiver.value())?;
                self.borrow_place(place, capability, expected)
            }
            ReceiverPreparation::BorrowTemporary(capability) => {
                let value = self.require_value(receiver.value())?;
                let place = self.materialize_value_storage(receiver.value(), value)?;
                self.borrow_place(place, capability, expected)
            }
            ReceiverPreparation::PreserveBorrow(_) => {
                let value = self.lower_place_carrier(receiver.value())?;
                self.require_value_type(value, expected, node)
            }
            ReceiverPreparation::WeakenReadwriteBorrow => {
                let value = self.lower_place_carrier(receiver.value())?;
                self.weaken_borrow(value, expected)
            }
        }
    }

    pub(super) fn lower_readonly_operand(
        &mut self,
        node: BodyNodeId,
        value: BodyNodeId,
        preparation: ReadonlyOperandPreparation,
        expected: nocter_model::TypeId,
    ) -> Result<MirValueId, MirLoweringError> {
        match preparation {
            ReadonlyOperandPreparation::BorrowPlace => {
                let place = self.lower_place_node(value)?;
                self.borrow_place(place, BorrowCapability::Readonly, expected)
            }
            ReadonlyOperandPreparation::BorrowTemporary => {
                let node = value;
                let value = self.require_value(node)?;
                let place = self.materialize_value_storage(node, value)?;
                self.borrow_place(place, BorrowCapability::Readonly, expected)
            }
            ReadonlyOperandPreparation::UseReadonlyBorrow => {
                let value = self.lower_place_carrier(value)?;
                self.require_value_type(value, expected, node)
            }
            ReadonlyOperandPreparation::WeakenReadwriteBorrow => {
                let value = self.lower_place_carrier(value)?;
                self.weaken_borrow(value, expected)
            }
        }
    }

    fn lower_place_node(&mut self, node: BodyNodeId) -> Result<MirPlaceId, MirLoweringError> {
        let checked = self
            .body
            .nodes()
            .get(node)
            .ok_or(MirLoweringError::UnknownNode(node))?;
        let CheckedOperation::Place(place) = checked.operation() else {
            return Err(MirLoweringError::ExpectedPlace(node));
        };
        self.lower_place(*place)
    }

    pub(super) fn lower_place_carrier(
        &mut self,
        node: BodyNodeId,
    ) -> Result<MirValueId, MirLoweringError> {
        let checked = self
            .body
            .nodes()
            .get(node)
            .ok_or(MirLoweringError::UnknownNode(node))?;
        if let CheckedOperation::Place(place) = checked.operation() {
            let source_ty = checked.ty();
            let place = *place;
            let place = self.lower_place(place)?;
            return self.append_value(
                self.concrete_type(source_ty)?,
                MirOperationKind::Read {
                    place,
                    mode: MirReadMode::Copy,
                },
            );
        }
        self.require_value(node)
    }

    fn borrow_place(
        &mut self,
        place: MirPlaceId,
        capability: BorrowCapability,
        expected: nocter_model::TypeId,
    ) -> Result<MirValueId, MirLoweringError> {
        self.append_value(expected, MirOperationKind::Borrow { place, capability })
    }

    pub(super) fn weaken_borrow(
        &mut self,
        value: MirValueId,
        target: nocter_model::TypeId,
    ) -> Result<MirValueId, MirLoweringError> {
        let source = self
            .builder
            .value_type(value)
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        self.append_value(
            target,
            MirOperationKind::Call(crate::MirCall::new(
                crate::MirCallTarget::Structural(MirStructuralCall::BorrowWeakening {
                    source,
                    target,
                }),
                [value],
            )),
        )
    }

    fn require_value_type(
        &self,
        value: MirValueId,
        expected: nocter_model::TypeId,
        node: BodyNodeId,
    ) -> Result<MirValueId, MirLoweringError> {
        if self.builder.value_type(value) == Some(expected) {
            Ok(value)
        } else {
            Err(MirLoweringError::InvalidDispatch(node))
        }
    }
}
