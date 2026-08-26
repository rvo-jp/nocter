use nocter_machine::{
    MachineArgumentAbi, MachineArgumentLocation, MachineLayout, MachineOperationId,
    MachineResultAbi, MachineResultLocation, MachineValueClass,
};
use nocter_runtime_contract::PrimitiveRole;

use crate::{
    Arm64DataSize, Arm64NocterAbi, Arm64SelectedBinaryOperation, Arm64SelectedInstruction,
    Arm64SelectedLoadExtension, Arm64SelectedMemoryAddress, Arm64SelectedRegister,
    Arm64SelectionError,
};

pub(super) fn select(
    program: &nocter_machine::MachineProgram,
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    match target.role() {
        PrimitiveRole::CopyStringToPointer => select_string_copy(operation, target, selected),
        PrimitiveRole::CopyPointerToPointer => select_pointer_copy(operation, target, selected),
        PrimitiveRole::StoreByteToPointer => select_byte_store(operation, target, selected),
        PrimitiveRole::StoreValueToPointer => {
            select_value_store(program, operation, target, selected)
        }
        PrimitiveRole::TakeValueAtPointer => {
            select_value_take(program, operation, target, selected)
        }
        _ => Err(Arm64SelectionError::PrimitiveCall(operation)),
    }
}

fn select_string_copy(
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_completion_abi(operation, target, &[1, 1, 2])?;
    super::primitive_selection::validate_type_arguments(operation, target, 0)?;
    let destination = select_dynamic_address(selected)?;
    selected.push(Arm64SelectedInstruction::CopyMemoryNonOverlappingDynamic {
        destination,
        source: fixed(2)?,
        bytes: fixed(3)?,
    });
    Ok(())
}

fn select_pointer_copy(
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_completion_abi(operation, target, &[1, 1, 1])?;
    super::primitive_selection::validate_type_arguments(operation, target, 0)?;
    selected.push(Arm64SelectedInstruction::CopyMemoryNonOverlappingDynamic {
        destination: fixed(0)?,
        source: fixed(1)?,
        bytes: fixed(2)?,
    });
    Ok(())
}

fn select_byte_store(
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_completion_abi(operation, target, &[1, 1, 1])?;
    super::primitive_selection::validate_type_arguments(operation, target, 0)?;
    let address = select_dynamic_address(selected)?;
    selected.push(Arm64SelectedInstruction::StoreMemory {
        bytes: 1,
        destination: Arm64SelectedMemoryAddress::Register {
            base: address,
            offset: 0,
        },
        source: fixed(2)?,
    });
    Ok(())
}

fn select_value_store(
    program: &nocter_machine::MachineProgram,
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let layout = value_layout(program, operation, target)?;
    validate_common_abi(operation, target, 3)?;
    require_register_argument(operation, target.abi().arguments()[0], 0, direct(1))?;
    require_register_argument(operation, target.abi().arguments()[1], 1, direct(1))?;
    require_value_argument(
        operation,
        target.abi().arguments()[2],
        2,
        value_class(program, layout),
    )?;
    if target.abi().result() != MachineResultAbi::Completion {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    let address = select_dynamic_address(selected)?;
    match value_class(program, layout) {
        MachineValueClass::Zero => Ok(()),
        MachineValueClass::Direct { words } => {
            let sizes =
                crate::memory_selection::direct_lane_sizes(layout.size(), usize::from(words))?;
            for (lane, bytes) in sizes.into_iter().enumerate() {
                selected.push(Arm64SelectedInstruction::StoreMemory {
                    bytes,
                    destination: Arm64SelectedMemoryAddress::Register {
                        base: address,
                        offset: crate::memory_selection::lane_offset(lane)?,
                    },
                    source: fixed(
                        2_u8.checked_add(
                            u8::try_from(lane)
                                .map_err(|_| Arm64SelectionError::PrimitiveCall(operation))?,
                        )
                        .ok_or(Arm64SelectionError::PrimitiveCall(operation))?,
                    )?,
                });
            }
            Ok(())
        }
        MachineValueClass::Indirect => {
            selected.push(Arm64SelectedInstruction::CopyMemoryNonOverlapping {
                destination: Arm64SelectedMemoryAddress::Register {
                    base: address,
                    offset: 0,
                },
                source: Arm64SelectedMemoryAddress::Register {
                    base: fixed(2)?,
                    offset: 0,
                },
                bytes: layout.size(),
            });
            Ok(())
        }
    }
}

fn select_value_take(
    program: &nocter_machine::MachineProgram,
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let layout = value_layout(program, operation, target)?;
    validate_common_abi(operation, target, 2)?;
    require_register_argument(operation, target.abi().arguments()[0], 0, direct(1))?;
    require_register_argument(operation, target.abi().arguments()[1], 1, direct(1))?;
    let result = match target.abi().result() {
        MachineResultAbi::Value(result) if result.class() == value_class(program, layout) => result,
        _ => return Err(Arm64SelectionError::PrimitiveCall(operation)),
    };
    let address = select_dynamic_address(selected)?;
    match (value_class(program, layout), result.location()) {
        (MachineValueClass::Zero, MachineResultLocation::Omitted) => Ok(()),
        (MachineValueClass::Direct { words }, MachineResultLocation::Registers(registers))
            if registers.first() == 0 && registers.words() == words =>
        {
            let sizes =
                crate::memory_selection::direct_lane_sizes(layout.size(), usize::from(words))?;
            for (lane, bytes) in sizes.into_iter().enumerate() {
                selected.push(Arm64SelectedInstruction::LoadMemory {
                    bytes,
                    extension: Arm64SelectedLoadExtension::Zero,
                    destination: fixed(
                        u8::try_from(lane)
                            .map_err(|_| Arm64SelectionError::PrimitiveCall(operation))?,
                    )?,
                    source: Arm64SelectedMemoryAddress::Register {
                        base: address,
                        offset: crate::memory_selection::lane_offset(lane)?,
                    },
                });
            }
            Ok(())
        }
        (
            MachineValueClass::Indirect,
            MachineResultLocation::CallerStorage { pointer_register },
        ) if pointer_register == Arm64NocterAbi::indirect_result_register().number() => {
            selected.push(Arm64SelectedInstruction::CopyMemoryNonOverlapping {
                destination: Arm64SelectedMemoryAddress::Register {
                    base: Arm64SelectedRegister::Fixed(Arm64NocterAbi::indirect_result_register()),
                    offset: 0,
                },
                source: Arm64SelectedMemoryAddress::Register {
                    base: address,
                    offset: 0,
                },
                bytes: layout.size(),
            });
            Ok(())
        }
        _ => Err(Arm64SelectionError::PrimitiveCall(operation)),
    }
}

fn value_layout<'program>(
    program: &'program nocter_machine::MachineProgram,
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
) -> Result<&'program MachineLayout, Arm64SelectionError> {
    super::primitive_selection::validate_type_arguments(operation, target, 1)?;
    program
        .layouts()
        .get(target.type_arguments()[0])
        .ok_or(Arm64SelectionError::PrimitiveCall(operation))
}

fn validate_completion_abi(
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    argument_words: &[u8],
) -> Result<(), Arm64SelectionError> {
    validate_common_abi(operation, target, argument_words.len())?;
    let mut first = 0_u8;
    for (argument, words) in target.abi().arguments().iter().zip(argument_words) {
        require_register_argument(operation, *argument, first, direct(*words))?;
        first = first
            .checked_add(*words)
            .ok_or(Arm64SelectionError::PrimitiveCall(operation))?;
    }
    if target.abi().result() == MachineResultAbi::Completion {
        Ok(())
    } else {
        Err(Arm64SelectionError::PrimitiveCall(operation))
    }
}

fn validate_common_abi(
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    arguments: usize,
) -> Result<(), Arm64SelectionError> {
    if target.abi().arguments().len() == arguments
        && target.abi().pack().is_none()
        && target.abi().stack_argument_size() == 0
    {
        Ok(())
    } else {
        Err(Arm64SelectionError::PrimitiveCall(operation))
    }
}

fn require_value_argument(
    operation: MachineOperationId,
    argument: MachineArgumentAbi,
    first: u8,
    class: MachineValueClass,
) -> Result<(), Arm64SelectionError> {
    require_register_argument(operation, argument, first, class)
}

fn require_register_argument(
    operation: MachineOperationId,
    argument: MachineArgumentAbi,
    first: u8,
    class: MachineValueClass,
) -> Result<(), Arm64SelectionError> {
    if argument.class() != class {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    let transport_words = match class {
        MachineValueClass::Zero => 0,
        MachineValueClass::Direct { words } => words,
        MachineValueClass::Indirect => 1,
    };
    match (class, argument.location()) {
        (MachineValueClass::Zero, None) => Ok(()),
        (
            MachineValueClass::Direct { .. } | MachineValueClass::Indirect,
            Some(MachineArgumentLocation::Registers(registers)),
        ) if registers.first() == first && registers.words() == transport_words => Ok(()),
        _ => Err(Arm64SelectionError::PrimitiveCall(operation)),
    }
}

const fn direct(words: u8) -> MachineValueClass {
    MachineValueClass::Direct { words }
}

fn value_class(
    program: &nocter_machine::MachineProgram,
    layout: &MachineLayout,
) -> MachineValueClass {
    MachineValueClass::for_layout(layout, program.layouts().target())
}

fn select_dynamic_address(
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<Arm64SelectedRegister, Arm64SelectionError> {
    let address = Arm64SelectedRegister::Fixed(
        Arm64NocterAbi::compiler_scratch_register(1)
            .ok_or(Arm64SelectionError::RegisterOverflow)?,
    );
    selected.push(Arm64SelectedInstruction::Binary {
        size: Arm64DataSize::Bits64,
        operation: Arm64SelectedBinaryOperation::Add,
        destination: address,
        left: fixed(0)?,
        right: fixed(1)?,
    });
    Ok(address)
}

fn fixed(index: u8) -> Result<Arm64SelectedRegister, Arm64SelectionError> {
    super::primitive_selection::fixed_register(index)
}
