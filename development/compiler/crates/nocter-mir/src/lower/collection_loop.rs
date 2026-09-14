use nocter_checking::{TypedIteration, TypedIterationStep};
use nocter_model::{
    BodyNodeId, BorrowCapability, LocalBindingId, LoopId, MirPlaceId, TypeId, TypeKind,
};
use nocter_target_program::{ExecutableDispatchStep, ExecutableOpaqueReceiver};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use super::loop_control::LoopTargets;
use crate::{
    MirBranchTarget, MirLocalKind, MirOperationKind, MirPlaceRoot, MirSwitchCase, MirSwitchSubject,
    MirSwitchValue, MirTerminator,
};

pub(super) struct IterationInvocation {
    pub(super) iterator_place: MirPlaceId,
    pub(super) item: TypeId,
    pub(super) result: TypeId,
    pub(super) receiver: TypeId,
    pub(super) target_receiver: TypeId,
    pub(super) capability: BorrowCapability,
    pub(super) opaque_receiver: Option<ExecutableOpaqueReceiver>,
    pub(super) step: ExecutableDispatchStep,
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
        let contract = self.prepare_iteration_invocation(node, loop_, iteration.step())?;
        let IterationInvocation {
            iterator_place,
            item: item_ty,
            result: next_ty,
            receiver: receiver_ty,
            target_receiver,
            capability,
            opaque_receiver,
            step,
        } = contract;
        if !matches!(
            self.executable.types().get(next_ty),
            Some(TypeKind::Optional(payload)) if *payload == item_ty
        ) {
            return Err(MirLoweringError::InvalidLoop(loop_));
        }
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
        let receiver = if let Some(opaque) = opaque_receiver {
            self.lower_opaque_receiver(
                node,
                iteration.iterator(),
                receiver,
                opaque,
                target_receiver,
            )?
        } else {
            receiver
        };
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
        self.bind_optional_iteration_item(next_local, item_ty, binding)?;
        self.lower_node(body)?;
        self.finish_loop_iteration(header)?;
        self.leave_loop(loop_)?;
        self.current = Some(exit);
        Ok(())
    }

    pub(super) fn prepare_iteration_invocation(
        &mut self,
        node: BodyNodeId,
        loop_: LoopId,
        iteration: &TypedIterationStep,
    ) -> Result<IterationInvocation, MirLoweringError> {
        let iterator = self.require_value(iteration.iterator())?;
        let iterator_place = self.materialize_value_storage(iteration.iterator(), iterator)?;
        let iterator_ty = self
            .builder
            .place(iterator_place)
            .map(crate::MirPlace::ty)
            .ok_or(MirLoweringError::InvalidLoop(loop_))?;
        let item_ty = self.concrete_type(iteration.item())?;
        let invocation = self.invocation_plan(node, iteration.next())?;
        let step = invocation.step;
        let signature = self.step_signature(&step)?;
        let [target_receiver] = signature.parameters() else {
            return Err(MirLoweringError::InvalidLoop(loop_));
        };
        let receiver_ty = invocation
            .opaque_receiver
            .map_or(*target_receiver, ExecutableOpaqueReceiver::source);
        if invocation
            .opaque_receiver
            .is_some_and(|opaque| opaque.target() != *target_receiver)
        {
            return Err(MirLoweringError::InvalidLoop(loop_));
        }
        let Some(TypeKind::Borrow {
            capability,
            referent,
        }) = self.executable.types().get(receiver_ty)
        else {
            return Err(MirLoweringError::InvalidLoop(loop_));
        };
        if *referent != iterator_ty {
            return Err(MirLoweringError::InvalidLoop(loop_));
        }
        Ok(IterationInvocation {
            iterator_place,
            item: item_ty,
            result: signature.result(),
            receiver: receiver_ty,
            target_receiver: *target_receiver,
            capability: *capability,
            opaque_receiver: invocation.opaque_receiver,
            step,
        })
    }
}
