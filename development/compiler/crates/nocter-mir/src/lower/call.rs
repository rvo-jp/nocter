use nocter_checking::{CallTarget, CheckedCall, ResolvedPrimitiveDispatch, StaticSelection};
use nocter_model::{BodyNodeId, BuiltinType, MirValueId, PlaceId, TypeId, TypeKind};
use nocter_target_program::{
    ExecutableDispatchPlan, ExecutableDispatchStep, ExecutableOpaqueReceiver,
    ExecutablePrimitiveCall, ExecutablePrimitiveDependency, ExecutableSignature,
};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{
    MirCall, MirCallAllocation, MirCallSignature, MirCallTarget, MirOperationKind, MirPackArgument,
    MirStructuralCall, MirTerminator,
};

pub(super) struct InvocationPlan {
    pub(super) step: ExecutableDispatchStep,
    pub(super) opaque_receiver: Option<ExecutableOpaqueReceiver>,
}

impl FunctionLowerer<'_> {
    pub(super) fn invocation_plan(
        &self,
        node: BodyNodeId,
        selection: &StaticSelection,
    ) -> Result<InvocationPlan, MirLoweringError> {
        match self
            .item
            .body()
            .dispatch(selection)
            .ok_or(MirLoweringError::InvalidDispatch(node))?
        {
            ExecutableDispatchPlan::Invocation(step) => Ok(InvocationPlan {
                step: step.clone(),
                opaque_receiver: None,
            }),
            ExecutableDispatchPlan::OpaqueInvocation {
                receiver,
                operation,
            } => Ok(InvocationPlan {
                step: operation.clone(),
                opaque_receiver: Some(*receiver),
            }),
            ExecutableDispatchPlan::Comparison { .. } | ExecutableDispatchPlan::Index { .. } => {
                Err(MirLoweringError::InvalidDispatch(node))
            }
        }
    }

    pub(super) fn invocation_step(
        &self,
        node: BodyNodeId,
        selection: &StaticSelection,
    ) -> Result<ExecutableDispatchStep, MirLoweringError> {
        let plan = self.invocation_plan(node, selection)?;
        if plan.opaque_receiver.is_some() {
            return Err(MirLoweringError::InvalidDispatch(node));
        }
        Ok(plan.step)
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
        let InvocationPlan {
            step,
            opaque_receiver,
        } = self.invocation_plan(node, selection)?;
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
            let source = opaque_receiver.map_or(expected, ExecutableOpaqueReceiver::source);
            let value = self.lower_receiver(node, receiver, source)?;
            arguments.push(if let Some(opaque) = opaque_receiver {
                self.lower_opaque_receiver(node, receiver.value(), value, opaque, expected)?
            } else {
                value
            });
        } else if opaque_receiver.is_some() {
            return Err(MirLoweringError::InvalidDispatch(node));
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
        let target = self.step_target(node, step)?;
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
        let target = self.step_target(node, step)?;
        self.emit_call_with_allocation(ty, target, arguments, allocation)
    }

    pub(super) fn emit_place_dispatch_step(
        &mut self,
        place: PlaceId,
        ty: TypeId,
        step: &ExecutableDispatchStep,
        arguments: impl Into<Box<[MirValueId]>>,
    ) -> Result<MirValueId, MirLoweringError> {
        let target = Self::step_target_for_place(place, step)?;
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

    pub(super) fn emit_pack_call(
        &mut self,
        ty: TypeId,
        target: MirCallTarget,
        pack: MirPackArgument,
        allocation: MirCallAllocation,
    ) -> Result<MirValueId, MirLoweringError> {
        let value = self.append_value(
            ty,
            MirOperationKind::Call(MirCall::with_pack(target, pack, allocation)),
        )?;
        self.finish_call(ty, value)
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
        self.finish_call(ty, value)
    }

    fn finish_call(
        &mut self,
        ty: TypeId,
        value: MirValueId,
    ) -> Result<MirValueId, MirLoweringError> {
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

    pub(super) fn step_target(
        &self,
        owner: BodyNodeId,
        step: &ExecutableDispatchStep,
    ) -> Result<MirCallTarget, MirLoweringError> {
        match step {
            ExecutableDispatchStep::Direct(callee) => Ok(MirCallTarget::Direct(*callee)),
            ExecutableDispatchStep::StandardPrimitive(call) => self.primitive_target(owner, call),
            ExecutableDispatchStep::StructuralPrimitive(primitive) => {
                Ok(MirCallTarget::Structural(structural_target(primitive)))
            }
            ExecutableDispatchStep::CallableValue(_) => {
                Err(MirLoweringError::InvalidDispatch(owner))
            }
        }
    }

    fn step_target_for_place(
        place: PlaceId,
        step: &ExecutableDispatchStep,
    ) -> Result<MirCallTarget, MirLoweringError> {
        match step {
            ExecutableDispatchStep::Direct(callee) => Ok(MirCallTarget::Direct(*callee)),
            ExecutableDispatchStep::StandardPrimitive(call)
                if matches!(call.dependency(), ExecutablePrimitiveDependency::None) =>
            {
                Ok(primitive_target(call, crate::MirPrimitiveDependency::None))
            }
            ExecutableDispatchStep::StructuralPrimitive(primitive) => {
                Ok(MirCallTarget::Structural(structural_target(primitive)))
            }
            ExecutableDispatchStep::StandardPrimitive(_)
            | ExecutableDispatchStep::CallableValue(_) => {
                Err(MirLoweringError::InvalidPlaceDispatch(place))
            }
        }
    }

    fn primitive_target(
        &self,
        owner: BodyNodeId,
        call: &ExecutablePrimitiveCall,
    ) -> Result<MirCallTarget, MirLoweringError> {
        let dependency = match call.dependency() {
            ExecutablePrimitiveDependency::None => crate::MirPrimitiveDependency::None,
            ExecutablePrimitiveDependency::Destruction { subject, plan } => {
                crate::MirPrimitiveDependency::Destruction {
                    subject: *subject,
                    plan: plan
                        .as_deref()
                        .map(|plan| self.lower_deferred_destruction(owner, plan))
                        .transpose()?
                        .map(Box::new),
                }
            }
        };
        Ok(primitive_target(call, dependency))
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

fn primitive_target(
    call: &ExecutablePrimitiveCall,
    dependency: crate::MirPrimitiveDependency,
) -> MirCallTarget {
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
        dependency,
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
