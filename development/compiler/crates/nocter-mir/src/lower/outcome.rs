use nocter_checking::{CheckedOutcome, OutcomeLayer};
use nocter_model::{
    BodyNodeId, BuiltinType, MirBlockId, MirLocalId, MirPlaceId, MirValueId, TypeId, TypeKind,
};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{
    MirAggregate, MirBranchTarget, MirOperationKind, MirPlaceRoot, MirProjection,
    MirProjectionKind, MirReadMode, MirSwitchCase, MirSwitchSubject, MirSwitchValue, MirTerminator,
};

#[derive(Clone, Copy)]
struct OutcomeStorage {
    local: MirLocalId,
    place: MirPlaceId,
    ty: TypeId,
}

impl FunctionLowerer<'_> {
    pub(super) fn lower_outcome(
        &mut self,
        node: BodyNodeId,
        ty: TypeId,
        outcome: &CheckedOutcome,
    ) -> Result<Option<MirValueId>, MirLoweringError> {
        match outcome {
            CheckedOutcome::Inject { layer, payload } => {
                let payload = self.outcome_injection_payload(*payload)?;
                self.inject_outcome(node, ty, *layer, payload).map(Some)
            }
            CheckedOutcome::Absent => self
                .append_value(
                    ty,
                    MirOperationKind::Aggregate(MirAggregate::Optional(None)),
                )
                .map(Some),
            CheckedOutcome::Failure(error) => {
                let error = self.require_value(*error)?;
                self.append_value(
                    ty,
                    MirOperationKind::Aggregate(MirAggregate::FallibleFailure(error)),
                )
                .map(Some)
            }
            CheckedOutcome::Propagate {
                operand,
                layer,
                outer,
            } => self.lower_propagation(node, *operand, *layer, outer),
            CheckedOutcome::Force { operand, layer } => self.lower_force(node, *operand, *layer),
            CheckedOutcome::Recover {
                operand,
                layer,
                binding,
                fallback,
            } => self.lower_recovery(node, ty, *operand, *layer, *binding, *fallback),
        }
    }

    fn lower_propagation(
        &mut self,
        node: BodyNodeId,
        operand: BodyNodeId,
        layer: OutcomeLayer,
        outer: &[OutcomeLayer],
    ) -> Result<Option<MirValueId>, MirLoweringError> {
        let place = self.materialize_outcome_operand(operand)?;
        let payload = self.outcome_payload_type(place, layer, node)?;
        let (success, failure) = self.switch_outcome(place, layer)?;

        self.current = Some(failure);
        self.lower_cleanup(node, nocter_checking::CleanupTiming::OnOutcomePropagation)?;
        let returned = self.propagated_failure(node, place, layer, outer)?;
        self.destroy_pack()?;
        let failure_block = self
            .current
            .take()
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        self.builder
            .terminate(failure_block, MirTerminator::Return(Some(returned)))?;

        self.current = Some(success);
        self.read_outcome_payload(place, layer, payload)
    }

    fn lower_force(
        &mut self,
        node: BodyNodeId,
        operand: BodyNodeId,
        layer: OutcomeLayer,
    ) -> Result<Option<MirValueId>, MirLoweringError> {
        let place = self.materialize_outcome_operand(operand)?;
        let payload = self.outcome_payload_type(place, layer, node)?;
        let (success, failure) = self.switch_outcome(place, layer)?;
        self.builder.terminate(failure, MirTerminator::Trap)?;
        self.current = Some(success);
        self.read_outcome_payload(place, layer, payload)
    }

    fn lower_recovery(
        &mut self,
        node: BodyNodeId,
        ty: TypeId,
        operand: BodyNodeId,
        layer: OutcomeLayer,
        binding: Option<nocter_model::LocalBindingId>,
        fallback: BodyNodeId,
    ) -> Result<Option<MirValueId>, MirLoweringError> {
        let place = self.materialize_outcome_operand(operand)?;
        let payload = self.outcome_payload_type(place, layer, node)?;
        if payload != ty {
            return Err(MirLoweringError::InvalidDispatch(node));
        }
        let (success, failure) = self.switch_outcome(place, layer)?;

        self.current = Some(success);
        let success_value = self.read_outcome_payload(place, layer, payload)?;
        let success_exit = self.current.map(|block| (block, success_value));

        self.current = Some(failure);
        if let Some(binding) = binding {
            let error = self.read_outcome_failure(place, layer, node)?;
            let local = self.ensure_local(binding)?;
            let destination = self.builder.add_place(
                MirPlaceRoot::Local(local),
                [],
                self.executable.types().builtin(BuiltinType::Error),
            );
            self.append_effect(MirOperationKind::Initialize {
                destination,
                value: error,
            })?;
        }
        let fallback_value = self.lower_node(fallback)?;
        let fallback_exit = self.current.map(|block| (block, fallback_value));
        let carries_value = !matches!(
            self.executable.types().get(ty),
            Some(TypeKind::Builtin(BuiltinType::Void | BuiltinType::Never))
        );
        self.join_branches(ty, carries_value, [success_exit, fallback_exit])
    }

    fn materialize_outcome_operand(
        &mut self,
        operand: BodyNodeId,
    ) -> Result<OutcomeStorage, MirLoweringError> {
        let value = self.require_value(operand)?;
        let ty = self
            .builder
            .value_type(value)
            .ok_or(MirLoweringError::UnknownValue(value))?;
        let place = self.materialize_value_storage(operand, value)?;
        let Some(MirPlaceRoot::Local(local)) = self.builder.place(place).map(crate::MirPlace::root)
        else {
            return Err(MirLoweringError::InvalidCleanup(operand));
        };
        Ok(OutcomeStorage { local, place, ty })
    }

    fn switch_outcome(
        &mut self,
        storage: OutcomeStorage,
        layer: OutcomeLayer,
    ) -> Result<(MirBlockId, MirBlockId), MirLoweringError> {
        let source = self
            .current
            .take()
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        let (success, _) = self.builder.create_block([]);
        let (failure, _) = self.builder.create_block([]);
        let success_value = match layer {
            OutcomeLayer::Optional => MirSwitchValue::OptionalPresent,
            OutcomeLayer::Fallible => MirSwitchValue::FallibleSuccess,
        };
        self.builder.terminate(
            source,
            MirTerminator::Switch {
                subject: MirSwitchSubject::Place(storage.place),
                cases: Box::new([MirSwitchCase::new(
                    success_value,
                    MirBranchTarget::new(success, []),
                )]),
                fallback: MirBranchTarget::new(failure, []),
            },
        )?;
        Ok((success, failure))
    }

    fn outcome_payload_type(
        &self,
        storage: OutcomeStorage,
        layer: OutcomeLayer,
        node: BodyNodeId,
    ) -> Result<TypeId, MirLoweringError> {
        match (layer, self.executable.types().get(storage.ty)) {
            (OutcomeLayer::Optional, Some(TypeKind::Optional(payload)))
            | (OutcomeLayer::Fallible, Some(TypeKind::Fallible(payload))) => Ok(*payload),
            _ => Err(MirLoweringError::InvalidDispatch(node)),
        }
    }

    fn read_outcome_payload(
        &mut self,
        source: OutcomeStorage,
        layer: OutcomeLayer,
        payload: TypeId,
    ) -> Result<Option<MirValueId>, MirLoweringError> {
        if layer == OutcomeLayer::Fallible
            && matches!(
                self.executable.types().get(payload),
                Some(TypeKind::Builtin(BuiltinType::Void))
            )
        {
            return Ok(None);
        }
        let root = MirPlaceRoot::Local(source.local);
        let projection = match layer {
            OutcomeLayer::Optional => MirProjectionKind::OptionalPayload,
            OutcomeLayer::Fallible => MirProjectionKind::FallibleSuccess,
        };
        let place =
            self.builder
                .add_place(root, [MirProjection::new(projection, payload)], payload);
        self.append_value(
            payload,
            MirOperationKind::Read {
                place,
                mode: MirReadMode::Move,
            },
        )
        .map(Some)
    }

    fn read_outcome_failure(
        &mut self,
        source: OutcomeStorage,
        layer: OutcomeLayer,
        node: BodyNodeId,
    ) -> Result<MirValueId, MirLoweringError> {
        if layer != OutcomeLayer::Fallible {
            return Err(MirLoweringError::InvalidDispatch(node));
        }
        let error = self.executable.types().builtin(BuiltinType::Error);
        let place = self.builder.add_place(
            MirPlaceRoot::Local(source.local),
            [MirProjection::new(
                MirProjectionKind::FallibleFailure,
                error,
            )],
            error,
        );
        self.append_value(
            error,
            MirOperationKind::Read {
                place,
                mode: MirReadMode::Move,
            },
        )
    }

    fn propagated_failure(
        &mut self,
        node: BodyNodeId,
        operand: OutcomeStorage,
        layer: OutcomeLayer,
        outer: &[OutcomeLayer],
    ) -> Result<MirValueId, MirLoweringError> {
        let result = self.item.signature().result();
        let (base, wrappers) = self.propagation_types(node, result, outer)?;
        let mut value = match layer {
            OutcomeLayer::Optional => self.append_value(
                base,
                MirOperationKind::Aggregate(MirAggregate::Optional(None)),
            )?,
            OutcomeLayer::Fallible => {
                let error = self.read_outcome_failure(operand, layer, node)?;
                self.append_value(
                    base,
                    MirOperationKind::Aggregate(MirAggregate::FallibleFailure(error)),
                )?
            }
        };
        for (layer, wrapper) in outer.iter().copied().zip(wrappers) {
            value = self.inject_outcome(node, wrapper, layer, Some(value))?;
        }
        Ok(value)
    }

    fn propagation_types(
        &self,
        node: BodyNodeId,
        result: TypeId,
        outer: &[OutcomeLayer],
    ) -> Result<(TypeId, Vec<TypeId>), MirLoweringError> {
        let mut current = result;
        let mut wrappers = Vec::with_capacity(outer.len());
        for layer in outer.iter().rev() {
            let payload = match (layer, self.executable.types().get(current)) {
                (OutcomeLayer::Optional, Some(TypeKind::Optional(payload)))
                | (OutcomeLayer::Fallible, Some(TypeKind::Fallible(payload))) => *payload,
                _ => return Err(MirLoweringError::InvalidDispatch(node)),
            };
            wrappers.push(current);
            current = payload;
        }
        wrappers.reverse();
        Ok((current, wrappers))
    }

    fn inject_outcome(
        &mut self,
        node: BodyNodeId,
        ty: TypeId,
        layer: OutcomeLayer,
        payload: Option<MirValueId>,
    ) -> Result<MirValueId, MirLoweringError> {
        let aggregate = match layer {
            OutcomeLayer::Optional => {
                MirAggregate::Optional(Some(payload.ok_or(MirLoweringError::InvalidOutcome(node))?))
            }
            OutcomeLayer::Fallible => MirAggregate::FallibleSuccess(payload),
        };
        self.append_value(ty, MirOperationKind::Aggregate(aggregate))
    }

    fn outcome_injection_payload(
        &mut self,
        payload: BodyNodeId,
    ) -> Result<Option<MirValueId>, MirLoweringError> {
        let source_type = self
            .body
            .nodes()
            .get(payload)
            .map(nocter_checking::CheckedNode::ty)
            .ok_or(MirLoweringError::UnknownNode(payload))?;
        let payload_type = self.concrete_type(source_type)?;
        if matches!(
            self.executable.types().get(payload_type),
            Some(TypeKind::Builtin(BuiltinType::Void))
        ) {
            self.lower_node(payload)?;
            Ok(None)
        } else {
            self.require_value(payload).map(Some)
        }
    }
}
