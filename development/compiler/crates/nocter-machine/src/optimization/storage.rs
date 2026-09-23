use std::collections::{BTreeMap, BTreeSet};

use crate::{
    MachineAddressId, MachineAddressRoot, MachineOperationId, MachineOperationKind, MachineStackId,
    MachineValueId,
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
        self.removed_operations.len()
    }
}

pub(super) fn prove(
    draft: &crate::program::MachineBodyDraft,
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
                    }
                }
                kind if kind.has_call_boundary() => stored.clear(),
                MachineOperationKind::DriveComputation { .. } => stored.clear(),
                _ => {}
            }
        }
    }
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
    Ok(match address_value.root() {
        MachineAddressRoot::Stack(stack) if address_value.steps().is_empty() => Some(stack),
        _ => None,
    })
}
