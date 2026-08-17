use nocter_checking::TypedIteration;
use nocter_model::{
    BodyNodeId, BorrowCapability, LocalBindingId, LoopId, MirPlaceId, TypeId, TypeKind,
};
use nocter_target_program::ExecutableDispatchStep;

use super::MirLoweringError;
use super::function::FunctionLowerer;
use super::loop_control::LoopTargets;
use crate::{
    MirBranchTarget, MirLocalKind, MirOperationKind, MirPlaceRoot, MirProjection,
    MirProjectionKind, MirReadMode, MirSwitchCase, MirSwitchSubject, MirSwitchValue, MirTerminator,
};

struct IterationContract {
    iterator_place: MirPlaceId,
    item: TypeId,
    next: TypeId,
    receiver: TypeId,
    capability: BorrowCapability,
    step: ExecutableDispatchStep,
}

impl FunctionLowerer<'_> {
    pub(super) fn lower_collection_loop(
        &mut self,
        node: BodyNodeId,
        loop_: LoopId,
        binding: LocalBindingId,
        iteration: &TypedIteration,
        body: BodyNodeId,
    ) -> Result<(), MirLoweringError> {
        let contract = self.prepare_iteration_contract(node, loop_, iteration)?;
        let IterationContract {
            iterator_place,
            item: item_ty,
            next: next_ty,
            receiver: receiver_ty,
            capability,
            step,
        } = contract;
        let next_local = self
            .builder
            .add_local(next_ty, MirLocalKind::Temporary, true);
        let next_place = self
            .builder
            .add_place(MirPlaceRoot::Local(next_local), [], next_ty);

        let source = self
            .current
            .take()
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        let (header, _) = self.builder.create_block([]);
        let (body_block, _) = self.builder.create_block([]);
        let (exit, _) = self.builder.create_block([]);
        self.builder.terminate(
            source,
            MirTerminator::Goto(MirBranchTarget::new(header, [])),
        )?;
        self.enter_loop(
            loop_,
            LoopTargets {
                continue_: header,
                break_: Some(exit),
            },
        )?;

        self.current = Some(header);
        let receiver = self.borrow_place(iterator_place, capability, receiver_ty)?;
        let next = self.emit_dispatch_step(node, next_ty, &step, [receiver])?;
        self.append_effect(MirOperationKind::Initialize {
            destination: next_place,
            value: next,
        })?;
        let header = self
            .current
            .take()
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        self.builder.terminate(
            header,
            MirTerminator::Switch {
                subject: MirSwitchSubject::Place(next_place),
                cases: Box::new([MirSwitchCase::new(
                    MirSwitchValue::OptionalPresent,
                    MirBranchTarget::new(body_block, []),
                )]),
                fallback: MirBranchTarget::new(exit, []),
            },
        )?;

        self.current = Some(body_block);
        let item_place = self.builder.add_place(
            MirPlaceRoot::Local(next_local),
            [MirProjection::new(
                MirProjectionKind::OptionalPayload,
                item_ty,
            )],
            item_ty,
        );
        let item = self.append_value(
            item_ty,
            MirOperationKind::Read {
                place: item_place,
                mode: MirReadMode::Move,
            },
        )?;
        let binding_local = self.ensure_local(binding)?;
        let binding_place = self
            .builder
            .add_place(MirPlaceRoot::Local(binding_local), [], item_ty);
        self.append_effect(MirOperationKind::Initialize {
            destination: binding_place,
            value: item,
        })?;
        self.mark_binding_initialized(binding)?;
        self.lower_node(body)?;
        self.finish_loop_iteration(header)?;
        self.leave_loop(loop_)?;
        self.current = Some(exit);
        Ok(())
    }

    fn prepare_iteration_contract(
        &mut self,
        node: BodyNodeId,
        loop_: LoopId,
        iteration: &TypedIteration,
    ) -> Result<IterationContract, MirLoweringError> {
        let iterator = self.require_value(iteration.iterator())?;
        let iterator_place = self.materialize_value_storage(iteration.iterator(), iterator)?;
        let iterator_ty = self
            .builder
            .place(iterator_place)
            .map(crate::MirPlace::ty)
            .ok_or(MirLoweringError::InvalidLoop(loop_))?;
        let item_ty = self.concrete_type(iteration.item())?;
        let step = self.invocation_step(node, iteration.next())?;
        let signature = self.step_signature(&step)?;
        let [receiver_ty] = signature.parameters() else {
            return Err(MirLoweringError::InvalidLoop(loop_));
        };
        let Some(TypeKind::Borrow {
            capability,
            referent,
        }) = self.executable.types().get(*receiver_ty)
        else {
            return Err(MirLoweringError::InvalidLoop(loop_));
        };
        if *referent != iterator_ty
            || !matches!(
                self.executable.types().get(signature.result()),
                Some(TypeKind::Optional(payload)) if *payload == item_ty
            )
        {
            return Err(MirLoweringError::InvalidLoop(loop_));
        }
        Ok(IterationContract {
            iterator_place,
            item: item_ty,
            next: signature.result(),
            receiver: *receiver_ty,
            capability: *capability,
            step,
        })
    }
}
