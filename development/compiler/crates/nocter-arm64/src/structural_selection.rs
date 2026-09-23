use nocter_machine::{MachineIndex, MachineOperationId, MachineValueId};

use crate::{
    Arm64SelectedIndexAddressDomain, Arm64SelectedInstruction, Arm64SelectedRegister,
    Arm64SelectionError, Arm64ValuePlan,
};

pub(crate) fn select_index_borrow(
    index: nocter_machine::MachineIndexBorrow,
    result: MachineValueId,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let receiver = crate::selection::direct_value(values, index.receiver())?;
    let domain = match index.domain() {
        nocter_machine::MachineIndexDomain::Fixed { length, stride } => {
            let [pointer] = receiver else {
                return Err(Arm64SelectionError::MemoryShape(index.receiver()));
            };
            Arm64SelectedIndexAddressDomain::Fixed {
                pointer: Arm64SelectedRegister::Virtual(*pointer),
                length,
                stride,
            }
        }
        nocter_machine::MachineIndexDomain::View {
            pointer_offset,
            length_offset,
            stride,
        } => Arm64SelectedIndexAddressDomain::View {
            pointer: Arm64SelectedRegister::Virtual(
                receiver[crate::memory_selection::direct_lane(pointer_offset, receiver.len())?],
            ),
            length: Arm64SelectedRegister::Virtual(
                receiver[crate::memory_selection::direct_lane(length_offset, receiver.len())?],
            ),
            stride,
        },
    };
    selected.push(Arm64SelectedInstruction::IndexAddress {
        destination: one_word(values, result)?,
        index: match index.index() {
            MachineIndex::Constant(index) => crate::Arm64SelectedIndex::Constant(index),
            MachineIndex::Value(value) => {
                crate::Arm64SelectedIndex::Register(one_word(values, value)?)
            }
        },
        domain,
        check: index.check(),
    });
    Ok(())
}

pub(crate) fn select_storage_alias(
    operation: MachineOperationId,
    source: MachineValueId,
    result: MachineValueId,
    values: &Arm64ValuePlan,
) -> Result<(), Arm64SelectionError> {
    if values.value(source) != values.value(result) {
        return Err(Arm64SelectionError::StorageAlias(operation));
    }
    Ok(())
}

fn one_word(
    values: &Arm64ValuePlan,
    value: MachineValueId,
) -> Result<Arm64SelectedRegister, Arm64SelectionError> {
    match crate::selection::direct_value(values, value)? {
        [register] => Ok(Arm64SelectedRegister::Virtual(*register)),
        _ => Err(Arm64SelectionError::ExpectedOneWord(value)),
    }
}
