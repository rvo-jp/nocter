use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64Register,
    Arm64Shift,
};

#[derive(Clone, Copy)]
pub(crate) struct IntegerBinaryEmission {
    pub(crate) operation: crate::Arm64SelectedBinaryOperation,
    pub(crate) arithmetic: Option<crate::selection::Arm64IntegerArithmetic>,
    pub(crate) destination: Arm64Register,
    pub(crate) left: Arm64Register,
    pub(crate) right: Arm64Register,
    pub(crate) size: Arm64DataSize,
    pub(crate) required: bool,
}

pub(crate) fn emit_add_subtract(
    binary: IntegerBinaryEmission,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64MaterializationError> {
    code.append(Arm64Instruction::AddSubtractRegister {
        size: binary.size,
        operation: if binary.operation == crate::Arm64SelectedBinaryOperation::Add {
            Arm64AddSubtract::Add
        } else {
            Arm64AddSubtract::Subtract
        },
        set_flags: binary.required,
        destination: Arm64DataRegister::General(binary.destination),
        left: Arm64DataRegister::General(binary.left),
        right: Arm64DataRegister::General(binary.right),
    });
    if binary.required {
        let arithmetic = binary
            .arithmetic
            .expect("required arithmetic has integer semantics");
        if arithmetic.bits == native_bits(binary.size) {
            let condition = if arithmetic.signed {
                Arm64BranchCondition::Overflow
            } else if binary.operation == crate::Arm64SelectedBinaryOperation::Add {
                Arm64BranchCondition::CarrySet
            } else {
                Arm64BranchCondition::CarryClear
            };
            trap_if(code, condition)?;
        } else {
            emit_narrow_range_check(code, binary.destination, binary.size, arithmetic)?;
        }
    }
    if let Some(arithmetic) = binary.arithmetic {
        normalize_integer(code, binary.destination, binary.size, arithmetic);
    }
    Ok(())
}

pub(crate) fn emit_multiply(
    binary: IntegerBinaryEmission,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64MaterializationError> {
    if binary.required {
        emit_checked_multiply(
            code,
            binary.destination,
            binary.left,
            binary.right,
            binary.size,
            binary
                .arithmetic
                .expect("required arithmetic has integer semantics"),
        )?;
    } else {
        code.append(Arm64Instruction::MultiplyAdd {
            size: binary.size,
            destination: binary.destination,
            left: binary.left,
            right: binary.right,
            addend: Arm64DataRegister::Zero,
            subtract_product: false,
        });
    }
    if let Some(arithmetic) = binary.arithmetic {
        normalize_integer(code, binary.destination, binary.size, arithmetic);
    }
    Ok(())
}

pub(crate) fn emit_shift(binary: IntegerBinaryEmission, code: &mut Arm64CodeBuilder) {
    let operation = match binary.operation {
        crate::Arm64SelectedBinaryOperation::ShiftLeft => Arm64Shift::Left,
        crate::Arm64SelectedBinaryOperation::ShiftRight { signed: true } => {
            Arm64Shift::RightArithmetic
        }
        crate::Arm64SelectedBinaryOperation::ShiftRight { signed: false } => {
            Arm64Shift::RightLogical
        }
        crate::Arm64SelectedBinaryOperation::RotateRight => Arm64Shift::RotateRight,
        _ => unreachable!(),
    };
    code.append(Arm64Instruction::VariableShift {
        size: binary.size,
        operation,
        destination: binary.destination,
        value: binary.left,
        amount: binary.right,
    });
    if let Some(arithmetic) = binary.arithmetic {
        normalize_integer(code, binary.destination, binary.size, arithmetic);
    }
}

pub(crate) fn emit_precalculation_check(
    code: &mut Arm64CodeBuilder,
    operation: crate::Arm64SelectedBinaryOperation,
    left: Arm64Register,
    right: Arm64Register,
    size: Arm64DataSize,
    arithmetic: crate::selection::Arm64IntegerArithmetic,
) -> Result<(), crate::Arm64MaterializationError> {
    match operation {
        crate::Arm64SelectedBinaryOperation::Divide { .. }
        | crate::Arm64SelectedBinaryOperation::Remainder { .. } => {
            compare_with_zero(code, size, right);
            trap_if(code, Arm64BranchCondition::Equal)?;
            if arithmetic.signed {
                let valid_divisor = code.create_label();
                let scratch = crate::frame_access::scratch(2);
                crate::frame_access::load_immediate(code, scratch, u64::MAX, size);
                compare_registers(code, size, right, scratch);
                code.branch_conditional(valid_divisor, Arm64BranchCondition::NotEqual);
                crate::frame_access::load_immediate(
                    code,
                    scratch,
                    signed_minimum_pattern(arithmetic.bits, size),
                    size,
                );
                compare_registers(code, size, left, scratch);
                trap_if(code, Arm64BranchCondition::Equal)?;
                code.bind(valid_divisor)?;
            }
        }
        crate::Arm64SelectedBinaryOperation::ShiftLeft
        | crate::Arm64SelectedBinaryOperation::ShiftRight { .. } => {
            let scratch = crate::frame_access::scratch(2);
            crate::frame_access::load_immediate(code, scratch, u64::from(arithmetic.bits), size);
            compare_registers(code, size, right, scratch);
            trap_if(code, Arm64BranchCondition::CarrySet)?;
        }
        crate::Arm64SelectedBinaryOperation::Add
        | crate::Arm64SelectedBinaryOperation::Subtract
        | crate::Arm64SelectedBinaryOperation::Multiply
        | crate::Arm64SelectedBinaryOperation::MultiplyHigh
        | crate::Arm64SelectedBinaryOperation::BitwiseXor
        | crate::Arm64SelectedBinaryOperation::BitwiseAnd
        | crate::Arm64SelectedBinaryOperation::BitwiseOr
        | crate::Arm64SelectedBinaryOperation::RotateRight
        | crate::Arm64SelectedBinaryOperation::Equal
        | crate::Arm64SelectedBinaryOperation::Less { .. } => {}
    }
    Ok(())
}

pub(crate) fn normalize_integer(
    code: &mut Arm64CodeBuilder,
    register: Arm64Register,
    size: Arm64DataSize,
    arithmetic: crate::selection::Arm64IntegerArithmetic,
) {
    if arithmetic.bits < native_bits(size) {
        code.append(Arm64Instruction::BitfieldExtend {
            size,
            signed: arithmetic.signed,
            source_bits: arithmetic.bits,
            destination: register,
            source: register,
        });
    }
}

pub(crate) fn emit_trap(code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::Arithmetic.immediate(),
    });
}

pub(crate) fn trap_if(
    code: &mut Arm64CodeBuilder,
    condition: Arm64BranchCondition,
) -> Result<(), crate::Arm64MaterializationError> {
    let valid = code.create_label();
    code.branch_conditional(valid, condition.invert());
    emit_trap(code);
    code.bind(valid)?;
    Ok(())
}

pub(crate) const fn signed_minimum_pattern(bits: u8, size: Arm64DataSize) -> u64 {
    let value = 1_u64 << (bits - 1);
    if bits == native_bits(size) {
        value
    } else {
        let native_mask = match size {
            Arm64DataSize::Bits32 => u32::MAX as u64,
            Arm64DataSize::Bits64 => u64::MAX,
        };
        value | (native_mask << bits)
    }
}

fn emit_checked_multiply(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    left: Arm64Register,
    right: Arm64Register,
    size: Arm64DataSize,
    arithmetic: crate::selection::Arm64IntegerArithmetic,
) -> Result<(), crate::Arm64MaterializationError> {
    let high = crate::frame_access::scratch(2);
    if size == Arm64DataSize::Bits32 {
        code.append(Arm64Instruction::MultiplyLong {
            signed: arithmetic.signed,
            destination: high,
            left,
            right,
        });
        emit_range_check_with_scratch(code, high, Arm64DataSize::Bits64, arithmetic, destination)?;
        crate::address_code::move_register(code, high, destination);
        return Ok(());
    }
    code.append(Arm64Instruction::MultiplyHigh {
        signed: arithmetic.signed,
        destination: high,
        left,
        right,
    });
    code.append(Arm64Instruction::MultiplyAdd {
        size,
        destination,
        left,
        right,
        addend: Arm64DataRegister::Zero,
        subtract_product: false,
    });
    if arithmetic.signed {
        let negative = code.create_label();
        let valid = code.create_label();
        compare_with_zero(code, size, destination);
        code.branch_conditional(negative, Arm64BranchCondition::Minus);
        compare_with_zero(code, size, high);
        code.branch_conditional(valid, Arm64BranchCondition::Equal);
        emit_trap(code);
        code.bind(negative)?;
        code.append(Arm64Instruction::AddSubtractImmediate {
            size,
            operation: Arm64AddSubtract::Add,
            set_flags: true,
            destination: Arm64AddSubtractDestination::General(high),
            source: Arm64BaseRegister::General(high),
            immediate: 1,
            shift_12: false,
        });
        code.branch_conditional(valid, Arm64BranchCondition::Equal);
        emit_trap(code);
        code.bind(valid)?;
    } else {
        compare_with_zero(code, size, high);
        trap_if(code, Arm64BranchCondition::NotEqual)?;
    }
    Ok(())
}

fn emit_narrow_range_check(
    code: &mut Arm64CodeBuilder,
    value: Arm64Register,
    size: Arm64DataSize,
    arithmetic: crate::selection::Arm64IntegerArithmetic,
) -> Result<(), crate::Arm64MaterializationError> {
    emit_range_check_with_scratch(
        code,
        value,
        size,
        arithmetic,
        crate::frame_access::scratch(2),
    )
}

fn emit_range_check_with_scratch(
    code: &mut Arm64CodeBuilder,
    value: Arm64Register,
    size: Arm64DataSize,
    arithmetic: crate::selection::Arm64IntegerArithmetic,
    scratch: Arm64Register,
) -> Result<(), crate::Arm64MaterializationError> {
    if arithmetic.signed {
        let minimum = signed_minimum_pattern(arithmetic.bits, size);
        let maximum = (1_u64 << (arithmetic.bits - 1)) - 1;
        crate::frame_access::load_immediate(code, scratch, minimum, size);
        compare_registers(code, size, value, scratch);
        trap_if(code, Arm64BranchCondition::SignedLess)?;
        crate::frame_access::load_immediate(code, scratch, maximum, size);
        compare_registers(code, size, value, scratch);
        trap_if(code, Arm64BranchCondition::SignedGreater)?;
    } else {
        let maximum = (1_u64 << arithmetic.bits) - 1;
        crate::frame_access::load_immediate(code, scratch, maximum, size);
        compare_registers(code, size, value, scratch);
        trap_if(code, Arm64BranchCondition::UnsignedHigher)?;
    }
    Ok(())
}

const fn native_bits(size: Arm64DataSize) -> u8 {
    match size {
        Arm64DataSize::Bits32 => 32,
        Arm64DataSize::Bits64 => 64,
    }
}

fn compare_with_zero(code: &mut Arm64CodeBuilder, size: Arm64DataSize, value: Arm64Register) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: 0,
        shift_12: false,
    });
}

fn compare_registers(
    code: &mut Arm64CodeBuilder,
    size: Arm64DataSize,
    left: Arm64Register,
    right: Arm64Register,
) {
    code.append(Arm64Instruction::AddSubtractRegister {
        size,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(left),
        right: Arm64DataRegister::General(right),
    });
}
