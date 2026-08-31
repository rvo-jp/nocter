use nocter_machine::{
    MachineArgumentLocation, MachineCallableAbi, MachineOperationId, MachinePrimitiveTarget,
    MachineResultAbi, MachineResultLocation, MachineValueClass,
};
use nocter_runtime_contract::PrimitiveRole;

use crate::{
    Arm64DataSize, Arm64NocterAbi, Arm64SelectedBinaryOperation, Arm64SelectedInstruction,
    Arm64SelectedLoadExtension, Arm64SelectedMemoryAddress, Arm64SelectedRegister,
    Arm64SelectionError,
};

/// A primitive call paired with its canonical machine ABI entry.
///
/// Resolution happens once at the selection boundary. Role-specific selectors receive this closed
/// view and cannot reinterpret a signature or consult MIR metadata.
#[derive(Clone, Copy)]
pub(crate) struct Arm64PrimitiveTarget<'program> {
    target: &'program MachinePrimitiveTarget,
    abi: &'program MachineCallableAbi,
}

impl<'program> Arm64PrimitiveTarget<'program> {
    pub(crate) fn resolve(
        program: &'program nocter_machine::MachineProgram,
        target: &'program MachinePrimitiveTarget,
    ) -> Option<Self> {
        Some(Self {
            target,
            abi: program.primitive_abi(target)?,
        })
    }

    pub(crate) const fn abi(self) -> &'program MachineCallableAbi {
        self.abi
    }
}

impl std::ops::Deref for Arm64PrimitiveTarget<'_> {
    type Target = MachinePrimitiveTarget;

    fn deref(&self) -> &Self::Target {
        self.target
    }
}

/// Expands one closed primitive role while preserving its ordinary Nocter ABI boundary.
pub(crate) fn select(
    program: &nocter_machine::MachineProgram,
    frame: &crate::Arm64FunctionFrame,
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    match target.role() {
        PrimitiveRole::NewError
        | PrimitiveRole::ErrorContext
        | PrimitiveRole::ErrorCode
        | PrimitiveRole::ErrorMessage
        | PrimitiveRole::AllocationFailureError => {
            crate::error_selection::select_primitive(operation, target, frame, selected)
        }
        PrimitiveRole::ProcessArgumentCount
        | PrimitiveRole::ProcessArgument
        | PrimitiveRole::ProcessEnvironmentCount
        | PrimitiveRole::ProcessEnvironmentName
        | PrimitiveRole::ProcessEnvironmentValue => {
            crate::process_selection::select_primitive(operation, target, selected)
        }
        PrimitiveRole::CurrentAllocatorState | PrimitiveRole::CurrentAllocatorKind => {
            select_context_reader(operation, target, selected)
        }
        PrimitiveRole::PointerAddress
        | PrimitiveRole::PointerFromReference
        | PrimitiveRole::PointerFromReadWriteReference
        | PrimitiveRole::PointerFromAddress => {
            validate_register_abi(operation, target, &[1], 1)?;
            validate_type_arguments(operation, target, 1)
        }
        PrimitiveRole::PointeeSize | PrimitiveRole::PointeeAlignment => {
            select_pointee_layout(program, operation, target, selected)
        }
        PrimitiveRole::CopyStringToPointer
        | PrimitiveRole::CopyPointerToPointer
        | PrimitiveRole::StoreByteToPointer
        | PrimitiveRole::StoreValueToPointer
        | PrimitiveRole::TakeValueAtPointer => {
            super::primitive_memory_selection::select(program, operation, target, selected)
        }
        PrimitiveRole::DropValueAtPointer => select_noop_destruction(operation, target),
        PrimitiveRole::StringFromRawParts
        | PrimitiveRole::ByteSliceFromRawParts
        | PrimitiveRole::MutableByteSliceFromRawParts => {
            validate_register_abi(operation, target, &[1, 1], 2)?;
            validate_type_arguments(operation, target, 0)
        }
        PrimitiveRole::ValueSliceFromRawParts | PrimitiveRole::MutableValueSliceFromRawParts => {
            validate_register_abi(operation, target, &[1, 1], 2)?;
            validate_type_arguments(operation, target, 1)
        }
        PrimitiveRole::BytesFromString => {
            validate_register_abi(operation, target, &[2], 2)?;
            validate_type_arguments(operation, target, 0)
        }
        PrimitiveRole::StringSubviewUnchecked => select_string_subview(operation, target, selected),
        PrimitiveRole::SliceLength | PrimitiveRole::StringLength => {
            select_view_length(operation, target, selected)
        }
        PrimitiveRole::SlicePointerAddress | PrimitiveRole::StringPointerAddress => {
            validate_register_abi(operation, target, &[2], 1)?;
            validate_view_type_arguments(operation, target)
        }
        PrimitiveRole::U8Truncate
        | PrimitiveRole::U16Truncate
        | PrimitiveRole::U32Truncate
        | PrimitiveRole::I8Truncate
        | PrimitiveRole::I16Truncate
        | PrimitiveRole::I32Truncate => {
            validate_register_abi(operation, target, &[1], 1)?;
            validate_type_arguments(operation, target, 0)
        }
        PrimitiveRole::U64WrappingAdd
        | PrimitiveRole::U64WrappingMultiply
        | PrimitiveRole::U64BitwiseXor
        | PrimitiveRole::U64RotateRight => select_u64_mixing(operation, target, selected),
        PrimitiveRole::AllocationAbort
        | PrimitiveRole::ProcessExit
        | PrimitiveRole::Syscall0
        | PrimitiveRole::Syscall1
        | PrimitiveRole::Syscall2
        | PrimitiveRole::Syscall3
        | PrimitiveRole::Syscall4
        | PrimitiveRole::Syscall5
        | PrimitiveRole::Syscall6
        | PrimitiveRole::Trap
        | PrimitiveRole::Unreachable => {
            super::system_primitive_selection::select(operation, target, selected)
        }
    }
}

fn select_u64_mixing(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_register_abi(operation, target, &[1, 1], 1)?;
    validate_type_arguments(operation, target, 0)?;
    let operation = match target.role() {
        PrimitiveRole::U64WrappingAdd => Arm64SelectedBinaryOperation::Add,
        PrimitiveRole::U64WrappingMultiply => Arm64SelectedBinaryOperation::Multiply,
        PrimitiveRole::U64BitwiseXor => Arm64SelectedBinaryOperation::BitwiseXor,
        PrimitiveRole::U64RotateRight => Arm64SelectedBinaryOperation::RotateRight,
        _ => return Err(Arm64SelectionError::PrimitiveCall(operation)),
    };
    selected.push(Arm64SelectedInstruction::Binary {
        size: Arm64DataSize::Bits64,
        operation,
        destination: fixed_register(0)?,
        left: fixed_register(0)?,
        right: fixed_register(1)?,
    });
    Ok(())
}

fn select_noop_destruction(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
) -> Result<(), Arm64SelectionError> {
    validate_type_arguments(operation, target, 1)?;
    let nocter_machine::MachinePrimitiveDependency::NoopDestruction { subject } =
        target.dependency()
    else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    if target.type_arguments() != [*subject]
        || target.abi().arguments().len() != 2
        || target.abi().pack().is_some()
        || target.abi().stack_argument_size() != 0
        || target.abi().result() != MachineResultAbi::Completion
    {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    for (first, argument) in target.abi().arguments().iter().enumerate() {
        let first =
            u8::try_from(first).map_err(|_| Arm64SelectionError::PrimitiveCall(operation))?;
        let Some(MachineArgumentLocation::Registers(registers)) = argument.location() else {
            return Err(Arm64SelectionError::PrimitiveCall(operation));
        };
        if argument.class() != (MachineValueClass::Direct { words: 1 })
            || registers.first() != first
            || registers.words() != 1
        {
            return Err(Arm64SelectionError::PrimitiveCall(operation));
        }
    }
    Ok(())
}

fn select_context_reader(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_register_abi(operation, target, &[], 1)?;
    validate_type_arguments(operation, target, 0)?;
    let offset = if target.role() == PrimitiveRole::CurrentAllocatorState {
        0
    } else {
        Arm64NocterAbi::word_size()
    };
    selected.push(Arm64SelectedInstruction::LoadMemory {
        bytes: word_bytes(),
        extension: Arm64SelectedLoadExtension::Zero,
        destination: fixed_register(0)?,
        source: Arm64SelectedMemoryAddress::Register {
            base: Arm64SelectedRegister::Fixed(Arm64NocterAbi::allocation_context_register()),
            offset,
        },
    });
    Ok(())
}

fn select_pointee_layout(
    program: &nocter_machine::MachineProgram,
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_register_abi(operation, target, &[1], 1)?;
    validate_type_arguments(operation, target, 1)?;
    let layout = program
        .layouts()
        .get(target.type_arguments()[0])
        .ok_or(Arm64SelectionError::PrimitiveCall(operation))?;
    let value = if target.role() == PrimitiveRole::PointeeSize {
        layout.size()
    } else {
        layout.alignment()
    };
    selected.push(Arm64SelectedInstruction::LoadImmediate {
        size: Arm64DataSize::Bits64,
        destination: fixed_register(0)?,
        value,
    });
    Ok(())
}

fn select_string_subview(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_register_abi(operation, target, &[2, 1, 1], 2)?;
    validate_type_arguments(operation, target, 0)?;
    selected.push(Arm64SelectedInstruction::Binary {
        size: Arm64DataSize::Bits64,
        operation: Arm64SelectedBinaryOperation::Add,
        destination: fixed_register(0)?,
        left: fixed_register(0)?,
        right: fixed_register(2)?,
    });
    selected.push(Arm64SelectedInstruction::Move {
        size: Arm64DataSize::Bits64,
        destination: fixed_register(1)?,
        source: fixed_register(3)?,
    });
    Ok(())
}

fn select_view_length(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_register_abi(operation, target, &[2], 1)?;
    validate_view_type_arguments(operation, target)?;
    selected.push(Arm64SelectedInstruction::Move {
        size: Arm64DataSize::Bits64,
        destination: fixed_register(0)?,
        source: fixed_register(1)?,
    });
    Ok(())
}

fn validate_view_type_arguments(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
) -> Result<(), Arm64SelectionError> {
    let expected = usize::from(matches!(
        target.role(),
        PrimitiveRole::SliceLength | PrimitiveRole::SlicePointerAddress
    ));
    validate_type_arguments(operation, target, expected)
}

pub(super) fn validate_type_arguments(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
    expected: usize,
) -> Result<(), Arm64SelectionError> {
    if target.type_arguments().len() == expected {
        Ok(())
    } else {
        Err(Arm64SelectionError::PrimitiveCall(operation))
    }
}

fn validate_register_abi(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
    argument_words: &[u8],
    result_words: u8,
) -> Result<(), Arm64SelectionError> {
    if target.abi().arguments().len() != argument_words.len()
        || target.abi().pack().is_some()
        || target.abi().stack_argument_size() != 0
    {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    let mut first = 0_u8;
    for (argument, expected_words) in target.abi().arguments().iter().zip(argument_words) {
        let Some(MachineArgumentLocation::Registers(registers)) = argument.location() else {
            return Err(Arm64SelectionError::PrimitiveCall(operation));
        };
        if argument.class()
            != (MachineValueClass::Direct {
                words: *expected_words,
            })
            || registers.first() != first
            || registers.words() != *expected_words
        {
            return Err(Arm64SelectionError::PrimitiveCall(operation));
        }
        first = first
            .checked_add(*expected_words)
            .ok_or(Arm64SelectionError::PrimitiveCall(operation))?;
    }
    let MachineResultAbi::Value(result) = target.abi().result() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    let MachineResultLocation::Registers(registers) = result.location() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    if result.class()
        != (MachineValueClass::Direct {
            words: result_words,
        })
        || registers.first() != 0
        || registers.words() != result_words
    {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    Ok(())
}

pub(super) fn fixed_register(index: u8) -> Result<Arm64SelectedRegister, Arm64SelectionError> {
    Arm64NocterAbi::argument_register(index)
        .map(Arm64SelectedRegister::Fixed)
        .ok_or(Arm64SelectionError::RegisterOverflow)
}

fn word_bytes() -> u8 {
    u8::try_from(Arm64NocterAbi::word_size())
        .expect("the target word size fits selected byte width")
}
