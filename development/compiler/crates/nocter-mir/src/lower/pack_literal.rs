use nocter_checking::SpreadMode;
use nocter_model::{
    BodyNodeId, BorrowCapability, BuiltinType, MirPlaceId, MirValueId, TypeId, TypeKind,
};
use nocter_target_program::{
    ExecutableArgumentPackPlan, ExecutablePackInput, ExecutablePackSegment, ExecutablePackSpread,
};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{
    MirBinaryOperation, MirCallPack, MirCallTarget, MirConstant, MirDestructionPlan,
    MirOperationKind, MirPackArgument, MirPackContribution, MirPackInput, MirPackNext,
    MirPackSegment, MirPackSpread,
};

#[derive(Clone, Copy)]
struct PreparedIterator {
    source: BodyNodeId,
    place: MirPlaceId,
    ty: TypeId,
}

enum PreparedSegment {
    Value {
        value: MirValueId,
        destruction: Option<MirDestructionPlan>,
    },
    KeyedValue {
        key: MirValueId,
        key_destruction: Option<MirDestructionPlan>,
        value: MirValueId,
        value_destruction: Option<MirDestructionPlan>,
    },
    Spread {
        iterator: PreparedIterator,
        plan: Box<ExecutablePackSpread>,
        destruction: Option<MirDestructionPlan>,
    },
}

impl FunctionLowerer<'_> {
    pub(super) fn lower_pack_literal(
        &mut self,
        node: BodyNodeId,
        ty: TypeId,
    ) -> Result<MirValueId, MirLoweringError> {
        let plan = self
            .item
            .body()
            .pack_literal(node)
            .cloned()
            .ok_or(MirLoweringError::InvalidDispatch(node))?;
        if plan.result() != ty {
            return Err(MirLoweringError::InvalidDispatch(node));
        }
        let allocation = self.lower_call_allocation(plan.allocation())?;
        let pack = self.lower_pack_argument(node, plan.input(), plan.segments())?;
        self.emit_pack_call(
            ty,
            MirCallTarget::Direct(plan.constructor()),
            [],
            MirCallPack::Prepared(pack),
            allocation,
        )
    }

    pub(super) fn lower_call_pack(
        &mut self,
        node: BodyNodeId,
    ) -> Result<MirCallPack, MirLoweringError> {
        let plan = self
            .item
            .body()
            .argument_pack(node)
            .cloned()
            .ok_or(MirLoweringError::InvalidDispatch(node))?;
        self.lower_executable_argument_pack(node, &plan)
    }

    fn lower_executable_argument_pack(
        &mut self,
        node: BodyNodeId,
        plan: &ExecutableArgumentPackPlan,
    ) -> Result<MirCallPack, MirLoweringError> {
        if plan.is_forwarded() {
            let input = self
                .item
                .signature()
                .pack()
                .ok_or(MirLoweringError::InvalidDispatch(node))?;
            if input.element() != plan.input().element() || input.next() != plan.input().next() {
                return Err(MirLoweringError::InvalidDispatch(node));
            }
            return Ok(MirCallPack::Forwarded(MirPackInput::new(
                input.element(),
                input.next(),
            )));
        }
        self.lower_pack_argument(node, plan.input(), plan.segments())
            .map(MirCallPack::Prepared)
    }

    fn lower_pack_argument(
        &mut self,
        node: BodyNodeId,
        input: ExecutablePackInput,
        segments: &[ExecutablePackSegment],
    ) -> Result<MirPackArgument, MirLoweringError> {
        let prepared = self.prepare_pack_segments(node, input, segments)?;
        let (length, remaining) = self.compute_pack_length(node, &prepared)?;
        let segments = self.finish_pack_segments(node, prepared, remaining)?;
        Ok(MirPackArgument::new(
            input.element(),
            input.next(),
            length,
            segments,
        ))
    }

    fn prepare_pack_segments(
        &mut self,
        owner: BodyNodeId,
        input: ExecutablePackInput,
        segments: &[ExecutablePackSegment],
    ) -> Result<Vec<PreparedSegment>, MirLoweringError> {
        segments
            .iter()
            .map(|segment| match segment {
                ExecutablePackSegment::Value {
                    source,
                    ty,
                    destruction,
                } => {
                    let value = self.require_value(*source)?;
                    if self.builder.value_type(value) != Some(*ty) || *ty != input.element() {
                        return Err(MirLoweringError::InvalidDispatch(owner));
                    }
                    Ok(PreparedSegment::Value {
                        value,
                        destruction: destruction
                            .as_ref()
                            .map(|plan| self.lower_deferred_destruction(owner, plan))
                            .transpose()?,
                    })
                }
                ExecutablePackSegment::Spread(spread) => {
                    let source = spread.iterator();
                    let value = self.require_value(source)?;
                    if self.builder.value_type(value) != Some(spread.iterator_type()) {
                        return Err(MirLoweringError::InvalidDispatch(owner));
                    }
                    let iterator = self.materialize_value_storage(source, value)?;
                    Ok(PreparedSegment::Spread {
                        iterator: PreparedIterator {
                            source,
                            place: iterator,
                            ty: spread.iterator_type(),
                        },
                        plan: Box::new(spread.clone()),
                        destruction: spread
                            .destruction()
                            .map(|plan| self.lower_deferred_destruction(owner, plan))
                            .transpose()?,
                    })
                }
                ExecutablePackSegment::KeyedValue {
                    key,
                    key_type,
                    key_destruction,
                    value,
                    value_type,
                    value_destruction,
                } => {
                    let key_value = self.require_value(*key)?;
                    let value_value = self.require_value(*value)?;
                    if self.builder.value_type(key_value) != Some(*key_type)
                        || self.builder.value_type(value_value) != Some(*value_type)
                        || !matches!(
                            self.executable.types().get(input.element()),
                            Some(TypeKind::PackEntry {
                                key: expected_key,
                                value: expected_value,
                            }) if expected_key == key_type && expected_value == value_type
                        )
                    {
                        return Err(MirLoweringError::InvalidDispatch(owner));
                    }
                    Ok(PreparedSegment::KeyedValue {
                        key: key_value,
                        key_destruction: key_destruction
                            .as_ref()
                            .map(|plan| self.lower_deferred_destruction(owner, plan))
                            .transpose()?,
                        value: value_value,
                        value_destruction: value_destruction
                            .as_ref()
                            .map(|plan| self.lower_deferred_destruction(owner, plan))
                            .transpose()?,
                    })
                }
            })
            .collect()
    }

    fn compute_pack_length(
        &mut self,
        owner: BodyNodeId,
        segments: &[PreparedSegment],
    ) -> Result<(MirValueId, Vec<Option<MirValueId>>), MirLoweringError> {
        let usize_ty = self.executable.types().builtin(BuiltinType::Usize);
        let fixed = segments
            .iter()
            .filter(|segment| {
                matches!(
                    segment,
                    PreparedSegment::Value { .. } | PreparedSegment::KeyedValue { .. }
                )
            })
            .count();
        let fixed = i128::try_from(fixed).map_err(|_| MirLoweringError::InvalidDispatch(owner))?;
        let mut length = self.append_value(
            usize_ty,
            MirOperationKind::Constant(MirConstant::Integer(fixed)),
        )?;
        let mut remaining = Vec::with_capacity(segments.len());
        for segment in segments {
            let PreparedSegment::Spread { iterator, plan, .. } = segment else {
                remaining.push(None);
                continue;
            };
            let count = self.invoke_pack_iterator_method(
                owner,
                iterator,
                plan.exact_size(),
                BorrowCapability::Readonly,
                usize_ty,
            )?;
            length = self.append_value(
                usize_ty,
                MirOperationKind::Binary {
                    operation: MirBinaryOperation::Add,
                    left: length,
                    right: count,
                },
            )?;
            remaining.push(Some(count));
        }
        Ok((length, remaining))
    }

    fn finish_pack_segments(
        &mut self,
        owner: BodyNodeId,
        segments: Vec<PreparedSegment>,
        remaining: Vec<Option<MirValueId>>,
    ) -> Result<Vec<MirPackSegment>, MirLoweringError> {
        segments
            .into_iter()
            .zip(remaining)
            .map(|(segment, remaining)| match segment {
                PreparedSegment::Value { value, destruction } => {
                    if remaining.is_some() {
                        return Err(MirLoweringError::InvalidDispatch(owner));
                    }
                    Ok(MirPackSegment::Value { value, destruction })
                }
                PreparedSegment::KeyedValue {
                    key,
                    key_destruction,
                    value,
                    value_destruction,
                } => {
                    if remaining.is_some() {
                        return Err(MirLoweringError::InvalidDispatch(owner));
                    }
                    Ok(MirPackSegment::KeyedValue {
                        key,
                        key_destruction,
                        value,
                        value_destruction,
                    })
                }
                PreparedSegment::Spread {
                    iterator,
                    plan,
                    destruction,
                } => {
                    let remaining = remaining.ok_or(MirLoweringError::InvalidDispatch(owner))?;
                    let (receiver, next_target, next_result) =
                        self.prepare_pack_next(owner, &iterator, &plan)?;
                    self.deactivate_value_storage(iterator.source)?;
                    Ok(MirPackSegment::Spread(MirPackSpread::new(
                        iterator.place,
                        remaining,
                        MirPackNext::new(receiver, next_target, next_result, plan.item()),
                        contribution_mode(self.executable.types(), &plan, owner)?,
                        destruction,
                    )))
                }
            })
            .collect()
    }

    fn prepare_pack_next(
        &mut self,
        owner: BodyNodeId,
        iterator: &PreparedIterator,
        plan: &ExecutablePackSpread,
    ) -> Result<(MirValueId, MirCallTarget, TypeId), MirLoweringError> {
        let invocation = self.invocation_plan(owner, plan.next())?;
        let signature = self.step_signature(&invocation.step)?;
        let Some(TypeKind::Optional(item)) = self.executable.types().get(signature.result()) else {
            return Err(MirLoweringError::InvalidDispatch(owner));
        };
        if *item != plan.item() {
            return Err(MirLoweringError::InvalidDispatch(owner));
        }
        let receiver =
            self.prepare_pack_receiver(owner, iterator, &invocation, BorrowCapability::ReadWrite)?;
        let target = self.step_target(owner, &invocation.step)?;
        Ok((receiver, target, signature.result()))
    }

    fn invoke_pack_iterator_method(
        &mut self,
        owner: BodyNodeId,
        iterator: &PreparedIterator,
        selection: &nocter_checking::StaticSelection,
        capability: BorrowCapability,
        result: TypeId,
    ) -> Result<MirValueId, MirLoweringError> {
        let invocation = self.invocation_plan(owner, selection)?;
        let signature = self.step_signature(&invocation.step)?;
        if signature.result() != result {
            return Err(MirLoweringError::InvalidDispatch(owner));
        }
        let receiver = self.prepare_pack_receiver(owner, iterator, &invocation, capability)?;
        self.emit_dispatch_step(owner, result, &invocation.step, [receiver])
    }

    fn prepare_pack_receiver(
        &mut self,
        owner: BodyNodeId,
        iterator: &PreparedIterator,
        invocation: &super::call::InvocationPlan,
        capability: BorrowCapability,
    ) -> Result<MirValueId, MirLoweringError> {
        let signature = self.step_signature(&invocation.step)?;
        let [target] = signature.parameters() else {
            return Err(MirLoweringError::InvalidDispatch(owner));
        };
        let receiver_ty = invocation.opaque_receiver.map_or(
            *target,
            nocter_target_program::ExecutableOpaqueReceiver::source,
        );
        if !matches!(
            self.executable.types().get(receiver_ty),
            Some(TypeKind::Borrow {
                capability: actual,
                referent,
            }) if *actual == capability && *referent == iterator.ty
        ) {
            return Err(MirLoweringError::InvalidDispatch(owner));
        }
        let receiver = self.borrow_place(iterator.place, capability, receiver_ty)?;
        if let Some(opaque) = invocation.opaque_receiver {
            self.lower_opaque_receiver(owner, iterator.source, receiver, opaque, *target)
        } else {
            Ok(receiver)
        }
    }
}

fn contribution_mode(
    types: &nocter_model::TypeStore,
    plan: &ExecutablePackSpread,
    owner: BodyNodeId,
) -> Result<MirPackContribution, MirLoweringError> {
    match plan.mode() {
        SpreadMode::Copy => matches!(
            types.get(plan.item()),
            Some(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            }) if *referent == plan.contribution()
        )
        .then_some(MirPackContribution::CopyBorrowed)
        .ok_or(MirLoweringError::InvalidDispatch(owner)),
        SpreadMode::Borrow | SpreadMode::Move if plan.item() == plan.contribution() => {
            Ok(MirPackContribution::Direct)
        }
        SpreadMode::Borrow | SpreadMode::Move => Err(MirLoweringError::InvalidDispatch(owner)),
    }
}
