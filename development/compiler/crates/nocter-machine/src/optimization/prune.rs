use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::effect::MachineOperationEffect;
use crate::identity::MachineId;
use crate::{
    MachineAddress, MachineAddressExtent, MachineAddressId, MachineAddressRoot, MachineAddressStep,
    MachineAggregate, MachineAggregateWrite, MachineAsyncFrame, MachineBlock, MachineBlockId,
    MachineBranchTarget, MachineCall, MachineCallAllocation, MachineCallPack, MachineCallTarget,
    MachineCancellationAction, MachineDropFlagId, MachineErasedCallable, MachineFrameField,
    MachineFunctionExecution, MachineIndex, MachineIndexBorrow, MachineInitialAsyncState,
    MachineOperation, MachineOperationId, MachineOperationKind, MachinePack, MachinePackId,
    MachinePackSegment, MachinePackSpread, MachineStackId, MachineSuspensionState,
    MachineSwitchCase, MachineTerminator, MachineValue, MachineValueDefinition, MachineValueId,
    MachineValueStorage,
};

use super::{MachineOptimizationError, MachineOptimizationReport};

/// Retains the complete execution-resource closure of reachable blocks and compacts every
/// affected body-local identity domain together.
pub(super) fn unreachable_and_unused(
    draft: &mut crate::program::MachineBodyDraft,
    execution: &mut MachineFunctionExecution,
    rewrites: &super::rewrite::MachineRewriteProof,
    report: &mut MachineOptimizationReport,
) -> Result<(), MachineOptimizationError> {
    let retention = Retention::build(draft, execution, rewrites)?;
    if retention.blocks.len() == draft.blocks.len()
        && retention.operations.len() == draft.operations.len()
        && retention.values.len() == draft.values.len()
        && retention.stack.len() == draft.stack.len()
        && retention.addresses.len() == draft.addresses.len()
        && retention.drop_flags.len() == draft.drop_flags.len()
        && retention.packs.len() == draft.packs.len()
    {
        return Ok(());
    }
    let remap = DenseRemap::new(draft, &retention, rewrites);
    let old_block_count = draft.blocks.len();
    let old_operation_count = draft.operations.len();
    let old_value_count = draft.values.len();
    let old_stack_count = draft.stack.len();
    let old_address_count = draft.addresses.len();
    let old_drop_flag_count = draft.drop_flags.len();
    let old_pack_count = draft.packs.len();

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
    draft.parameters = draft
        .parameters
        .iter()
        .map(|stack| remap.stack(*stack))
        .collect::<Result<Vec<_>, _>>()?;
    draft.stack = retain_domain(std::mem::take(&mut draft.stack), &retention.stack);
    draft.drop_flags = retain_domain(std::mem::take(&mut draft.drop_flags), &retention.drop_flags);
    draft.addresses = std::mem::take(&mut draft.addresses)
        .into_iter()
        .enumerate()
        .filter(|(index, _)| retention.addresses.contains(&MachineAddressId::new(*index)))
        .map(|(_, address)| remap_address(&address, &remap))
        .collect::<Result<Vec<_>, _>>()?;
    draft.packs = std::mem::take(&mut draft.packs)
        .into_iter()
        .enumerate()
        .filter(|(index, _)| retention.packs.contains(&MachinePackId::new(*index)))
        .map(|(_, pack)| remap_pack(&pack, &remap))
        .collect::<Result<Vec<_>, _>>()?;
    draft.blocks = std::mem::take(&mut draft.blocks)
        .into_iter()
        .enumerate()
        .filter(|(index, _)| retention.blocks.contains(&MachineBlockId::new(*index)))
        .map(|(_, block)| remap_block(&block, &remap))
        .collect::<Result<Vec<_>, _>>()?;
    draft.entry = remap.block(draft.entry)?;
    remap_execution(execution, &retention, &remap)?;

    report.blocks_removed += old_block_count - draft.blocks.len();
    report.operations_removed += old_operation_count - draft.operations.len();
    report.values_removed += old_value_count - draft.values.len();
    report.stack_objects_removed += old_stack_count - draft.stack.len();
    report.addresses_removed += old_address_count - draft.addresses.len();
    report.drop_flags_removed += old_drop_flag_count - draft.drop_flags.len();
    report.packs_removed += old_pack_count - draft.packs.len();
    Ok(())
}

#[derive(Default)]
struct Retention {
    blocks: BTreeSet<MachineBlockId>,
    operations: BTreeSet<MachineOperationId>,
    values: BTreeSet<MachineValueId>,
    stack: BTreeSet<MachineStackId>,
    addresses: BTreeSet<MachineAddressId>,
    drop_flags: BTreeSet<MachineDropFlagId>,
    packs: BTreeSet<MachinePackId>,
    value_aliases: BTreeMap<MachineValueId, MachineValueId>,
}

impl Retention {
    fn build(
        draft: &crate::program::MachineBodyDraft,
        execution: &MachineFunctionExecution,
        rewrites: &super::rewrite::MachineRewriteProof,
    ) -> Result<Self, MachineOptimizationError> {
        let mut retained = Self::reachable_blocks(draft)?;
        retained.value_aliases = rewrites.aliases().clone();
        for block_id in retained.blocks.clone() {
            let block = draft
                .blocks
                .get(block_id.index())
                .ok_or(MachineOptimizationError::UnknownBlock(block_id))?;
            for parameter in block.parameters() {
                retained.mark_value(draft, *parameter)?;
            }
            retained.mark_terminator(draft, block.terminator())?;
            for operation_id in block.operations() {
                let operation = draft
                    .operations
                    .get(operation_id.index())
                    .ok_or(MachineOptimizationError::UnknownOperation(*operation_id))?;
                if !rewrites.removes(*operation_id)
                    && !rewrites.is_proven_pure(*operation_id)
                    && operation.kind().effect() != MachineOperationEffect::Pure
                {
                    retained.operations.insert(*operation_id);
                }
            }
        }
        for parameter in &draft.parameters {
            retained.mark_stack(draft, *parameter)?;
        }
        retained.mark_execution(draft, execution)?;

        let mut visited_operations = BTreeSet::new();
        let mut visited_values = BTreeSet::new();
        let mut visited_addresses = BTreeSet::new();
        let mut visited_packs = BTreeSet::new();
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
            let pending_addresses = retained
                .addresses
                .difference(&visited_addresses)
                .copied()
                .collect::<Vec<_>>();
            let pending_packs = retained
                .packs
                .difference(&visited_packs)
                .copied()
                .collect::<Vec<_>>();
            if pending_operations.is_empty()
                && pending_values.is_empty()
                && pending_addresses.is_empty()
                && pending_packs.is_empty()
            {
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
            for address_id in pending_addresses {
                visited_addresses.insert(address_id);
                let address = draft
                    .addresses
                    .get(address_id.index())
                    .ok_or(MachineOptimizationError::UnknownAddress(address_id))?;
                retained.mark_address_inputs(draft, address)?;
            }
            for pack_id in pending_packs {
                visited_packs.insert(pack_id);
                let pack = draft
                    .packs
                    .get(pack_id.index())
                    .ok_or(MachineOptimizationError::UnknownPack(pack_id))?;
                retained.mark_pack_inputs(draft, pack)?;
            }
        }
        Ok(retained)
    }

    fn reachable_blocks(
        draft: &crate::program::MachineBodyDraft,
    ) -> Result<Self, MachineOptimizationError> {
        let mut retained = Self::default();
        let mut pending = VecDeque::from([draft.entry]);
        while let Some(block_id) = pending.pop_front() {
            let block = draft
                .blocks
                .get(block_id.index())
                .ok_or(MachineOptimizationError::UnknownBlock(block_id))?;
            if !retained.blocks.insert(block_id) {
                continue;
            }
            pending.extend(successors(block.terminator()));
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
        mut value: MachineValueId,
    ) -> Result<(), MachineOptimizationError> {
        while let Some(source) = self.value_aliases.get(&value) {
            value = *source;
        }
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

    fn mark_stack(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        stack: MachineStackId,
    ) -> Result<(), MachineOptimizationError> {
        if draft.stack.get(stack.index()).is_none() {
            return Err(MachineOptimizationError::UnknownStack(stack));
        }
        self.stack.insert(stack);
        Ok(())
    }

    fn mark_address_id(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        address: MachineAddressId,
    ) -> Result<(), MachineOptimizationError> {
        if draft.addresses.get(address.index()).is_none() {
            return Err(MachineOptimizationError::UnknownAddress(address));
        }
        self.addresses.insert(address);
        Ok(())
    }

    fn mark_drop_flag(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        flag: MachineDropFlagId,
    ) -> Result<(), MachineOptimizationError> {
        if draft.drop_flags.get(flag.index()).is_none() {
            return Err(MachineOptimizationError::UnknownDropFlag(flag));
        }
        self.drop_flags.insert(flag);
        Ok(())
    }

    fn mark_pack_id(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        pack: MachinePackId,
    ) -> Result<(), MachineOptimizationError> {
        if draft.packs.get(pack.index()).is_none() {
            return Err(MachineOptimizationError::UnknownPack(pack));
        }
        self.packs.insert(pack);
        Ok(())
    }

    fn mark_operation_inputs(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        operation: &MachineOperationKind,
    ) -> Result<(), MachineOptimizationError> {
        match operation {
            MachineOperationKind::Load { source } | MachineOperationKind::AddressOf { source } => {
                self.mark_address_id(draft, *source)?;
            }
            MachineOperationKind::Store { destination, value } => {
                self.mark_address_id(draft, *destination)?;
                self.mark_value(draft, *value)?;
            }
            MachineOperationKind::Unary { operand: value, .. }
            | MachineOperationKind::NumericConversion { operand: value }
            | MachineOperationKind::BorrowWeakening { source: value } => {
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
                self.mark_value(draft, index.receiver())?;
                if let MachineIndex::Value(value) = index.index() {
                    self.mark_value(draft, value)?;
                }
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
                self.mark_call_resources(draft, call)?;
            }
            MachineOperationKind::InvokeDrop { place, .. }
            | MachineOperationKind::ReportError { place }
            | MachineOperationKind::ReleaseError { place }
            | MachineOperationKind::ReleaseComputation { place }
            | MachineOperationKind::ReleaseErasedCallable { place } => {
                self.mark_address_id(draft, *place)?;
            }
            MachineOperationKind::DriveComputation {
                computation,
                destination,
            } => {
                self.mark_address_id(draft, *computation)?;
                if let Some(destination) = destination {
                    self.mark_address_id(draft, *destination)?;
                }
            }
            MachineOperationKind::CreateRegion { parent, region } => {
                self.mark_value(draft, *parent)?;
                self.mark_stack(draft, *region)?;
            }
            MachineOperationKind::ReleaseRegion { region } => {
                self.mark_stack(draft, *region)?;
            }
            MachineOperationKind::SetDropFlag { flag, .. } => {
                self.mark_drop_flag(draft, *flag)?;
            }
            MachineOperationKind::Constant(_)
            | MachineOperationKind::PackLength
            | MachineOperationKind::PackNext
            | MachineOperationKind::DestroyPack => {}
        }
        Ok(())
    }

    fn mark_call_resources(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        call: &MachineCall,
    ) -> Result<(), MachineOptimizationError> {
        if let MachineCallTarget::Erased { callable, .. } = call.target() {
            self.mark_address_id(draft, *callable)?;
        }
        match call.allocation() {
            MachineCallAllocation::Inherit => {}
            MachineCallAllocation::Lexical(stack) => self.mark_stack(draft, stack)?,
            MachineCallAllocation::Explicit(address) => self.mark_address_id(draft, address)?,
        }
        if let Some(MachineCallPack::Prepared(pack)) = call.pack() {
            self.mark_pack_id(draft, pack)?;
        }
        Ok(())
    }

    fn mark_address_inputs(
        &mut self,
        draft: &crate::program::MachineBodyDraft,
        address: &MachineAddress,
    ) -> Result<(), MachineOptimizationError> {
        if let MachineAddressRoot::Pointer { value } | MachineAddressRoot::View { value, .. } =
            address.root()
        {
            self.mark_value(draft, value)?;
        } else if let MachineAddressRoot::Stack(stack) = address.root() {
            self.mark_stack(draft, stack)?;
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

    fn mark_pack_inputs(
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
                    self.mark_address_id(draft, spread.iterator())?;
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
                flag,
                initialized,
                uninitialized,
            } => {
                self.mark_drop_flag(draft, *flag)?;
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
                subject,
                cases,
                fallback,
                ..
            } => {
                self.mark_address_id(draft, *subject)?;
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
            if !self.blocks.contains(&state.suspend()) {
                continue;
            }
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
            match field {
                MachineFrameField::Pack => {}
                MachineFrameField::Stack(stack) => self.mark_stack(draft, *stack)?,
                MachineFrameField::Value(value) => self.mark_value(draft, *value)?,
                MachineFrameField::DropFlag(flag) => self.mark_drop_flag(draft, *flag)?,
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
            match action {
                MachineCancellationAction::ReleaseAwaited(value) => {
                    self.mark_value(draft, *value)?;
                }
                MachineCancellationAction::Destroy {
                    address,
                    initialized,
                    ..
                } => {
                    self.mark_address_id(draft, *address)?;
                    if let Some(flag) = initialized {
                        self.mark_drop_flag(draft, *flag)?;
                    }
                }
                MachineCancellationAction::ReleaseRegion(stack) => {
                    self.mark_stack(draft, *stack)?;
                }
                MachineCancellationAction::DestroyPack => {}
            }
        }
        Ok(())
    }
}

fn successors(terminator: &MachineTerminator) -> Vec<MachineBlockId> {
    match terminator {
        MachineTerminator::Goto(target) => vec![target.block()],
        MachineTerminator::Branch {
            then_target,
            else_target,
            ..
        }
        | MachineTerminator::BranchDropFlag {
            initialized: then_target,
            uninitialized: else_target,
            ..
        } => vec![then_target.block(), else_target.block()],
        MachineTerminator::SwitchValue {
            cases, fallback, ..
        }
        | MachineTerminator::SwitchTag {
            cases, fallback, ..
        } => cases
            .iter()
            .map(|case| case.target().block())
            .chain(std::iter::once(fallback.block()))
            .collect(),
        MachineTerminator::Suspend { resume, .. } => vec![resume.block()],
        MachineTerminator::Return(_)
        | MachineTerminator::Exit(_)
        | MachineTerminator::Trap
        | MachineTerminator::Unreachable => Vec::new(),
    }
}

struct DenseRemap {
    blocks: Vec<Option<MachineBlockId>>,
    operations: Vec<Option<MachineOperationId>>,
    values: Vec<Option<MachineValueId>>,
    stack: Vec<Option<MachineStackId>>,
    addresses: Vec<Option<MachineAddressId>>,
    drop_flags: Vec<Option<MachineDropFlagId>>,
    packs: Vec<Option<MachinePackId>>,
    value_aliases: BTreeMap<MachineValueId, MachineValueId>,
}

impl DenseRemap {
    fn new(
        draft: &crate::program::MachineBodyDraft,
        retention: &Retention,
        rewrites: &super::rewrite::MachineRewriteProof,
    ) -> Self {
        Self {
            blocks: dense_map::<MachineBlockId>(draft.blocks.len(), &retention.blocks),
            operations: dense_map::<MachineOperationId>(
                draft.operations.len(),
                &retention.operations,
            ),
            values: dense_map::<MachineValueId>(draft.values.len(), &retention.values),
            stack: dense_map::<MachineStackId>(draft.stack.len(), &retention.stack),
            addresses: dense_map::<MachineAddressId>(draft.addresses.len(), &retention.addresses),
            drop_flags: dense_map::<MachineDropFlagId>(
                draft.drop_flags.len(),
                &retention.drop_flags,
            ),
            packs: dense_map::<MachinePackId>(draft.packs.len(), &retention.packs),
            value_aliases: rewrites.aliases().clone(),
        }
    }

    fn block(&self, id: MachineBlockId) -> Result<MachineBlockId, MachineOptimizationError> {
        self.blocks
            .get(id.index())
            .copied()
            .flatten()
            .ok_or(MachineOptimizationError::UnknownBlock(id))
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

    fn value(&self, mut id: MachineValueId) -> Result<MachineValueId, MachineOptimizationError> {
        while let Some(source) = self.value_aliases.get(&id) {
            id = *source;
        }
        self.values
            .get(id.index())
            .copied()
            .flatten()
            .ok_or(MachineOptimizationError::UnknownValue(id))
    }

    fn stack(&self, id: MachineStackId) -> Result<MachineStackId, MachineOptimizationError> {
        self.stack
            .get(id.index())
            .copied()
            .flatten()
            .ok_or(MachineOptimizationError::UnknownStack(id))
    }

    fn address(&self, id: MachineAddressId) -> Result<MachineAddressId, MachineOptimizationError> {
        self.addresses
            .get(id.index())
            .copied()
            .flatten()
            .ok_or(MachineOptimizationError::UnknownAddress(id))
    }

    fn drop_flag(
        &self,
        id: MachineDropFlagId,
    ) -> Result<MachineDropFlagId, MachineOptimizationError> {
        self.drop_flags
            .get(id.index())
            .copied()
            .flatten()
            .ok_or(MachineOptimizationError::UnknownDropFlag(id))
    }

    fn pack(&self, id: MachinePackId) -> Result<MachinePackId, MachineOptimizationError> {
        self.packs
            .get(id.index())
            .copied()
            .flatten()
            .ok_or(MachineOptimizationError::UnknownPack(id))
    }
}

fn retain_domain<I: MachineId + Ord, T>(values: Vec<T>, retained: &BTreeSet<I>) -> Vec<T> {
    values
        .into_iter()
        .enumerate()
        .filter(|(index, _)| retained.contains(&I::new(*index)))
        .map(|(_, value)| value)
        .collect()
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
            MachineValueDefinition::BlockParameter {
                block: remap.block(block)?,
                position,
            }
        }
        MachineValueDefinition::Operation(operation) => {
            MachineValueDefinition::Operation(remap.operation(operation)?)
        }
    };
    let storage = match value.storage() {
        MachineValueStorage::Independent => MachineValueStorage::Independent,
        MachineValueStorage::Alias(source) => MachineValueStorage::Alias(remap.value(source)?),
    };
    Ok(MachineValue::new(value.ty(), value.representation(), definition).with_storage(storage))
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
        MachineOperationKind::Load { source } => MachineOperationKind::Load {
            source: remap.address(*source)?,
        },
        MachineOperationKind::AddressOf { source } => MachineOperationKind::AddressOf {
            source: remap.address(*source)?,
        },
        MachineOperationKind::Store { destination, value } => MachineOperationKind::Store {
            destination: remap.address(*destination)?,
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
                match index.index() {
                    MachineIndex::Constant(index) => MachineIndex::Constant(index),
                    MachineIndex::Value(value) => MachineIndex::Value(remap.value(value)?),
                },
                index.domain(),
                index.check(),
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
            place: remap.address(*place)?,
            allocation: remap_allocation(*allocation, remap)?,
        },
        MachineOperationKind::ReportError { place } => MachineOperationKind::ReportError {
            place: remap.address(*place)?,
        },
        MachineOperationKind::ReleaseError { place } => MachineOperationKind::ReleaseError {
            place: remap.address(*place)?,
        },
        MachineOperationKind::ReleaseComputation { place } => {
            MachineOperationKind::ReleaseComputation {
                place: remap.address(*place)?,
            }
        }
        MachineOperationKind::ReleaseErasedCallable { place } => {
            MachineOperationKind::ReleaseErasedCallable {
                place: remap.address(*place)?,
            }
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
            computation: remap.address(*computation)?,
            destination: destination
                .map(|address| remap.address(address))
                .transpose()?,
        },
        MachineOperationKind::CreateRegion { parent, region } => {
            MachineOperationKind::CreateRegion {
                parent: remap.value(*parent)?,
                region: remap.stack(*region)?,
            }
        }
        MachineOperationKind::ReleaseRegion { region } => MachineOperationKind::ReleaseRegion {
            region: remap.stack(*region)?,
        },
        MachineOperationKind::SetDropFlag { flag, initialized } => {
            MachineOperationKind::SetDropFlag {
                flag: remap.drop_flag(*flag)?,
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
            callable: remap.address(*callable)?,
            abi: *abi,
        },
    };
    Ok(MachineCall::new(
        target,
        call.arguments()
            .iter()
            .map(|value| remap.value(*value))
            .collect::<Result<Vec<_>, _>>()?,
        remap_allocation(call.allocation(), remap)?,
        match call.pack() {
            Some(MachineCallPack::Prepared(pack)) => {
                Some(MachineCallPack::Prepared(remap.pack(pack)?))
            }
            other => other,
        },
    ))
}

fn remap_allocation(
    allocation: MachineCallAllocation,
    remap: &DenseRemap,
) -> Result<MachineCallAllocation, MachineOptimizationError> {
    Ok(match allocation {
        MachineCallAllocation::Inherit => MachineCallAllocation::Inherit,
        MachineCallAllocation::Lexical(stack) => {
            MachineCallAllocation::Lexical(remap.stack(stack)?)
        }
        MachineCallAllocation::Explicit(address) => {
            MachineCallAllocation::Explicit(remap.address(address)?)
        }
    })
}

fn remap_address(
    address: &MachineAddress,
    remap: &DenseRemap,
) -> Result<MachineAddress, MachineOptimizationError> {
    let root = match address.root() {
        MachineAddressRoot::Stack(stack) => MachineAddressRoot::Stack(remap.stack(stack)?),
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
    Ok(match address.extent() {
        MachineAddressExtent::Stored { size, alignment } => {
            MachineAddress::new(address.ty(), size, alignment, root, steps)
        }
        MachineAddressExtent::View => MachineAddress::new_view(address.ty(), root, steps),
    })
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
            check,
        } => MachineAddressStep::Index {
            index: MachineIndex::Value(remap.value(value)?),
            stride,
            bound,
            check,
        },
        other => other,
    })
}

fn remap_pack(
    pack: &MachinePack,
    remap: &DenseRemap,
) -> Result<MachinePack, MachineOptimizationError> {
    let segments = pack
        .segments()
        .iter()
        .map(|segment| remap_pack_segment(segment, remap))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MachinePack::new(
        pack.element(),
        pack.next(),
        pack.next_result(),
        remap.value(pack.length())?,
        segments,
    ))
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
            remap.address(spread.iterator())?,
            remap.value(spread.remaining())?,
            spread.next().clone(),
            spread.contribution(),
            spread.destruction(),
        )),
    })
}

fn remap_block(
    block: &MachineBlock,
    remap: &DenseRemap,
) -> Result<MachineBlock, MachineOptimizationError> {
    let operations = block
        .operations()
        .iter()
        .map(|operation| remap.retained_operation(*operation))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    Ok(MachineBlock::new(
        block
            .parameters()
            .iter()
            .map(|value| remap.value(*value))
            .collect::<Result<Vec<_>, _>>()?,
        operations,
        remap_terminator(block.terminator(), remap)?,
    ))
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
            flag: remap.drop_flag(*flag)?,
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
            subject: remap.address(*subject)?,
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
        remap.block(target.block())?,
        target
            .arguments()
            .iter()
            .map(|value| remap.value(*value))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn remap_execution(
    execution: &mut MachineFunctionExecution,
    retention: &Retention,
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
        .filter(|state| retention.blocks.contains(&state.suspend()))
        .map(|state| {
            Ok(MachineSuspensionState::new(
                remap.block(state.suspend())?,
                remap.block(state.resume())?,
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
                MachineFrameField::Stack(stack) => MachineFrameField::Stack(remap.stack(stack)?),
                MachineFrameField::DropFlag(flag) => {
                    MachineFrameField::DropFlag(remap.drop_flag(flag)?)
                }
                MachineFrameField::Pack => MachineFrameField::Pack,
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
                MachineCancellationAction::Destroy {
                    address,
                    initialized,
                    destruction,
                } => MachineCancellationAction::Destroy {
                    address: remap.address(*address)?,
                    initialized: initialized.map(|flag| remap.drop_flag(flag)).transpose()?,
                    destruction: *destruction,
                },
                MachineCancellationAction::ReleaseRegion(stack) => {
                    MachineCancellationAction::ReleaseRegion(remap.stack(*stack)?)
                }
                MachineCancellationAction::DestroyPack => MachineCancellationAction::DestroyPack,
            })
        })
        .collect()
}
