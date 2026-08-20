use nocter_machine::{
    MachineAddressId, MachineArgumentLocation, MachineDropFlagId, MachineFunction,
    MachineFunctionId, MachineFunctionKind, MachineOperationId, MachineResultAbi,
    MachineValueClass,
};

use crate::{
    Arm64DataSize, Arm64FunctionFrame, Arm64NocterAbi, Arm64SelectedAddressPlan,
    Arm64SelectedInstruction, Arm64SelectedLoadExtension, Arm64SelectedMemoryAddress,
    Arm64SelectedRegister, Arm64SelectedStackAddress, Arm64SelectionError,
};

/// Initializes every conditional ownership bit before source operations begin.
pub(crate) fn select_entry(
    function: &MachineFunction,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    for (flag, definition) in function.body().drop_flags() {
        select_flag_write(flag, definition.initially_initialized(), frame, selected)?;
    }
    Ok(())
}

pub(crate) fn select_flag_write(
    flag: MachineDropFlagId,
    initialized: bool,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let source = scratch();
    selected.push(Arm64SelectedInstruction::LoadImmediate {
        size: Arm64DataSize::Bits32,
        destination: source,
        value: u64::from(initialized),
    });
    selected.push(Arm64SelectedInstruction::StoreMemory {
        bytes: 1,
        destination: flag_address(flag, frame)?,
        source,
    });
    Ok(())
}

pub(crate) fn select_flag_read(
    flag: MachineDropFlagId,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<Arm64SelectedRegister, Arm64SelectionError> {
    let destination = scratch();
    selected.push(Arm64SelectedInstruction::LoadMemory {
        bytes: 1,
        extension: Arm64SelectedLoadExtension::Zero,
        destination,
        source: flag_address(flag, frame)?,
    });
    Ok(destination)
}

pub(crate) fn select_drop(
    program: &nocter_machine::MachineProgram,
    operation: MachineOperationId,
    target: MachineFunctionId,
    place: MachineAddressId,
    frame: &Arm64FunctionFrame,
    addresses: &Arm64SelectedAddressPlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_drop_abi(program, operation, target)?;
    crate::allocation_selection::select_inherited_target(
        program, operation, target, frame, selected,
    )?;
    let place = addresses.use_address(place, selected)?;
    selected.push(Arm64SelectedInstruction::MemoryAddress {
        destination: Arm64SelectedRegister::Fixed(argument_register()?),
        source: place,
    });
    selected.push(Arm64SelectedInstruction::Call(target));
    Ok(())
}

fn validate_drop_abi(
    program: &nocter_machine::MachineProgram,
    operation: MachineOperationId,
    target: MachineFunctionId,
) -> Result<(), Arm64SelectionError> {
    let function = program
        .function(target)
        .ok_or(Arm64SelectionError::UnknownFunction(target))?;
    let MachineFunctionKind::Callable(abi) = function.kind() else {
        return Err(Arm64SelectionError::NonCallableTarget(target));
    };
    let [argument] = abi.arguments() else {
        return Err(Arm64SelectionError::DropAbi(operation));
    };
    let Some(MachineArgumentLocation::Registers(registers)) = argument.location() else {
        return Err(Arm64SelectionError::DropAbi(operation));
    };
    if argument.class() != (MachineValueClass::Direct { words: 1 })
        || registers.first() != 0
        || registers.words() != 1
        || abi.pack().is_some()
        || abi.stack_argument_size() != 0
        || abi.result() != MachineResultAbi::Completion
    {
        return Err(Arm64SelectionError::DropAbi(operation));
    }
    Ok(())
}

fn flag_address(
    flag: MachineDropFlagId,
    frame: &Arm64FunctionFrame,
) -> Result<Arm64SelectedMemoryAddress, Arm64SelectionError> {
    frame
        .drop_flag(flag)
        .map(|object| {
            Arm64SelectedMemoryAddress::Stack(Arm64SelectedStackAddress::FrameObject {
                object,
                offset: 0,
            })
        })
        .ok_or(Arm64SelectionError::UnknownDropFlag(flag))
}

fn scratch() -> Arm64SelectedRegister {
    Arm64SelectedRegister::Fixed(
        Arm64NocterAbi::compiler_scratch_register(0)
            .expect("the ABI reserves x16 for compiler-owned flag transport"),
    )
}

fn argument_register() -> Result<crate::Arm64Register, Arm64SelectionError> {
    Arm64NocterAbi::argument_register(0).ok_or(Arm64SelectionError::RegisterOverflow)
}
