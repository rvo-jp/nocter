use nocter_checking::TypedAsyncIteration;
use nocter_model::{BodyNodeId, LocalBindingId, LoopId, TypeId, TypeKind};

use super::MirLoweringError;
use super::collection_loop::IterationInvocation;
use super::function::FunctionLowerer;
use super::loop_control::LoopTargets;
use crate::{
    MirBranchTarget, MirLocalKind, MirOperationKind, MirPlaceRoot, MirSwitchCase, MirSwitchSubject,
    MirSwitchValue, MirTerminator,
};

struct AsyncIterationContract {
    invocation: IterationInvocation,
    deferred_output: TypeId,
    optional: TypeId,
}

impl FunctionLowerer<'_> {
    pub(super) fn lower_async_collection_loop(
        &mut self,
        node: BodyNodeId,
        loop_: LoopId,
        binding: LocalBindingId,
        iteration: &TypedAsyncIteration,
        body: BodyNodeId,
    ) -> Result<(), MirLoweringError> {
        let AsyncIterationContract {
            invocation:
                IterationInvocation {
                    iterator_place,
                    item: item_ty,
                    result: future_ty,
                    receiver: receiver_ty,
                    target_receiver,
                    capability,
                    opaque_receiver,
                    step,
                },
            deferred_output,
            optional,
        } = self.prepare_async_iteration_contract(node, loop_, iteration)?;

        let optional_local = self
            .builder
            .add_local(optional, MirLocalKind::Temporary, true);
        let optional_place =
            self.builder
                .add_place(MirPlaceRoot::Local(optional_local), [], optional);

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
        let future = self.emit_dispatch_step(node, future_ty, &step, [receiver])?;
        let cancellation = self.lower_cancellation_actions_for_value(node, future)?;
        let outcome = self.lower_await_value(node, deferred_output, future, cancellation)?;
        let optional_value = self.lower_fallible_value(node, outcome, iteration.failure_outer())?;
        self.append_effect(MirOperationKind::Initialize {
            destination: optional_place,
            value: optional_value,
        })?;

        let ready = self
            .current
            .take()
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        self.builder.terminate(
            ready,
            MirTerminator::Switch {
                subject: MirSwitchSubject::Place(optional_place),
                cases: Box::new([MirSwitchCase::new(
                    MirSwitchValue::OptionalPresent,
                    MirBranchTarget::new(body_block, []),
                )]),
                fallback: MirBranchTarget::new(exit, []),
            },
        )?;

        self.current = Some(body_block);
        self.bind_optional_iteration_item(optional_local, item_ty, binding)?;
        self.lower_node(body)?;
        self.finish_loop_iteration(header)?;
        self.leave_loop(loop_)?;
        self.current = Some(exit);
        Ok(())
    }

    fn prepare_async_iteration_contract(
        &mut self,
        node: BodyNodeId,
        loop_: LoopId,
        iteration: &TypedAsyncIteration,
    ) -> Result<AsyncIterationContract, MirLoweringError> {
        let invocation = self.prepare_iteration_invocation(node, loop_, iteration.step())?;
        let Some(TypeKind::Future(deferred_output)) =
            self.executable.types().get(invocation.result)
        else {
            return Err(MirLoweringError::InvalidLoop(loop_));
        };
        let deferred_output = *deferred_output;
        let Some(TypeKind::Fallible(optional)) = self.executable.types().get(deferred_output)
        else {
            return Err(MirLoweringError::InvalidLoop(loop_));
        };
        let optional = *optional;
        if !matches!(
            self.executable.types().get(optional),
            Some(TypeKind::Optional(payload)) if *payload == invocation.item
        ) {
            return Err(MirLoweringError::InvalidLoop(loop_));
        }
        Ok(AsyncIterationContract {
            invocation,
            deferred_output,
            optional,
        })
    }
}
