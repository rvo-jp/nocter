use std::collections::BTreeSet;

use crate::effect::MachineOperationEffect;
use crate::identity::MachineId;
use crate::{
    MachineAddress, MachineAddressExtent, MachineAddressRoot, MachineAddressStep, MachineAggregate,
    MachineAggregateWrite, MachineAsyncFrame, MachineBlock, MachineBranchTarget, MachineCall,
    MachineCallTarget, MachineCancellationAction, MachineErasedCallable, MachineFrameField,
    MachineFunctionExecution, MachineIndex, MachineIndexBorrow, MachineInitialAsyncState,
    MachineOperation, MachineOperationId, MachineOperationKind, MachinePack, MachinePackSegment,
    MachinePackSpread, MachineSuspensionState, MachineSwitchCase, MachineTerminator, MachineValue,
    MachineValueDefinition, MachineValueId,
};

use super::{MachineOptimizationError, MachineOptimizationReport};

pub(super) fn eliminate(
    draft: &mut crate::program::MachineBodyDraft,
    execution: &mut MachineFunctionExecution,
    report: &mut MachineOptimizationReport,
) -> Result<(), MachineOptimizationError> {
    let retention = Retention::build(draft, execution)?;
    if retention.operations.len() == draft.operations.len()
        && retention.values.len() == draft.values.len()
    {
        return Ok(());
    }
    let remap = DenseRemap::new(draft, &retention);
    let old_operation_count = draft.operations.len();
    let old_value_count = draft.values.len();

    draft.operations = std::mem::take(&mut draft.operations)
        .into_iter()
        .enumerate()
        .filter(|(index, _)| {
            retention
                .operations
                .contains(&MachineOperationId::new(*index))
        })
        .map(|(_, operation)| remap_operation(&operation, &remap))
        .collect::<Result<Vec<_>, _>>()?;
    draft.values = std::mem::take(&mut draft.values)
        .into_iter()
        .enumerate()
        .filter(|(index, _)| retention.values.contains(&MachineValueId::new(*index)))
        .map(|(_, value)| remap_value(value, &remap))
        .collect::<Result<Vec<_>, _>>()?;
    remap_addresses(&mut draft.addresses, &remap)?;
    remap_packs(&mut draft.packs, &remap)?;
    remap_blocks(&mut draft.blocks, &remap)?;
    remap_execution(execution, &remap)?;

    report.operations_removed += old_operation_count - draft.operations.len();
    report.values_removed += old_value_count - draft.values.len();
    Ok(())
}

#[derive(Default)]
struct Retention {
    operations: BTreeSet<MachineOperationId>,
    values: BTreeSet<MachineValueId>,
}

impl Retention {
    fn build(
        draft: &crate::program::MachineBodyDraft,
        execution: &MachineFunctionExecution,
    ) -> Result<Self, MachineOptimizationError> {
        let mut retained = Self::default();
        for (index, operation) in draft.operations.iter().enumerate() {
            if operation.kind().effect() != MachineOperationEffect::Pure {
                retained.operations.insert(MachineOperationId::new(index));
            }
        }
        for block in &draft.blocks {
            for parameter in block.parameters() {
                retained.mark_value(draft, *parameter)?;
            }
            retained.mark_terminator(draft, block.terminator())?;
        }
        for address in &draft.addresses {
            retained.mark_address(draft, address)?;
        }
        for pack in &draft.packs {
            retained.mark_pack(draft, pack)?;
        }
        retained.mark_execution(draft, execution)?;

        let mut visited_operations = BTreeSet::new();
        let mut visited_values = BTreeSet::new();
        loop {
            let pending_operations = retained
                .operations
                .difference(&visited_operations)
                .copied()
                .collect::<Vec<_>>();
            let pending_values = retained
                .values
                .difference(&visited_values)
                .copied()
                .collect::<Vec<_>>();
            if pending_operations.is_empty() && pending_values.is_empty() {
                break;
            }
            for operation_id in pending_operations {
                visited_operations.insert(operation_id);
                let operation = draft
                    .operations
                    .get(operation_id.index())
                    .ok_or(MachineOptimizationError::UnknownOperation(operation_id))?;
                if let Some(result) = operation.result() {
                    retained.mark_value(draft, result)?;
                }
                retained.mark_operation_inputs(draft, operation.kind())?;
            }
            for value_id in pending_values {
                visited_values.insert(value_id);
                let value = draft
                    .values
                    .get(value_id.index())
                    .ok_or(MachineOptimizationError::UnknownValue(value_id))?;
                if let MachineValueDefinition::Operation(operation) = value.definition() {
                    retained.mark_operation(draft, operation)?;
                }
            }
        }
        Ok(retained)
    }

    fn mark_operation(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        operation: MachineOperationId,
    ) -> Result<(), MachineOptimizationError> {
        if draft.operations.get(operation.index()).is_none() {
            return Err(MachineOptimizationError::UnknownOperation(operation));
        }
        self.operations.insert(operation);
        Ok(())
    }

    fn mark_value(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        value: MachineValueId,
    ) -> Result<(), MachineOptimizationError> {
        if draft.values.get(value.index()).is_none() {
            return Err(MachineOptimizationError::UnknownValue(value));
        }
        self.values.insert(value);
        Ok(())
    }

    fn mark_values(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        values: impl IntoIterator<Item = MachineValueId>,
    ) -> Result<(), MachineOptimizationError> {
        for value in values {
            self.mark_value(draft, value)?;
        }
        Ok(())
    }

    fn mark_operation_inputs(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        operation: &MachineOperationKind,
    ) -> Result<(), MachineOptimizationError> {
        match operation {
            MachineOperationKind::Store { value, .. }
            | MachineOperationKind::Unary { operand: value, .. }
            | MachineOperationKind::NumericConversion { operand: value }
            | MachineOperationKind::BorrowWeakening { source: value }
            | MachineOperationKind::CreateRegion { parent: value, .. } => {
                self.mark_value(draft, *value)?;
            }
            MachineOperationKind::Binary { left, right, .. }
            | MachineOperationKind::ReleaseMappedStorage {
                pointer: left,
                bytes: right,
            } => self.mark_values(draft, [*left, *right])?,
            MachineOperationKind::Comparison(comparison) => {
                self.mark_values(draft, [comparison.left(), comparison.right()])?;
            }
            MachineOperationKind::IndexBorrow(index) => {
                self.mark_values(draft, [index.receiver(), index.index()])?;
            }
            MachineOperationKind::Aggregate(aggregate) => {
                for write in aggregate.writes() {
                    if let MachineAggregateWrite::Value { value, .. }
                    | MachineAggregateWrite::RepeatedValue { value, .. } = write
                    {
                        self.mark_value(draft, *value)?;
                    }
                }
            }
            MachineOperationKind::EraseCallable(erasure) => {
                self.mark_value(draft, erasure.environment())?;
            }
            MachineOperationKind::Call(call) => {
                self.mark_values(draft, call.arguments().iter().copied())?;
            }
            MachineOperationKind::Constant(_)
            | MachineOperationKind::Load { .. }
            | MachineOperationKind::AddressOf { .. }
            | MachineOperationKind::InvokeDrop { .. }
            | MachineOperationKind::ReportError { .. }
            | MachineOperationKind::ReleaseError { .. }
            | MachineOperationKind::ReleaseComputation { .. }
            | MachineOperationKind::ReleaseErasedCallable { .. }
            | MachineOperationKind::DriveComputation { .. }
            | MachineOperationKind::ReleaseRegion { .. }
            | MachineOperationKind::SetDropFlag { .. }
            | MachineOperationKind::PackLength
            | MachineOperationKind::PackNext
            | MachineOperationKind::DestroyPack => {}
        }
        Ok(())
    }

    fn mark_address(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        address: &MachineAddress,
    ) -> Result<(), MachineOptimizationError> {
        if let MachineAddressRoot::Pointer { value } | MachineAddressRoot::View { value, .. } =
            address.root()
        {
            self.mark_value(draft, value)?;
        }
        for step in address.steps() {
            if let MachineAddressStep::OffsetValue(value)
            | MachineAddressStep::Index {
                index: MachineIndex::Value(value),
                ..
            } = step
            {
                self.mark_value(draft, *value)?;
            }
        }
        Ok(())
    }

    fn mark_pack(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        pack: &MachinePack,
    ) -> Result<(), MachineOptimizationError> {
        self.mark_value(draft, pack.length())?;
        for segment in pack.segments() {
            match segment {
                MachinePackSegment::Value { value, .. } => self.mark_value(draft, *value)?,
                MachinePackSegment::KeyedValue { key, value, .. } => {
                    self.mark_values(draft, [*key, *value])?;
                }
                MachinePackSegment::Spread(spread) => {
                    self.mark_value(draft, spread.remaining())?;
                }
            }
        }
        Ok(())
    }

    fn mark_terminator(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        terminator: &MachineTerminator,
    ) -> Result<(), MachineOptimizationError> {
        match terminator {
            MachineTerminator::Goto(target) => self.mark_target(draft, target)?,
            MachineTerminator::Branch {
                condition,
                then_target,
                else_target,
            } => {
                self.mark_value(draft, *condition)?;
                self.mark_target(draft, then_target)?;
                self.mark_target(draft, else_target)?;
            }
            MachineTerminator::BranchDropFlag {
                initialized,
                uninitialized,
                ..
            } => {
                self.mark_target(draft, initialized)?;
                self.mark_target(draft, uninitialized)?;
            }
            MachineTerminator::SwitchValue {
                subject,
                cases,
                fallback,
            } => {
                self.mark_value(draft, *subject)?;
                for case in cases {
                    self.mark_target(draft, case.target())?;
                }
                self.mark_target(draft, fallback)?;
            }
            MachineTerminator::SwitchTag {
                cases, fallback, ..
            } => {
                for case in cases {
                    self.mark_target(draft, case.target())?;
                }
                self.mark_target(draft, fallback)?;
            }
            MachineTerminator::Suspend {
                computation,
                resume,
            } => {
                self.mark_value(draft, *computation)?;
                self.mark_target(draft, resume)?;
            }
            MachineTerminator::Return(value) | MachineTerminator::Exit(value) => {
                if let Some(value) = value {
                    self.mark_value(draft, *value)?;
                }
            }
            MachineTerminator::Trap | MachineTerminator::Unreachable => {}
        }
        Ok(())
    }

    fn mark_target(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        target: &MachineBranchTarget,
    ) -> Result<(), MachineOptimizationError> {
        self.mark_values(draft, target.arguments().iter().copied())
    }

    fn mark_execution(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        execution: &MachineFunctionExecution,
    ) -> Result<(), MachineOptimizationError> {
        let MachineFunctionExecution::Deferred(frame) = execution else {
            return Ok(());
        };
        self.mark_fields(draft, frame.initial().fields())?;
        self.mark_cancellation(draft, frame.initial().cancellation())?;
        for state in frame.states() {
            self.mark_value(draft, state.awaited())?;
            self.mark_fields(draft, state.fields())?;
            self.mark_cancellation(draft, state.cancellation())?;
        }
        Ok(())
    }

    fn mark_fields(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        fields: &[MachineFrameField],
    ) -> Result<(), MachineOptimizationError> {
        for field in fields {
            if let MachineFrameField::Value(value) = field {
                self.mark_value(draft, *value)?;
            }
        }
        Ok(())
    }

    fn mark_cancellation(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        actions: &[MachineCancellationAction],
    ) -> Result<(), MachineOptimizationError> {
        for action in actions {
            if let MachineCancellationAction::ReleaseAwaited(value) = action {
                self.mark_value(draft, *value)?;
            }
        }
        Ok(())
    }
}

struct DenseRemap {
    operations: Vec<Option<MachineOperationId>>,
    values: Vec<Option<MachineValueId>>,
}

impl DenseRemap {
    fn new(draft: &crate::program::MachineBodyDraft, retention: &Retention) -> Self {
        Self {
            operations: dense_map::<MachineOperationId>(
                draft.operations.len(),
                &retention.operations,
            ),
            values: dense_map::<MachineValueId>(draft.values.len(), &retention.values),
        }
    }

    fn operation(
        &self,
        id: MachineOperationId,
    ) -> Result<MachineOperationId, MachineOptimizationError> {
        self.operations
            .get(id.index())
            .copied()
            .flatten()
            .ok_or(MachineOptimizationError::UnknownOperation(id))
    }

    fn retained_operation(
        &self,
        id: MachineOperationId,
    ) -> Result<Option<MachineOperationId>, MachineOptimizationError> {
        self.operations
            .get(id.index())
            .copied()
            .ok_or(MachineOptimizationError::UnknownOperation(id))
    }

    fn value(&self, id: MachineValueId) -> Result<MachineValueId, MachineOptimizationError> {
        self.values
            .get(id.index())
            .copied()
            .flatten()
            .ok_or(MachineOptimizationError::UnknownValue(id))
    }
}

fn dense_map<I: MachineId + Ord>(length: usize, retained: &BTreeSet<I>) -> Vec<Option<I>> {
    let mut next = 0;
    (0..length)
        .map(|index| {
            let old = I::new(index);
            retained.contains(&old).then(|| {
                let mapped = I::new(next);
                next += 1;
                mapped
            })
        })
        .collect()
}

fn remap_value(
    value: MachineValue,
    remap: &DenseRemap,
) -> Result<MachineValue, MachineOptimizationError> {
    let definition = match value.definition() {
        MachineValueDefinition::BlockParameter { block, position } => {
            MachineValueDefinition::BlockParameter { block, position }
        }
        MachineValueDefinition::Operation(operation) => {
            MachineValueDefinition::Operation(remap.operation(operation)?)
        }
    };
    Ok(MachineValue::new(
        value.ty(),
        value.representation(),
        definition,
    ))
}

fn remap_operation(
    operation: &MachineOperation,
    remap: &DenseRemap,
) -> Result<MachineOperation, MachineOptimizationError> {
    Ok(MachineOperation::new(
        remap_operation_kind(operation.kind(), remap)?,
        operation
            .result()
            .map(|value| remap.value(value))
            .transpose()?,
    ))
}

#[expect(
    clippy::too_many_lines,
    reason = "the closed machine operation domain is remapped exhaustively"
)]
fn remap_operation_kind(
    operation: &MachineOperationKind,
    remap: &DenseRemap,
) -> Result<MachineOperationKind, MachineOptimizationError> {
    Ok(match operation {
        MachineOperationKind::Constant(value) => MachineOperationKind::Constant(*value),
        MachineOperationKind::Load { source } => MachineOperationKind::Load { source: *source },
        MachineOperationKind::AddressOf { source } => {
            MachineOperationKind::AddressOf { source: *source }
        }
        MachineOperationKind::Store { destination, value } => MachineOperationKind::Store {
            destination: *destination,
            value: remap.value(*value)?,
        },
        MachineOperationKind::Unary { operation, operand } => MachineOperationKind::Unary {
            operation: *operation,
            operand: remap.value(*operand)?,
        },
        MachineOperationKind::Binary {
            operation,
            left,
            right,
        } => MachineOperationKind::Binary {
            operation: *operation,
            left: remap.value(*left)?,
            right: remap.value(*right)?,
        },
        MachineOperationKind::NumericConversion { operand } => {
            MachineOperationKind::NumericConversion {
                operand: remap.value(*operand)?,
            }
        }
        MachineOperationKind::Comparison(comparison) => {
            MachineOperationKind::Comparison(crate::MachineComparison::new(
                comparison.operation(),
                comparison.representation(),
                remap.value(comparison.left())?,
                remap.value(comparison.right())?,
            ))
        }
        MachineOperationKind::IndexBorrow(index) => {
            MachineOperationKind::IndexBorrow(MachineIndexBorrow::new(
                remap.value(index.receiver())?,
                remap.value(index.index())?,
                index.domain(),
            ))
        }
        MachineOperationKind::BorrowWeakening { source } => MachineOperationKind::BorrowWeakening {
            source: remap.value(*source)?,
        },
        MachineOperationKind::Aggregate(aggregate) => {
            MachineOperationKind::Aggregate(MachineAggregate::new(
                aggregate.size(),
                aggregate.alignment(),
                aggregate
                    .writes()
                    .iter()
                    .map(|write| remap_aggregate_write(*write, remap))
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        }
        MachineOperationKind::EraseCallable(erasure) => {
            MachineOperationKind::EraseCallable(MachineErasedCallable::new(
                remap.value(erasure.environment())?,
                erasure.environment_ty(),
                erasure.invoke(),
                erasure.destroy(),
            ))
        }
        MachineOperationKind::InvokeDrop {
            target,
            place,
            allocation,
        } => MachineOperationKind::InvokeDrop {
            target: *target,
            place: *place,
            allocation: *allocation,
        },
        MachineOperationKind::ReportError { place } => {
            MachineOperationKind::ReportError { place: *place }
        }
        MachineOperationKind::ReleaseError { place } => {
            MachineOperationKind::ReleaseError { place: *place }
        }
        MachineOperationKind::ReleaseComputation { place } => {
            MachineOperationKind::ReleaseComputation { place: *place }
        }
        MachineOperationKind::ReleaseErasedCallable { place } => {
            MachineOperationKind::ReleaseErasedCallable { place: *place }
        }
        MachineOperationKind::ReleaseMappedStorage { pointer, bytes } => {
            MachineOperationKind::ReleaseMappedStorage {
                pointer: remap.value(*pointer)?,
                bytes: remap.value(*bytes)?,
            }
        }
        MachineOperationKind::DriveComputation {
            computation,
            destination,
        } => MachineOperationKind::DriveComputation {
            computation: *computation,
            destination: *destination,
        },
        MachineOperationKind::CreateRegion { parent, region } => {
            MachineOperationKind::CreateRegion {
                parent: remap.value(*parent)?,
                region: *region,
            }
        }
        MachineOperationKind::ReleaseRegion { region } => {
            MachineOperationKind::ReleaseRegion { region: *region }
        }
        MachineOperationKind::SetDropFlag { flag, initialized } => {
            MachineOperationKind::SetDropFlag {
                flag: *flag,
                initialized: *initialized,
            }
        }
        MachineOperationKind::Call(call) => MachineOperationKind::Call(remap_call(call, remap)?),
        MachineOperationKind::PackLength => MachineOperationKind::PackLength,
        MachineOperationKind::PackNext => MachineOperationKind::PackNext,
        MachineOperationKind::DestroyPack => MachineOperationKind::DestroyPack,
    })
}

fn remap_aggregate_write(
    write: MachineAggregateWrite,
    remap: &DenseRemap,
) -> Result<MachineAggregateWrite, MachineOptimizationError> {
    Ok(match write {
        MachineAggregateWrite::Tag { offset, value } => {
            MachineAggregateWrite::Tag { offset, value }
        }
        MachineAggregateWrite::Value { offset, value } => MachineAggregateWrite::Value {
            offset,
            value: remap.value(value)?,
        },
        MachineAggregateWrite::RepeatedValue {
            offset,
            stride,
            count,
            value,
        } => MachineAggregateWrite::RepeatedValue {
            offset,
            stride,
            count,
            value: remap.value(value)?,
        },
    })
}

fn remap_call(
    call: &MachineCall,
    remap: &DenseRemap,
) -> Result<MachineCall, MachineOptimizationError> {
    let target = match call.target() {
        MachineCallTarget::Direct(target) => MachineCallTarget::Direct(*target),
        MachineCallTarget::Primitive(target) => MachineCallTarget::Primitive(target.clone()),
        MachineCallTarget::Imported(target) => MachineCallTarget::Imported(target.clone()),
        MachineCallTarget::Erased { callable, abi } => MachineCallTarget::Erased {
            callable: *callable,
            abi: *abi,
        },
    };
    Ok(MachineCall::new(
        target,
        call.arguments()
            .iter()
            .map(|value| remap.value(*value))
            .collect::<Result<Vec<_>, _>>()?,
        call.allocation(),
        call.pack(),
    ))
}

fn remap_addresses(
    addresses: &mut [MachineAddress],
    remap: &DenseRemap,
) -> Result<(), MachineOptimizationError> {
    for address in addresses {
        let root = match address.root() {
            MachineAddressRoot::Stack(stack) => MachineAddressRoot::Stack(stack),
            MachineAddressRoot::Data(data) => MachineAddressRoot::Data(data),
            MachineAddressRoot::Pointer { value } => MachineAddressRoot::Pointer {
                value: remap.value(value)?,
            },
            MachineAddressRoot::View {
                value,
                pointer_offset,
                length_offset,
            } => MachineAddressRoot::View {
                value: remap.value(value)?,
                pointer_offset,
                length_offset,
            },
        };
        let steps = address
            .steps()
            .iter()
            .map(|step| remap_address_step(*step, remap))
            .collect::<Result<Vec<_>, _>>()?;
        *address = match address.extent() {
            MachineAddressExtent::Stored { size, alignment } => {
                MachineAddress::new(address.ty(), size, alignment, root, steps)
            }
            MachineAddressExtent::View => MachineAddress::new_view(address.ty(), root, steps),
        };
    }
    Ok(())
}

fn remap_address_step(
    step: MachineAddressStep,
    remap: &DenseRemap,
) -> Result<MachineAddressStep, MachineOptimizationError> {
    Ok(match step {
        MachineAddressStep::OffsetValue(value) => {
            MachineAddressStep::OffsetValue(remap.value(value)?)
        }
        MachineAddressStep::Index {
            index: MachineIndex::Value(value),
            stride,
            bound,
        } => MachineAddressStep::Index {
            index: MachineIndex::Value(remap.value(value)?),
            stride,
            bound,
        },
        other => other,
    })
}

fn remap_packs(
    packs: &mut [MachinePack],
    remap: &DenseRemap,
) -> Result<(), MachineOptimizationError> {
    for pack in packs {
        let segments = pack
            .segments()
            .iter()
            .map(|segment| remap_pack_segment(segment, remap))
            .collect::<Result<Vec<_>, _>>()?;
        *pack = MachinePack::new(
            pack.element(),
            pack.next(),
            pack.next_result(),
            remap.value(pack.length())?,
            segments,
        );
    }
    Ok(())
}

fn remap_pack_segment(
    segment: &MachinePackSegment,
    remap: &DenseRemap,
) -> Result<MachinePackSegment, MachineOptimizationError> {
    Ok(match segment {
        MachinePackSegment::Value { value, destruction } => MachinePackSegment::Value {
            value: remap.value(*value)?,
            destruction: *destruction,
        },
        MachinePackSegment::KeyedValue {
            key,
            key_destruction,
            value,
            value_destruction,
        } => MachinePackSegment::KeyedValue {
            key: remap.value(*key)?,
            key_destruction: *key_destruction,
            value: remap.value(*value)?,
            value_destruction: *value_destruction,
        },
        MachinePackSegment::Spread(spread) => MachinePackSegment::Spread(MachinePackSpread::new(
            spread.iterator(),
            remap.value(spread.remaining())?,
            spread.next().clone(),
            spread.contribution(),
            spread.destruction(),
        )),
    })
}

fn remap_blocks(
    blocks: &mut [MachineBlock],
    remap: &DenseRemap,
) -> Result<(), MachineOptimizationError> {
    for block in blocks {
        let operations = block
            .operations()
            .iter()
            .map(|operation| remap.retained_operation(*operation))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        *block = MachineBlock::new(
            block
                .parameters()
                .iter()
                .map(|value| remap.value(*value))
                .collect::<Result<Vec<_>, _>>()?,
            operations,
            remap_terminator(block.terminator(), remap)?,
        );
    }
    Ok(())
}

fn remap_terminator(
    terminator: &MachineTerminator,
    remap: &DenseRemap,
) -> Result<MachineTerminator, MachineOptimizationError> {
    Ok(match terminator {
        MachineTerminator::Goto(target) => MachineTerminator::Goto(remap_target(target, remap)?),
        MachineTerminator::Branch {
            condition,
            then_target,
            else_target,
        } => MachineTerminator::Branch {
            condition: remap.value(*condition)?,
            then_target: remap_target(then_target, remap)?,
            else_target: remap_target(else_target, remap)?,
        },
        MachineTerminator::BranchDropFlag {
            flag,
            initialized,
            uninitialized,
        } => MachineTerminator::BranchDropFlag {
            flag: *flag,
            initialized: remap_target(initialized, remap)?,
            uninitialized: remap_target(uninitialized, remap)?,
        },
        MachineTerminator::SwitchValue {
            subject,
            cases,
            fallback,
        } => MachineTerminator::SwitchValue {
            subject: remap.value(*subject)?,
            cases: remap_cases(cases, remap)?,
            fallback: remap_target(fallback, remap)?,
        },
        MachineTerminator::SwitchTag {
            subject,
            tag_offset,
            cases,
            fallback,
        } => MachineTerminator::SwitchTag {
            subject: *subject,
            tag_offset: *tag_offset,
            cases: remap_cases(cases, remap)?,
            fallback: remap_target(fallback, remap)?,
        },
        MachineTerminator::Suspend {
            computation,
            resume,
        } => MachineTerminator::Suspend {
            computation: remap.value(*computation)?,
            resume: remap_target(resume, remap)?,
        },
        MachineTerminator::Return(value) => {
            MachineTerminator::Return(value.map(|value| remap.value(value)).transpose()?)
        }
        MachineTerminator::Exit(value) => {
            MachineTerminator::Exit(value.map(|value| remap.value(value)).transpose()?)
        }
        MachineTerminator::Trap => MachineTerminator::Trap,
        MachineTerminator::Unreachable => MachineTerminator::Unreachable,
    })
}

fn remap_cases(
    cases: &[MachineSwitchCase],
    remap: &DenseRemap,
) -> Result<Box<[MachineSwitchCase]>, MachineOptimizationError> {
    cases
        .iter()
        .map(|case| {
            Ok(MachineSwitchCase::new(
                case.value(),
                remap_target(case.target(), remap)?,
            ))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn remap_target(
    target: &MachineBranchTarget,
    remap: &DenseRemap,
) -> Result<MachineBranchTarget, MachineOptimizationError> {
    Ok(MachineBranchTarget::new(
        target.block(),
        target
            .arguments()
            .iter()
            .map(|value| remap.value(*value))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn remap_execution(
    execution: &mut MachineFunctionExecution,
    remap: &DenseRemap,
) -> Result<(), MachineOptimizationError> {
    let MachineFunctionExecution::Deferred(frame) = execution else {
        return Ok(());
    };
    let initial = MachineInitialAsyncState::new(
        remap_fields(frame.initial().fields(), remap)?,
        remap_cancellation(frame.initial().cancellation(), remap)?,
    );
    let states = frame
        .states()
        .iter()
        .map(|state| {
            Ok(MachineSuspensionState::new(
                state.suspend(),
                state.resume(),
                remap.value(state.awaited())?,
                remap_fields(state.fields(), remap)?,
                remap_cancellation(state.cancellation(), remap)?,
            ))
        })
        .collect::<Result<Vec<_>, MachineOptimizationError>>()?;
    *frame = MachineAsyncFrame::new(
        frame.output_representation(),
        initial,
        states,
        frame.completed_destruction(),
    );
    Ok(())
}

fn remap_fields(
    fields: &[MachineFrameField],
    remap: &DenseRemap,
) -> Result<Vec<MachineFrameField>, MachineOptimizationError> {
    fields
        .iter()
        .map(|field| {
            Ok(match *field {
                MachineFrameField::Value(value) => MachineFrameField::Value(remap.value(value)?),
                other => other,
            })
        })
        .collect()
}

fn remap_cancellation(
    actions: &[MachineCancellationAction],
    remap: &DenseRemap,
) -> Result<Vec<MachineCancellationAction>, MachineOptimizationError> {
    actions
        .iter()
        .map(|action| {
            Ok(match action {
                MachineCancellationAction::ReleaseAwaited(value) => {
                    MachineCancellationAction::ReleaseAwaited(remap.value(*value)?)
                }
                other => other.clone(),
            })
        })
        .collect()
}
