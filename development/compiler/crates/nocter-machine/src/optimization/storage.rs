use std::collections::{BTreeMap, BTreeSet};

use crate::identity::MachineId;
use crate::{
    MachineAddressExtent, MachineAddressId, MachineAddressRoot, MachineCallAllocation,
    MachineCallTarget, MachineCancellationAction, MachineFrameField, MachineFunctionExecution,
    MachineOperationId, MachineOperationKind, MachineStackId, MachineTerminator, MachineValueId,
};

use super::MachineOptimizationError;

/// Exact local-storage equivalences proved before body compaction.
///
/// A proof never crosses a basic-block or callable boundary. It is consumed by the common pruning
/// remapper, so forwarding does not create another mutable IR or a second identity-rewrite pass.
#[derive(Default)]
pub(super) struct LocalStorageProof {
    aliases: BTreeMap<MachineValueId, MachineValueId>,
    removed_operations: BTreeSet<MachineOperationId>,
    forwarded_loads: usize,
    removed_stores: usize,
}

impl LocalStorageProof {
    pub(super) fn aliases(&self) -> &BTreeMap<MachineValueId, MachineValueId> {
        &self.aliases
    }

    pub(super) fn canonical_value(&self, mut value: MachineValueId) -> MachineValueId {
        while let Some(source) = self.aliases.get(&value) {
            value = *source;
        }
        value
    }

    pub(super) fn removes(&self, operation: MachineOperationId) -> bool {
        self.removed_operations.contains(&operation)
    }

    pub(super) fn loads_forwarded(&self) -> usize {
        self.forwarded_loads
    }

    pub(super) fn stores_removed(&self) -> usize {
        self.removed_stores
    }
}

pub(super) fn prove(
    draft: &crate::program::MachineBodyDraft,
    execution: &MachineFunctionExecution,
) -> Result<LocalStorageProof, MachineOptimizationError> {
    let mut proof = LocalStorageProof::default();
    for block in &draft.blocks {
        let mut stored = BTreeMap::<MachineStackId, MachineValueId>::new();
        for operation_id in block.operations() {
            let operation = draft
                .operations
                .get(operation_id.index())
                .ok_or(MachineOptimizationError::UnknownOperation(*operation_id))?;
            match operation.kind() {
                MachineOperationKind::Store { destination, value } => {
                    let value = proof.canonical_value(*value);
                    if let Some(stack) = whole_stack(draft, *destination)? {
                        stored.insert(stack, value);
                    } else {
                        stored.clear();
                    }
                }
                MachineOperationKind::Load { source } => {
                    let Some(result) = operation.result() else {
                        continue;
                    };
                    let Some(stack) = whole_stack(draft, *source)? else {
                        continue;
                    };
                    let Some(value) = stored.get(&stack).copied() else {
                        continue;
                    };
                    let result_value = draft
                        .values
                        .get(result.index())
                        .ok_or(MachineOptimizationError::UnknownValue(result))?;
                    let source_value = draft
                        .values
                        .get(value.index())
                        .ok_or(MachineOptimizationError::UnknownValue(value))?;
                    if result_value.ty() == source_value.ty()
                        && result_value.representation() == source_value.representation()
                    {
                        proof.aliases.insert(result, value);
                        proof.removed_operations.insert(*operation_id);
                        proof.forwarded_loads += 1;
                    }
                }
                kind if kind.has_call_boundary() => stored.clear(),
                MachineOperationKind::DriveComputation { .. } => stored.clear(),
                _ => {}
            }
        }
    }
    remove_unobserved_stack_storage(draft, execution, &mut proof)?;
    Ok(proof)
}

fn whole_stack(
    draft: &crate::program::MachineBodyDraft,
    address: MachineAddressId,
) -> Result<Option<MachineStackId>, MachineOptimizationError> {
    let address_value = draft
        .addresses
        .get(address.index())
        .ok_or(MachineOptimizationError::UnknownAddress(address))?;
    let MachineAddressRoot::Stack(stack) = address_value.root() else {
        return Ok(None);
    };
    let stack_value = draft
        .stack
        .get(stack.index())
        .ok_or(MachineOptimizationError::UnknownStack(stack))?;
    Ok(matches!(
        address_value.extent(),
        MachineAddressExtent::Stored { size, alignment }
            if address_value.steps().is_empty()
                && address_value.ty() == stack_value.ty()
                && size == stack_value.size()
                && alignment == stack_value.alignment()
    )
    .then_some(stack))
}

fn remove_unobserved_stack_storage(
    draft: &crate::program::MachineBodyDraft,
    execution: &MachineFunctionExecution,
    proof: &mut LocalStorageProof,
) -> Result<(), MachineOptimizationError> {
    let mut candidates = (0..draft.stack.len())
        .map(MachineStackId::new)
        .collect::<BTreeSet<_>>();
    for parameter in &draft.parameters {
        candidates.remove(parameter);
    }

    let mut owners = BTreeMap::<MachineAddressId, MachineStackId>::new();
    for (index, address) in draft.addresses.iter().enumerate() {
        let MachineAddressRoot::Stack(stack) = address.root() else {
            continue;
        };
        let address_id = MachineAddressId::new(index);
        if whole_stack(draft, address_id)?.is_some() {
            owners.insert(address_id, stack);
        } else {
            candidates.remove(&stack);
        }
    }

    let mut stores = BTreeMap::<MachineStackId, Vec<MachineOperationId>>::new();
    for (index, operation) in draft.operations.iter().enumerate() {
        let operation_id = MachineOperationId::new(index);
        match operation.kind() {
            MachineOperationKind::Store { destination, .. } => {
                if let Some(stack) = owners.get(destination) {
                    stores.entry(*stack).or_default().push(operation_id);
                }
            }
            MachineOperationKind::Load { source } => {
                if !proof.removes(operation_id) {
                    disqualify_address(*source, &owners, &mut candidates);
                }
            }
            MachineOperationKind::AddressOf { source }
            | MachineOperationKind::ReportError { place: source }
            | MachineOperationKind::ReleaseError { place: source }
            | MachineOperationKind::ReleaseComputation { place: source }
            | MachineOperationKind::ReleaseErasedCallable { place: source } => {
                disqualify_address(*source, &owners, &mut candidates);
            }
            MachineOperationKind::InvokeDrop {
                place, allocation, ..
            } => {
                disqualify_address(*place, &owners, &mut candidates);
                disqualify_allocation(*allocation, &owners, &mut candidates);
            }
            MachineOperationKind::DriveComputation {
                computation,
                destination,
            } => {
                disqualify_address(*computation, &owners, &mut candidates);
                if let Some(destination) = destination {
                    disqualify_address(*destination, &owners, &mut candidates);
                }
            }
            MachineOperationKind::CreateRegion { region, .. }
            | MachineOperationKind::ReleaseRegion { region } => {
                candidates.remove(region);
            }
            MachineOperationKind::Call(call) => {
                if let MachineCallTarget::Erased { callable, .. } = call.target() {
                    disqualify_address(*callable, &owners, &mut candidates);
                }
                disqualify_allocation(call.allocation(), &owners, &mut candidates);
            }
            _ => {}
        }
    }

    for block in &draft.blocks {
        if let MachineTerminator::SwitchTag { subject, .. } = block.terminator() {
            disqualify_address(*subject, &owners, &mut candidates);
        }
    }
    for pack in &draft.packs {
        for segment in pack.segments() {
            if let crate::MachinePackSegment::Spread(spread) = segment {
                disqualify_address(spread.iterator(), &owners, &mut candidates);
            }
        }
    }
    disqualify_execution(execution, &owners, &mut candidates);

    for stack in candidates {
        let Some(stack_stores) = stores.get(&stack) else {
            continue;
        };
        for operation in stack_stores {
            if proof.removed_operations.insert(*operation) {
                proof.removed_stores += 1;
            }
        }
    }
    Ok(())
}

fn disqualify_address(
    address: MachineAddressId,
    owners: &BTreeMap<MachineAddressId, MachineStackId>,
    candidates: &mut BTreeSet<MachineStackId>,
) {
    if let Some(stack) = owners.get(&address) {
        candidates.remove(stack);
    }
}

fn disqualify_allocation(
    allocation: MachineCallAllocation,
    owners: &BTreeMap<MachineAddressId, MachineStackId>,
    candidates: &mut BTreeSet<MachineStackId>,
) {
    match allocation {
        MachineCallAllocation::Inherit => {}
        MachineCallAllocation::Lexical(stack) => {
            candidates.remove(&stack);
        }
        MachineCallAllocation::Explicit(address) => {
            disqualify_address(address, owners, candidates);
        }
    }
}

fn disqualify_execution(
    execution: &MachineFunctionExecution,
    owners: &BTreeMap<MachineAddressId, MachineStackId>,
    candidates: &mut BTreeSet<MachineStackId>,
) {
    let MachineFunctionExecution::Deferred(frame) = execution else {
        return;
    };
    disqualify_fields(frame.initial().fields(), candidates);
    disqualify_cancellation(frame.initial().cancellation(), owners, candidates);
    for state in frame.states() {
        disqualify_fields(state.fields(), candidates);
        disqualify_cancellation(state.cancellation(), owners, candidates);
    }
}

fn disqualify_fields(fields: &[MachineFrameField], candidates: &mut BTreeSet<MachineStackId>) {
    for field in fields {
        if let MachineFrameField::Stack(stack) = field {
            candidates.remove(stack);
        }
    }
}

fn disqualify_cancellation(
    actions: &[MachineCancellationAction],
    owners: &BTreeMap<MachineAddressId, MachineStackId>,
    candidates: &mut BTreeSet<MachineStackId>,
) {
    for action in actions {
        match action {
            MachineCancellationAction::Destroy { address, .. } => {
                disqualify_address(*address, owners, candidates);
            }
            MachineCancellationAction::ReleaseRegion(stack) => {
                candidates.remove(stack);
            }
            MachineCancellationAction::ReleaseAwaited(_)
            | MachineCancellationAction::DestroyPack => {}
        }
    }
}
