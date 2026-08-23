use nocter_machine::{MachineOperationId, MachineOperationKind, MachineValueId};

use crate::{
    Arm64DataSize, Arm64NocterAbi, Arm64SelectedIndexAddressDomain, Arm64SelectedInstruction,
    Arm64SelectedRegister, Arm64SelectionError, Arm64ValuePlan,
};

pub(crate) fn select_operation(
    operation_id: MachineOperationId,
    operation: &nocter_machine::MachineOperation,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let result = operation
        .result()
        .ok_or(Arm64SelectionError::MissingResult(operation_id))?;
    match operation.kind() {
        MachineOperationKind::IndexBorrow(index) => {
            select_index_borrow(*index, result, values, selected)
        }
        MachineOperationKind::BorrowWeakening { source } => {
            select_direct_copy(operation_id, *source, result, values, selected)
        }
        _ => unreachable!("the caller classifies structural operations exhaustively"),
    }
}

fn select_index_borrow(
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
        index: one_word(values, index.index())?,
        domain,
    });
    Ok(())
}

fn select_direct_copy(
    operation: MachineOperationId,
    source: MachineValueId,
    result: MachineValueId,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let sources = crate::selection::direct_value(values, source)?;
    let destinations = crate::selection::direct_value(values, result)?;
    if sources.len() != destinations.len()
        || sources.len() > usize::from(Arm64NocterAbi::direct_value_word_limit())
    {
        return Err(Arm64SelectionError::DirectCopy(operation));
    }
    for (lane, source) in sources.iter().copied().enumerate() {
        selected.push(Arm64SelectedInstruction::Move {
            size: Arm64DataSize::Bits64,
            destination: Arm64SelectedRegister::Fixed(argument_register(lane)?),
            source: Arm64SelectedRegister::Virtual(source),
        });
    }
    for (lane, destination) in destinations.iter().copied().enumerate() {
        selected.push(Arm64SelectedInstruction::Move {
            size: Arm64DataSize::Bits64,
            destination: Arm64SelectedRegister::Virtual(destination),
            source: Arm64SelectedRegister::Fixed(argument_register(lane)?),
        });
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

fn argument_register(lane: usize) -> Result<crate::Arm64Register, Arm64SelectionError> {
    u8::try_from(lane)
        .ok()
        .and_then(Arm64NocterAbi::argument_register)
        .ok_or(Arm64SelectionError::RegisterOverflow)
}
