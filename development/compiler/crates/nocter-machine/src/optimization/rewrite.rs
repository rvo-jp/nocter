use std::collections::{BTreeMap, BTreeSet};

use crate::{MachineOperationId, MachineValueId};

/// Proven substitutions and removals consumed exactly once by dense body pruning.
#[derive(Default)]
pub(super) struct MachineRewriteProof {
    aliases: BTreeMap<MachineValueId, MachineValueId>,
    removed_operations: BTreeSet<MachineOperationId>,
}

impl MachineRewriteProof {
    pub(super) fn aliases(&self) -> &BTreeMap<MachineValueId, MachineValueId> {
        &self.aliases
    }

    pub(super) fn canonical_value(&self, mut value: MachineValueId) -> MachineValueId {
        while let Some(source) = self.aliases.get(&value) {
            value = *source;
        }
        value
    }

    pub(super) fn alias(&mut self, result: MachineValueId, source: MachineValueId) {
        self.aliases.insert(result, self.canonical_value(source));
    }

    pub(super) fn removes(&self, operation: MachineOperationId) -> bool {
        self.removed_operations.contains(&operation)
    }

    pub(super) fn remove(&mut self, operation: MachineOperationId) -> bool {
        self.removed_operations.insert(operation)
    }
}
