use std::collections::BTreeMap;

use crate::{MachineDropFlagId, MachineOperationId, MachineOperationKind};

use super::MachineOptimizationError;

/// Removes writes shadowed within one block before any callable boundary can observe cleanup state.
pub(super) fn prove(
    draft: &crate::program::MachineBodyDraft,
    rewrites: &mut super::rewrite::MachineRewriteProof,
) -> Result<usize, MachineOptimizationError> {
    let mut removed = 0;
    for block in &draft.blocks {
        let mut latest = BTreeMap::<MachineDropFlagId, (bool, MachineOperationId)>::new();
        for operation_id in block.operations() {
            if rewrites.removes(*operation_id) {
                continue;
            }
            let operation = draft
                .operations
                .get(operation_id.index())
                .ok_or(MachineOptimizationError::UnknownOperation(*operation_id))?;
            match operation.kind() {
                MachineOperationKind::SetDropFlag { flag, initialized } => {
                    let (retained, redundant) =
                        coalesce(latest.get(flag).copied(), (*initialized, *operation_id));
                    latest.insert(*flag, retained);
                    if redundant.is_some_and(|operation| rewrites.remove(operation)) {
                        removed += 1;
                    }
                }
                kind if kind.has_call_boundary() => latest.clear(),
                _ => {}
            }
        }
    }
    Ok(removed)
}

fn coalesce(
    previous: Option<(bool, MachineOperationId)>,
    current: (bool, MachineOperationId),
) -> ((bool, MachineOperationId), Option<MachineOperationId>) {
    match previous {
        Some(previous) if previous.0 == current.0 => (previous, Some(current.1)),
        Some(previous) => (current, Some(previous.1)),
        None => (current, None),
    }
}

#[cfg(test)]
mod tests {
    use crate::MachineOperationId;
    use crate::identity::MachineId;

    use super::coalesce;

    #[test]
    fn same_state_keeps_the_first_write_and_changed_state_keeps_the_last() {
        let first = MachineOperationId::new(0);
        let second = MachineOperationId::new(1);
        assert_eq!(
            coalesce(Some((true, first)), (true, second)),
            ((true, first), Some(second))
        );
        assert_eq!(
            coalesce(Some((true, first)), (false, second)),
            ((false, second), Some(first))
        );
    }
}
