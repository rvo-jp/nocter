use nocter_checking::{CallTarget, CheckedCall, ResolvedPrimitiveDispatch, StaticSelection};
use nocter_model::{BodyNodeId, BuiltinType, MirValueId, PlaceId, TypeId, TypeKind};
use nocter_target_program::{
    ExecutableDispatchPlan, ExecutableDispatchStep, ExecutablePrimitiveCall, ExecutableSignature,
};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{
    MirCall, MirCallAllocation, MirCallSignature, MirCallTarget, MirOperationKind,
    MirStructuralCall, MirTerminator,
};

impl FunctionLowerer<'_> {
    pub(super) fn invocation_step(
        &self,
        node: BodyNodeId,
        selection: &StaticSelection,
    ) -> Result<ExecutableDispatchStep, MirLoweringError> {
        let plan = self
            .item
            .body()
            .dispatch(selection)
            .ok_or(MirLoweringError::InvalidDispatch(node))?;
        let ExecutableDispatchPlan::Invocation(step) = plan else {
            return Err(MirLoweringError::InvalidDispatch(node));
        };
        Ok(step.clone())
    }

    pub(super) fn lower_call(
        &mut self,
        node: BodyNodeId,
        ty: TypeId,
        call: &CheckedCall,
    ) -> Result<MirValueId, MirLoweringError> {
        let CallTarget::Static(selection) = call.target() else {
            return match call.target() {
                CallTarget::ClosureValue { .. } => self.lower_closure_call(node, ty, call),
                CallTarget::CallableValue { .. } => self.lower_callable_value_call(node, ty, call),
                CallTarget::Static(_) => unreachable!("matched above"),
            };
        };
        let step = {
            let plan = self
                .item
                .body()
                .dispatch(selection)
                .ok_or(MirLoweringError::InvalidDispatch(node))?;
            let ExecutableDispatchPlan::Invocation(step) = plan else {
                return Err(MirLoweringError::UnsupportedOperation(node));
            };
            step.clone()
        };
        let signature = self.step_signature(&step)?;
        if signature.result() != ty {
            return Err(MirLoweringError::InvalidDispatch(node));
        }
        let mut arguments =
            Vec::with_capacity(call.arguments().len() + usize::from(call.receiver().is_some()));
        if let Some(receiver) = call.receiver() {
            let expected = signature
                .parameters()
                .first()
                .copied()
                .ok_or(MirLoweringError::InvalidDispatch(node))?;
            arguments.push(self.lower_receiver(node, receiver, expected)?);
        }
        arguments.extend(
            call.arguments()
                .iter()
                .map(|argument| self.require_value(*argument))
                .collect::<Result<Vec<_>, _>>()?,
        );
        if arguments.len() != signature.parameters().len() {
            return Err(MirLoweringError::InvalidDispatch(node));
        }
        self.emit_dispatch_step(node, ty, &step, arguments)
    }

    pub(super) fn emit_dispatch_step(
        &mut self,
        node: BodyNodeId,
        ty: TypeId,
        step: &ExecutableDispatchStep,
        arguments: impl Into<Box<[MirValueId]>>,
    ) -> Result<MirValueId, MirLoweringError> {
        let target = step_target(step).ok_or(MirLoweringError::InvalidDispatch(node))?;
        self.emit_call(ty, target, arguments)
    }

    pub(super) fn emit_dispatch_step_with_allocation(
        &mut self,
        node: BodyNodeId,
        ty: TypeId,
        step: &ExecutableDispatchStep,
        arguments: impl Into<Box<[MirValueId]>>,
        allocation: MirCallAllocation,
    ) -> Result<MirValueId, MirLoweringError> {
        let target = step_target(step).ok_or(MirLoweringError::InvalidDispatch(node))?;
        self.emit_call_with_allocation(ty, target, arguments, allocation)
    }

    pub(super) fn emit_place_dispatch_step(
        &mut self,
        place: PlaceId,
        ty: TypeId,
        step: &ExecutableDispatchStep,
        arguments: impl Into<Box<[MirValueId]>>,
    ) -> Result<MirValueId, MirLoweringError> {
        let target = step_target(step).ok_or(MirLoweringError::InvalidPlaceDispatch(place))?;
        self.emit_call(ty, target, arguments)
    }

    pub(super) fn emit_call(
        &mut self,
        ty: TypeId,
        target: MirCallTarget,
        arguments: impl Into<Box<[MirValueId]>>,
    ) -> Result<MirValueId, MirLoweringError> {
        self.emit_call_with_allocation(ty, target, arguments, MirCallAllocation::Inherit)
    }

    fn emit_call_with_allocation(
        &mut self,
        ty: TypeId,
        target: MirCallTarget,
        arguments: impl Into<Box<[MirValueId]>>,
        allocation: MirCallAllocation,
    ) -> Result<MirValueId, MirLoweringError> {
        let value = self.append_value(
            ty,
            MirOperationKind::Call(MirCall::with_allocation(target, arguments, allocation)),
        )?;
        if self.executable.types().get(ty) == Some(&TypeKind::Builtin(BuiltinType::Never)) {
            let block = self.current.ok_or(MirLoweringError::MissingCurrentBlock)?;
            self.builder.terminate(block, MirTerminator::Unreachable)?;
            self.current = None;
        }
        Ok(value)
    }

    pub(super) fn step_signature(
        &self,
        step: &ExecutableDispatchStep,
    ) -> Result<MirCallSignature, MirLoweringError> {
        Ok(match step {
            ExecutableDispatchStep::Direct(item) => {
                let signature = self
                    .executable
                    .items()
                    .get(*item)
                    .ok_or(MirLoweringError::InvalidDispatch(self.item.body().root()))?
                    .signature();
                executable_signature(signature)
            }
            ExecutableDispatchStep::StandardPrimitive(call) => {
                executable_signature(call.signature())
            }
            ExecutableDispatchStep::StructuralPrimitive(primitive) => {
                structural_signature(self.executable, primitive)
            }
            ExecutableDispatchStep::CallableValue(invocation) => {
                let contract = invocation.contract();
                MirCallSignature::new(contract.parameters().to_vec(), contract.result())
            }
        })
    }
}

fn executable_signature(signature: &ExecutableSignature) -> MirCallSignature {
    MirCallSignature::new(
        signature
            .inputs()
            .iter()
            .map(|input| input.ty())
            .collect::<Vec<_>>(),
        signature.result(),
    )
}

fn structural_signature(
    executable: &nocter_target_program::ExecutableProgram,
    primitive: &ResolvedPrimitiveDispatch,
) -> MirCallSignature {
    match primitive {
        ResolvedPrimitiveDispatch::Equality { operand, .. }
        | ResolvedPrimitiveDispatch::Ordering { operand, .. } => MirCallSignature::new(
            [*operand, *operand],
            executable.types().builtin(BuiltinType::Bool),
        ),
        ResolvedPrimitiveDispatch::Index {
            receiver,
            index,
            result,
            ..
        } => MirCallSignature::new([*receiver, *index], *result),
        ResolvedPrimitiveDispatch::BorrowWeakening { source, target } => {
            MirCallSignature::new([*source], *target)
        }
    }
}

fn step_target(step: &ExecutableDispatchStep) -> Option<MirCallTarget> {
    match step {
        ExecutableDispatchStep::Direct(callee) => Some(MirCallTarget::Direct(*callee)),
        ExecutableDispatchStep::StandardPrimitive(call) => Some(primitive_target(call)),
        ExecutableDispatchStep::StructuralPrimitive(primitive) => {
            Some(MirCallTarget::Structural(structural_target(primitive)))
        }
        ExecutableDispatchStep::CallableValue(_) => None,
    }
}

fn primitive_target(call: &ExecutablePrimitiveCall) -> MirCallTarget {
    MirCallTarget::StandardPrimitive {
        role: call.role(),
        type_arguments: call
            .generic_arguments()
            .as_slice()
            .iter()
            .map(|argument| argument.ty())
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        signature: executable_signature(call.signature()),
    }
}

const fn structural_target(primitive: &ResolvedPrimitiveDispatch) -> MirStructuralCall {
    match primitive {
        ResolvedPrimitiveDispatch::Equality { subject, operand } => MirStructuralCall::Equality {
            subject: *subject,
            operand: *operand,
        },
        ResolvedPrimitiveDispatch::Ordering { subject, operand } => MirStructuralCall::Ordering {
            subject: *subject,
            operand: *operand,
        },
        ResolvedPrimitiveDispatch::Index {
            capability,
            container,
            receiver,
            index,
            result,
        } => MirStructuralCall::Index {
            capability: *capability,
            container: *container,
            receiver: *receiver,
            index: *index,
            result: *result,
        },
        ResolvedPrimitiveDispatch::BorrowWeakening { source, target } => {
            MirStructuralCall::BorrowWeakening {
                source: *source,
                target: *target,
            }
        }
    }
}
