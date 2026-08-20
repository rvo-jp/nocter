use nocter_machine::{
    MachineArgumentLocation, MachineOperationId, MachinePrimitiveTarget, MachineResultAbi,
    MachineResultLocation, MachineValueClass, PrimitiveRole,
};

use crate::{Arm64SelectedInstruction, Arm64SelectionError};

pub(super) fn select(
    operation: MachineOperationId,
    target: &MachinePrimitiveTarget,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    match target.role() {
        PrimitiveRole::AllocationAbort => select_break(operation, target, selected),
        PrimitiveRole::ProcessExit => select_exit(operation, target, selected),
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

fn select_exit(
    operation: MachineOperationId,
    target: &MachinePrimitiveTarget,
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
    target: &MachinePrimitiveTarget,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let argument_count = syscall_argument_count(target.role())
        .ok_or(Arm64SelectionError::PrimitiveCall(operation))?;
    validate_ordinary_inputs(operation, target, argument_count + 1)?;
    let MachineResultAbi::Value(result) = target.abi().result() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    let MachineResultLocation::Registers(registers) = result.location() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    if result.class() != (MachineValueClass::Direct { words: 2 })
        || registers.first() != 0
        || registers.words() != 2
    {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    selected.push(Arm64SelectedInstruction::DarwinSystemCall { argument_count });
    Ok(())
}

fn select_break(
    operation: MachineOperationId,
    target: &MachinePrimitiveTarget,
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
    target: &MachinePrimitiveTarget,
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
    target: &MachinePrimitiveTarget,
) -> Result<(), Arm64SelectionError> {
    if target.abi().result() == MachineResultAbi::Diverging {
        Ok(())
    } else {
        Err(Arm64SelectionError::PrimitiveCall(operation))
    }
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
