use crate::identity::MachineId;
use crate::{
    MachineOperationId, MachineOperationKind, MachineValueDefinition, MachineValueRepresentation,
    MachineValueStorage,
};

use super::MachineOptimizationError;

/// Proves representation-preserving storage aliases without erasing semantic value identities.
///
/// Borrow weakening changes the capability carried by the value's type, but not its runtime bits.
/// Recording that fact on the result lets every target share storage without rediscovering the
/// semantic rule or replacing the readonly value with its mutable source.
pub(super) fn prove(
    draft: &mut crate::program::MachineBodyDraft,
) -> Result<usize, MachineOptimizationError> {
    let mut aliases = Vec::new();
    for (operation_index, operation) in draft.operations.iter().enumerate() {
        let operation_id = MachineOperationId::new(operation_index);
        let (MachineOperationKind::BorrowWeakening { source }, Some(result)) =
            (operation.kind(), operation.result())
        else {
            continue;
        };
        let result_value = draft
            .values
            .get(result.index())
            .ok_or(MachineOptimizationError::UnknownValue(result))?;
        let source_value = draft
            .values
            .get(source.index())
            .ok_or(MachineOptimizationError::UnknownValue(*source))?;
        if result_value.definition() != MachineValueDefinition::Operation(operation_id)
            || !same_stored_representation(
                result_value.representation(),
                source_value.representation(),
            )
        {
            return Err(MachineOptimizationError::InvalidStorageAlias {
                result,
                source: *source,
            });
        }
        aliases.push((result, *source));
    }

    for (result, source) in aliases.iter().copied() {
        let value = draft
            .values
            .get_mut(result.index())
            .ok_or(MachineOptimizationError::UnknownValue(result))?;
        *value = value.with_storage(MachineValueStorage::Alias(source));
    }
    Ok(aliases.len())
}

fn same_stored_representation(
    left: MachineValueRepresentation,
    right: MachineValueRepresentation,
) -> bool {
    match (left, right) {
        (
            MachineValueRepresentation::Stored {
                size: left_size,
                alignment: left_alignment,
                class: left_class,
            },
            MachineValueRepresentation::Stored {
                size: right_size,
                alignment: right_alignment,
                class: right_class,
            },
        ) => {
            left_size == right_size
                && left_alignment == right_alignment
                && left_class == right_class
        }
        _ => false,
    }
}
