use nocter_machine::{
    MachineArgumentLocation, MachineOperationId, MachineResultAbi, MachineResultLocation,
    MachineValueClass,
};
use nocter_runtime_contract::PrimitiveRole;

use crate::{
    Arm64DataSize, Arm64NocterAbi, Arm64SelectedBinaryOperation, Arm64SelectedInstruction,
    Arm64SelectedMemoryAddress, Arm64SelectedRegister, Arm64SelectionError,
};

pub(super) fn select(
    program: &nocter_machine::MachineProgram,
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    match target.role() {
        PrimitiveRole::AllocationAbort => select_break(operation, target, selected),
        PrimitiveRole::ProcessExit => select_exit(operation, target, selected),
        PrimitiveRole::MonotonicCounterRead | PrimitiveRole::MonotonicCounterFrequency => {
            select_counter_read(operation, target, selected)
        }
        PrimitiveRole::MonotonicCounterDelta => select_counter_delta(operation, target, selected),
        PrimitiveRole::SyscallPair0 => select_syscall_pair(program, operation, target, selected),
        PrimitiveRole::Syscall0
        | PrimitiveRole::Syscall1
        | PrimitiveRole::Syscall2
        | PrimitiveRole::Syscall3
        | PrimitiveRole::Syscall4
        | PrimitiveRole::Syscall5
        | PrimitiveRole::Syscall6 => select_syscall(operation, target, selected),
        PrimitiveRole::Trap | PrimitiveRole::Unreachable => {
            select_break(operation, target, selected)
        }
        _ => Err(Arm64SelectionError::PrimitiveCall(operation)),
    }
}

fn select_syscall_pair(
    program: &nocter_machine::MachineProgram,
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_ordinary_inputs(operation, target, 1)?;
    validate_indirect_result(program, operation, target)?;
    selected.push(Arm64SelectedInstruction::DarwinSystemCallPair);
    for lane in 0..3 {
        selected.push(Arm64SelectedInstruction::StoreMemory {
            bytes: 8,
            destination: Arm64SelectedMemoryAddress::Register {
                base: Arm64SelectedRegister::Fixed(Arm64NocterAbi::indirect_result_register()),
                offset: u64::from(lane) * 8,
            },
            source: super::primitive_selection::fixed_register(lane)?,
        });
    }
    Ok(())
}

fn validate_indirect_result(
    program: &nocter_machine::MachineProgram,
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
) -> Result<(), Arm64SelectionError> {
    let MachineResultAbi::Value(result) = target.abi().result() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    if result.class() != MachineValueClass::Indirect
        || result.location()
            != (MachineResultLocation::CallerStorage {
                pointer_register: Arm64NocterAbi::indirect_result_register().number(),
            })
    {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    let Some(layout) = program.layouts().get(result.ty()) else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    if layout.size() != 24 || layout.alignment() != 8 {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    Ok(())
}

fn select_counter_read(
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_ordinary_inputs(operation, target, 0)?;
    validate_direct_result(operation, target, 1)?;
    selected.push(match target.role() {
        PrimitiveRole::MonotonicCounterRead => Arm64SelectedInstruction::ReadMonotonicCounter,
        PrimitiveRole::MonotonicCounterFrequency => {
            Arm64SelectedInstruction::ReadMonotonicCounterFrequency
        }
        _ => return Err(Arm64SelectionError::PrimitiveCall(operation)),
    });
    Ok(())
}

fn select_counter_delta(
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_ordinary_inputs(operation, target, 2)?;
    validate_direct_result(operation, target, 1)?;
    selected.push(Arm64SelectedInstruction::Binary {
        size: Arm64DataSize::Bits64,
        operation: Arm64SelectedBinaryOperation::Subtract,
        destination: super::primitive_selection::fixed_register(0)?,
        left: super::primitive_selection::fixed_register(1)?,
        right: super::primitive_selection::fixed_register(0)?,
    });
    Ok(())
}

fn select_exit(
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_ordinary_inputs(operation, target, 1)?;
    validate_diverging(operation, target)?;
    selected.push(Arm64SelectedInstruction::ExitProcess {
        status: super::primitive_selection::fixed_register(0)?,
    });
    Ok(())
}

fn select_syscall(
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let argument_count = syscall_argument_count(target.role())
        .ok_or(Arm64SelectionError::PrimitiveCall(operation))?;
    validate_ordinary_inputs(operation, target, argument_count + 1)?;
    validate_direct_result(operation, target, 2)?;
    selected.push(Arm64SelectedInstruction::DarwinSystemCall { argument_count });
    Ok(())
}

fn select_break(
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_ordinary_inputs(operation, target, 0)?;
    validate_diverging(operation, target)?;
    let reason = match target.role() {
        PrimitiveRole::Trap => crate::runtime_trap::Arm64RuntimeTrap::ExplicitTrap,
        PrimitiveRole::Unreachable => crate::runtime_trap::Arm64RuntimeTrap::ExplicitUnreachable,
        PrimitiveRole::AllocationAbort => crate::runtime_trap::Arm64RuntimeTrap::AllocationFailure,
        _ => return Err(Arm64SelectionError::PrimitiveCall(operation)),
    };
    selected.push(Arm64SelectedInstruction::Break {
        immediate: reason.immediate(),
    });
    Ok(())
}

fn validate_ordinary_inputs(
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    count: u8,
) -> Result<(), Arm64SelectionError> {
    super::primitive_selection::validate_type_arguments(operation, target, 0)?;
    if target.abi().arguments().len() != usize::from(count)
        || target.abi().pack().is_some()
        || target.abi().stack_argument_size() != 0
    {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    for (position, argument) in target.abi().arguments().iter().enumerate() {
        let Some(MachineArgumentLocation::Registers(registers)) = argument.location() else {
            return Err(Arm64SelectionError::PrimitiveCall(operation));
        };
        if argument.class() != (MachineValueClass::Direct { words: 1 })
            || registers.first()
                != u8::try_from(position)
                    .map_err(|_| Arm64SelectionError::PrimitiveCall(operation))?
            || registers.words() != 1
        {
            return Err(Arm64SelectionError::PrimitiveCall(operation));
        }
    }
    Ok(())
}

fn validate_diverging(
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
) -> Result<(), Arm64SelectionError> {
    if target.abi().result() == MachineResultAbi::Diverging {
        Ok(())
    } else {
        Err(Arm64SelectionError::PrimitiveCall(operation))
    }
}

fn validate_direct_result(
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    words: u8,
) -> Result<(), Arm64SelectionError> {
    let MachineResultAbi::Value(result) = target.abi().result() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    let MachineResultLocation::Registers(registers) = result.location() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    if result.class() != (MachineValueClass::Direct { words })
        || registers.first() != 0
        || registers.words() != words
    {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    Ok(())
}

fn syscall_argument_count(role: PrimitiveRole) -> Option<u8> {
    match role {
        PrimitiveRole::Syscall0 => Some(0),
        PrimitiveRole::Syscall1 => Some(1),
        PrimitiveRole::Syscall2 => Some(2),
        PrimitiveRole::Syscall3 => Some(3),
        PrimitiveRole::Syscall4 => Some(4),
        PrimitiveRole::Syscall5 => Some(5),
        PrimitiveRole::Syscall6 => Some(6),
        _ => None,
    }
}
