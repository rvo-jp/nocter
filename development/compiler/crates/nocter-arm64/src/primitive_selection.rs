use nocter_machine::{
    MachineArgumentLocation, MachineCallableAbi, MachineOperationId, MachinePrimitiveTarget,
    MachineResultAbi, MachineResultLocation, MachineValueClass,
};
use nocter_runtime_contract::PrimitiveRole;

use crate::{
    Arm64DataSize, Arm64FloatRounding, Arm64NocterAbi, Arm64SelectedBinaryOperation,
    Arm64SelectedInstruction, Arm64SelectedLoadExtension, Arm64SelectedMemoryAddress,
    Arm64SelectedRegister, Arm64SelectionError,
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
        | PrimitiveRole::PointerFromAddress => select_pointer_identity(operation, target),
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
        | PrimitiveRole::MutableByteSliceFromRawParts => select_raw_view(operation, target),
        PrimitiveRole::ValueSliceFromRawParts | PrimitiveRole::MutableValueSliceFromRawParts => {
            validate_register_abi(operation, target, &[1, 1], 2)?;
            validate_type_arguments(operation, target, 1)
        }
        PrimitiveRole::BytesFromString => select_bytes_from_string(operation, target),
        PrimitiveRole::StringSubviewUnchecked => select_string_subview(operation, target, selected),
        PrimitiveRole::SliceLength | PrimitiveRole::StringLength => {
            select_view_length(operation, target, selected)
        }
        PrimitiveRole::SlicePointerAddress | PrimitiveRole::StringPointerAddress => {
            validate_register_abi(operation, target, &[2], 1)?;
            validate_view_type_arguments(operation, target)
        }
        PrimitiveRole::CharacterFromU32Unchecked
        | PrimitiveRole::CharacterCodePoint
        | PrimitiveRole::U8Truncate
        | PrimitiveRole::U16Truncate
        | PrimitiveRole::U32Truncate
        | PrimitiveRole::I8Truncate
        | PrimitiveRole::I16Truncate
        | PrimitiveRole::I32Truncate => select_direct_unary(operation, target),
        PrimitiveRole::U64WrappingAdd
        | PrimitiveRole::U64WrappingSubtract
        | PrimitiveRole::U64WrappingMultiply
        | PrimitiveRole::U64MultiplyHigh
        | PrimitiveRole::U64BitwiseAnd
        | PrimitiveRole::U64BitwiseOr
        | PrimitiveRole::U64BitwiseXor
        | PrimitiveRole::U64RotateRight
        | PrimitiveRole::U64LeadingZeros => select_u64_primitive(operation, target, selected),
        PrimitiveRole::F32FromBits
        | PrimitiveRole::F32ToBits
        | PrimitiveRole::F64FromBits
        | PrimitiveRole::F64ToBits
        | PrimitiveRole::F32Floor
        | PrimitiveRole::F32Ceil
        | PrimitiveRole::F32Trunc
        | PrimitiveRole::F32RoundTiesEven
        | PrimitiveRole::F64Floor
        | PrimitiveRole::F64Ceil
        | PrimitiveRole::F64Trunc
        | PrimitiveRole::F64RoundTiesEven
        | PrimitiveRole::F64ToF32
        | PrimitiveRole::F64ToI64
        | PrimitiveRole::F64ToU64 => select_float_primitive(operation, target, selected),
        PrimitiveRole::AllocationAbort
        | PrimitiveRole::ProcessExit
        | PrimitiveRole::MonotonicCounterRead
        | PrimitiveRole::MonotonicCounterFrequency
        | PrimitiveRole::MonotonicCounterDelta
        | PrimitiveRole::Syscall0
        | PrimitiveRole::SyscallPair0
        | PrimitiveRole::Syscall1
        | PrimitiveRole::Syscall2
        | PrimitiveRole::Syscall3
        | PrimitiveRole::Syscall4
        | PrimitiveRole::Syscall5
        | PrimitiveRole::Syscall6
        | PrimitiveRole::Trap
        | PrimitiveRole::Unreachable => {
            super::system_primitive_selection::select(program, operation, target, selected)
        }
        PrimitiveRole::DescriptorReadiness => {
            validate_register_abi(operation, target, &[1, 1], 1)?;
            validate_type_arguments(operation, target, 0)?;
            selected.push(Arm64SelectedInstruction::ConstructDescriptorReadiness);
            Ok(())
        }
    }
}

fn select_pointer_identity(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
) -> Result<(), Arm64SelectionError> {
    validate_register_abi(operation, target, &[1], 1)?;
    validate_type_arguments(operation, target, 1)
}

fn select_raw_view(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
) -> Result<(), Arm64SelectionError> {
    validate_register_abi(operation, target, &[1, 1], 2)?;
    validate_type_arguments(operation, target, 0)
}

fn select_bytes_from_string(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
) -> Result<(), Arm64SelectionError> {
    validate_register_abi(operation, target, &[2], 2)?;
    validate_type_arguments(operation, target, 0)
}

fn select_float_primitive(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    match target.role() {
        PrimitiveRole::F32FromBits
        | PrimitiveRole::F32ToBits
        | PrimitiveRole::F64FromBits
        | PrimitiveRole::F64ToBits => select_float_bits(operation, target, selected),
        PrimitiveRole::F32Floor
        | PrimitiveRole::F32Ceil
        | PrimitiveRole::F32Trunc
        | PrimitiveRole::F32RoundTiesEven
        | PrimitiveRole::F64Floor
        | PrimitiveRole::F64Ceil
        | PrimitiveRole::F64Trunc
        | PrimitiveRole::F64RoundTiesEven => select_float_round(operation, target, selected),
        PrimitiveRole::F64ToF32 | PrimitiveRole::F64ToI64 | PrimitiveRole::F64ToU64 => {
            select_float_conversion(operation, target, selected)
        }
        _ => Err(Arm64SelectionError::PrimitiveCall(operation)),
    }
}

fn select_direct_unary(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
) -> Result<(), Arm64SelectionError> {
    validate_register_abi(operation, target, &[1], 1)?;
    validate_type_arguments(operation, target, 0)
}

fn select_float_bits(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_type_arguments(operation, target, 0)?;
    let (size, float_class, from_bits) = match target.role() {
        PrimitiveRole::F32FromBits => (Arm64DataSize::Bits32, MachineValueClass::Float32, true),
        PrimitiveRole::F32ToBits => (Arm64DataSize::Bits32, MachineValueClass::Float32, false),
        PrimitiveRole::F64FromBits => (Arm64DataSize::Bits64, MachineValueClass::Float64, true),
        PrimitiveRole::F64ToBits => (Arm64DataSize::Bits64, MachineValueClass::Float64, false),
        _ => return Err(Arm64SelectionError::PrimitiveCall(operation)),
    };
    validate_float_bitcast_abi(operation, target, float_class, from_bits)?;
    let general = fixed_register(0)?;
    let floating = Arm64NocterAbi::floating_argument_register(0)
        .map(crate::Arm64SelectedFloatRegister::Fixed)
        .ok_or(Arm64SelectionError::RegisterOverflow)?;
    selected.push(if from_bits {
        Arm64SelectedInstruction::FloatMoveFromGeneral {
            size,
            destination: floating,
            source: general,
        }
    } else {
        Arm64SelectedInstruction::FloatMoveToGeneral {
            size,
            destination: general,
            source: floating,
        }
    });
    Ok(())
}

fn select_float_round(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_type_arguments(operation, target, 0)?;
    let (size, float_class, rounding) = match target.role() {
        PrimitiveRole::F32Floor => (
            Arm64DataSize::Bits32,
            MachineValueClass::Float32,
            Arm64FloatRounding::Floor,
        ),
        PrimitiveRole::F32Ceil => (
            Arm64DataSize::Bits32,
            MachineValueClass::Float32,
            Arm64FloatRounding::Ceil,
        ),
        PrimitiveRole::F32Trunc => (
            Arm64DataSize::Bits32,
            MachineValueClass::Float32,
            Arm64FloatRounding::Truncate,
        ),
        PrimitiveRole::F32RoundTiesEven => (
            Arm64DataSize::Bits32,
            MachineValueClass::Float32,
            Arm64FloatRounding::TiesEven,
        ),
        PrimitiveRole::F64Floor => (
            Arm64DataSize::Bits64,
            MachineValueClass::Float64,
            Arm64FloatRounding::Floor,
        ),
        PrimitiveRole::F64Ceil => (
            Arm64DataSize::Bits64,
            MachineValueClass::Float64,
            Arm64FloatRounding::Ceil,
        ),
        PrimitiveRole::F64Trunc => (
            Arm64DataSize::Bits64,
            MachineValueClass::Float64,
            Arm64FloatRounding::Truncate,
        ),
        PrimitiveRole::F64RoundTiesEven => (
            Arm64DataSize::Bits64,
            MachineValueClass::Float64,
            Arm64FloatRounding::TiesEven,
        ),
        _ => return Err(Arm64SelectionError::PrimitiveCall(operation)),
    };
    validate_float_register_abi(operation, target, float_class, float_class)?;
    let register = Arm64NocterAbi::floating_argument_register(0)
        .map(crate::Arm64SelectedFloatRegister::Fixed)
        .ok_or(Arm64SelectionError::RegisterOverflow)?;
    selected.push(Arm64SelectedInstruction::FloatRound {
        size,
        operation: rounding,
        destination: register,
        source: register,
    });
    Ok(())
}

fn select_float_conversion(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_type_arguments(operation, target, 0)?;
    let source = Arm64NocterAbi::floating_argument_register(0)
        .map(crate::Arm64SelectedFloatRegister::Fixed)
        .ok_or(Arm64SelectionError::RegisterOverflow)?;
    match target.role() {
        PrimitiveRole::F64ToF32 => {
            validate_float_register_abi(
                operation,
                target,
                MachineValueClass::Float64,
                MachineValueClass::Float32,
            )?;
            selected.push(Arm64SelectedInstruction::FloatNarrow {
                destination: source,
                source,
            });
        }
        PrimitiveRole::F64ToI64 | PrimitiveRole::F64ToU64 => {
            validate_float_register_abi(
                operation,
                target,
                MachineValueClass::Float64,
                MachineValueClass::Direct { words: 1 },
            )?;
            selected.push(Arm64SelectedInstruction::FloatToInteger {
                signed: target.role() == PrimitiveRole::F64ToI64,
                destination: fixed_register(0)?,
                source,
            });
        }
        _ => return Err(Arm64SelectionError::PrimitiveCall(operation)),
    }
    Ok(())
}

fn validate_float_bitcast_abi(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
    float_class: MachineValueClass,
    from_bits: bool,
) -> Result<(), Arm64SelectionError> {
    let integer_class = MachineValueClass::Direct { words: 1 };
    let (expected_argument, expected_result) = if from_bits {
        (integer_class, float_class)
    } else {
        (float_class, integer_class)
    };
    validate_float_register_abi(operation, target, expected_argument, expected_result)
}

fn validate_float_register_abi(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
    expected_argument: MachineValueClass,
    expected_result: MachineValueClass,
) -> Result<(), Arm64SelectionError> {
    let [argument] = target.abi().arguments() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    let MachineResultAbi::Value(result) = target.abi().result() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    let Some(MachineArgumentLocation::Registers(argument_registers)) = argument.location() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    let MachineResultLocation::Registers(result_registers) = result.location() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    if target.abi().pack().is_some()
        || target.abi().stack_argument_size() != 0
        || argument.class() != expected_argument
        || result.class() != expected_result
        || argument_registers.first() != 0
        || argument_registers.words() != 1
        || result_registers.first() != 0
        || result_registers.words() != 1
    {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    Ok(())
}

fn select_u64_primitive(
    operation: MachineOperationId,
    target: Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    if target.role() == PrimitiveRole::U64LeadingZeros {
        validate_register_abi(operation, target, &[1], 1)?;
        validate_type_arguments(operation, target, 0)?;
        selected.push(Arm64SelectedInstruction::Unary {
            size: Arm64DataSize::Bits64,
            operation: crate::Arm64SelectedUnaryOperation::CountLeadingZeros,
            destination: fixed_register(0)?,
            operand: fixed_register(0)?,
        });
        return Ok(());
    }
    validate_register_abi(operation, target, &[1, 1], 1)?;
    validate_type_arguments(operation, target, 0)?;
    let operation = match target.role() {
        PrimitiveRole::U64WrappingAdd => Arm64SelectedBinaryOperation::Add,
        PrimitiveRole::U64WrappingSubtract => Arm64SelectedBinaryOperation::Subtract,
        PrimitiveRole::U64WrappingMultiply => Arm64SelectedBinaryOperation::Multiply,
        PrimitiveRole::U64MultiplyHigh => Arm64SelectedBinaryOperation::MultiplyHigh,
        PrimitiveRole::U64BitwiseAnd => Arm64SelectedBinaryOperation::BitwiseAnd,
        PrimitiveRole::U64BitwiseOr => Arm64SelectedBinaryOperation::BitwiseOr,
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
