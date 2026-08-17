use nocter_checking::CheckedOpaqueWitness;
use nocter_model::{BodyNodeId, MirValueId, TypeId, TypeKind};
use nocter_target_program::ExecutableOpaqueReceiver;

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{
    MirAggregate, MirOperationKind, MirPlaceRoot, MirProjection, MirProjectionKind, MirReadMode,
};

impl FunctionLowerer<'_> {
    pub(super) fn lower_opaque_witness(
        &mut self,
        node: BodyNodeId,
        opaque: TypeId,
        witness: CheckedOpaqueWitness,
    ) -> Result<MirValueId, MirLoweringError> {
        let Some(TypeKind::Opaque { definition, .. }) = self.executable.types().get(opaque) else {
            return Err(MirLoweringError::InvalidOpaqueWitness(node));
        };
        let expected = self.concrete_type(witness.witness())?;
        let value = self.require_value(witness.value())?;
        if *definition != witness.definition() || self.builder.value_type(value) != Some(expected) {
            return Err(MirLoweringError::InvalidOpaqueWitness(node));
        }
        self.append_value(
            opaque,
            MirOperationKind::Aggregate(MirAggregate::Opaque { witness: value }),
        )
    }

    pub(super) fn lower_opaque_receiver(
        &mut self,
        node: BodyNodeId,
        storage_node: BodyNodeId,
        value: MirValueId,
        receiver: ExecutableOpaqueReceiver,
        expected: TypeId,
    ) -> Result<MirValueId, MirLoweringError> {
        if self.builder.value_type(value) != Some(receiver.source())
            || expected != receiver.target()
            || !matches!(
                self.executable.types().get(receiver.opaque()),
                Some(TypeKind::Opaque { definition, .. })
                    if *definition == receiver.definition()
            )
        {
            return Err(MirLoweringError::InvalidOpaqueWitness(node));
        }
        if receiver.source() == receiver.opaque() && receiver.target() == receiver.witness() {
            let storage = self.materialize_value_storage(storage_node, value)?;
            let base = self
                .builder
                .place(storage)
                .cloned()
                .ok_or(MirLoweringError::InvalidOpaqueWitness(node))?;
            let mut projections = base.projections().to_vec();
            projections.push(MirProjection::new(
                MirProjectionKind::OpaqueWitness(receiver.definition()),
                receiver.witness(),
            ));
            let witness = self
                .builder
                .add_place(base.root(), projections, receiver.witness());
            return self.append_value(
                receiver.witness(),
                MirOperationKind::Read {
                    place: witness,
                    mode: MirReadMode::Move,
                },
            );
        }
        let (
            Some(TypeKind::Borrow {
                capability: source_capability,
                referent: source_referent,
            }),
            Some(TypeKind::Borrow {
                capability: target_capability,
                referent: target_referent,
            }),
        ) = (
            self.executable.types().get(receiver.source()),
            self.executable.types().get(receiver.target()),
        )
        else {
            return Err(MirLoweringError::InvalidOpaqueWitness(node));
        };
        if source_capability != target_capability
            || *source_referent != receiver.opaque()
            || *target_referent != receiver.witness()
        {
            return Err(MirLoweringError::InvalidOpaqueWitness(node));
        }
        let witness = self.builder.add_place(
            MirPlaceRoot::Dereference {
                value,
                capability: *source_capability,
            },
            [MirProjection::new(
                MirProjectionKind::OpaqueWitness(receiver.definition()),
                receiver.witness(),
            )],
            receiver.witness(),
        );
        self.borrow_place(witness, *target_capability, receiver.target())
    }
}
