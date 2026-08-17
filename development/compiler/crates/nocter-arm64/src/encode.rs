use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BranchCondition, Arm64DataSize,
    Arm64Instruction, Arm64LoadStoreSize, Arm64Logical, Arm64MoveWide, Arm64Shift,
};

pub(crate) fn encode(instruction: Arm64Instruction) -> Result<u32, Arm64EncodingError> {
    match instruction {
        instruction @ (Arm64Instruction::AddSubtractImmediate { .. }
        | Arm64Instruction::AddSubtractRegister { .. }
        | Arm64Instruction::LogicalRegister { .. }
        | Arm64Instruction::MoveWide { .. }
        | Arm64Instruction::MultiplyAdd { .. }
        | Arm64Instruction::Divide { .. }
        | Arm64Instruction::VariableShift { .. }) => encode_arithmetic(instruction),
        instruction @ (Arm64Instruction::LoadUnsigned { .. }
        | Arm64Instruction::StoreUnsigned { .. }) => encode_memory(instruction),
        instruction @ (Arm64Instruction::ConditionalSet { .. }
        | Arm64Instruction::Branch { .. }
        | Arm64Instruction::BranchConditional { .. }
        | Arm64Instruction::BranchRegister { .. }
        | Arm64Instruction::Return { .. }
        | Arm64Instruction::Break { .. }
        | Arm64Instruction::SupervisorCall { .. }) => encode_control(instruction),
    }
}

fn encode_arithmetic(instruction: Arm64Instruction) -> Result<u32, Arm64EncodingError> {
    match instruction {
        Arm64Instruction::AddSubtractImmediate {
            size,
            operation,
            set_flags,
            destination,
            source,
            immediate,
            shift_12,
        } => encode_add_subtract_immediate(
            size,
            operation,
            set_flags,
            destination,
            source.encoding(),
            immediate,
            shift_12,
        ),
        Arm64Instruction::AddSubtractRegister {
            size,
            operation,
            set_flags,
            destination,
            left,
            right,
        } => Ok(encode_add_subtract_register(
            size,
            operation,
            set_flags,
            destination.encoding(),
            left.encoding(),
            right.encoding(),
        )),
        Arm64Instruction::LogicalRegister {
            size,
            operation,
            destination,
            left,
            right,
        } => Ok(encode_logical_register(
            size,
            operation,
            destination.encoding(),
            left.encoding(),
            right.encoding(),
        )),
        Arm64Instruction::MoveWide {
            size,
            operation,
            destination,
            immediate,
            shift,
        } => encode_move_wide(size, operation, destination.number(), immediate, shift),
        Arm64Instruction::MultiplyAdd {
            size,
            destination,
            left,
            right,
            addend,
            subtract_product,
        } => Ok(encode_multiply_add(
            size,
            destination.number(),
            left.number(),
            right.number(),
            addend.encoding(),
            subtract_product,
        )),
        Arm64Instruction::Divide {
            size,
            destination,
            left,
            right,
            signed,
        } => Ok(encode_divide(
            size,
            destination.number(),
            left.number(),
            right.number(),
            signed,
        )),
        Arm64Instruction::VariableShift {
            size,
            operation,
            destination,
            value,
            amount,
        } => Ok(encode_variable_shift(
            size,
            operation,
            destination.number(),
            value.number(),
            amount.number(),
        )),
        _ => {
            unreachable!("instruction category is closed by encode")
        }
    }
}

fn encode_memory(instruction: Arm64Instruction) -> Result<u32, Arm64EncodingError> {
    match instruction {
        Arm64Instruction::LoadUnsigned {
            size,
            destination,
            base,
            offset,
        } => encode_load_store(size, true, destination.encoding(), base.encoding(), offset),
        Arm64Instruction::StoreUnsigned {
            size,
            source,
            base,
            offset,
        } => encode_load_store(size, false, source.encoding(), base.encoding(), offset),
        _ => {
            unreachable!("instruction category is closed by encode")
        }
    }
}

fn encode_control(instruction: Arm64Instruction) -> Result<u32, Arm64EncodingError> {
    match instruction {
        Arm64Instruction::ConditionalSet {
            size,
            destination,
            condition,
        } => Ok(encode_conditional_set(
            size,
            destination.number(),
            condition,
        )),
        Arm64Instruction::Branch { displacement, link } => {
            let immediate = signed_scaled(displacement, 26)?;
            Ok(if link { 0x9400_0000 } else { 0x1400_0000 } | immediate)
        }
        Arm64Instruction::BranchConditional {
            displacement,
            condition,
        } => {
            let immediate = signed_scaled(displacement, 19)?;
            Ok(0x5400_0000 | immediate << 5 | u32::from(condition.encoding()))
        }
        Arm64Instruction::BranchRegister { target, link } => {
            Ok(if link { 0xd63f_0000 } else { 0xd61f_0000 } | u32::from(target.number()) << 5)
        }
        Arm64Instruction::Return { target } => Ok(0xd65f_0000 | u32::from(target.number()) << 5),
        Arm64Instruction::Break { immediate } => Ok(0xd420_0000 | u32::from(immediate) << 5),
        Arm64Instruction::SupervisorCall { immediate } => {
            Ok(0xd400_0001 | u32::from(immediate) << 5)
        }
        _ => {
            unreachable!("instruction category is closed by encode")
        }
    }
}

fn encode_add_subtract_immediate(
    size: Arm64DataSize,
    operation: Arm64AddSubtract,
    set_flags: bool,
    destination: Arm64AddSubtractDestination,
    source: u32,
    immediate: u16,
    shift_12: bool,
) -> Result<u32, Arm64EncodingError> {
    validate_add_subtract_destination(destination, set_flags)?;
    if immediate > 0x0fff {
        return Err(Arm64EncodingError::ImmediateOutOfRange);
    }
    Ok(size_bit(size)
        | add_subtract_bits(operation, set_flags)
        | 0x1100_0000
        | u32::from(shift_12) << 22
        | u32::from(immediate) << 10
        | source << 5
        | destination.encoding())
}

const fn encode_add_subtract_register(
    size: Arm64DataSize,
    operation: Arm64AddSubtract,
    set_flags: bool,
    destination: u32,
    left: u32,
    right: u32,
) -> u32 {
    size_bit(size)
        | add_subtract_bits(operation, set_flags)
        | 0x0b00_0000
        | right << 16
        | left << 5
        | destination
}

const fn encode_logical_register(
    size: Arm64DataSize,
    operation: Arm64Logical,
    destination: u32,
    left: u32,
    right: u32,
) -> u32 {
    size_bit(size) | logical_bits(operation) | 0x0a00_0000 | right << 16 | left << 5 | destination
}

fn encode_move_wide(
    size: Arm64DataSize,
    operation: Arm64MoveWide,
    destination: u8,
    immediate: u16,
    shift: u8,
) -> Result<u32, Arm64EncodingError> {
    let halfword = move_wide_halfword(size, shift)?;
    let operation = match operation {
        Arm64MoveWide::Zero => 0x5280_0000,
        Arm64MoveWide::Keep => 0x7280_0000,
    };
    Ok(size_bit(size)
        | operation
        | u32::from(halfword) << 21
        | u32::from(immediate) << 5
        | u32::from(destination))
}

const fn encode_multiply_add(
    size: Arm64DataSize,
    destination: u8,
    left: u8,
    right: u8,
    addend: u32,
    subtract_product: bool,
) -> u32 {
    size_bit(size)
        | 0x1b00_0000
        | (right as u32) << 16
        | (subtract_product as u32) << 15
        | addend << 10
        | (left as u32) << 5
        | destination as u32
}

const fn encode_divide(
    size: Arm64DataSize,
    destination: u8,
    left: u8,
    right: u8,
    signed: bool,
) -> u32 {
    size_bit(size)
        | if signed { 0x1ac0_0c00 } else { 0x1ac0_0800 }
        | (right as u32) << 16
        | (left as u32) << 5
        | destination as u32
}

const fn encode_variable_shift(
    size: Arm64DataSize,
    operation: Arm64Shift,
    destination: u8,
    value: u8,
    amount: u8,
) -> u32 {
    size_bit(size)
        | variable_shift_bits(operation)
        | (amount as u32) << 16
        | (value as u32) << 5
        | destination as u32
}

const fn encode_conditional_set(
    size: Arm64DataSize,
    destination: u8,
    condition: Arm64BranchCondition,
) -> u32 {
    size_bit(size)
        | 0x1a80_0400
        | 31 << 16
        | (condition.invert().encoding() as u32) << 12
        | 31 << 5
        | destination as u32
}

const fn size_bit(size: Arm64DataSize) -> u32 {
    match size {
        Arm64DataSize::Bits32 => 0,
        Arm64DataSize::Bits64 => 1 << 31,
    }
}

const fn add_subtract_bits(operation: Arm64AddSubtract, set_flags: bool) -> u32 {
    let operation = match operation {
        Arm64AddSubtract::Add => 0,
        Arm64AddSubtract::Subtract => 1 << 30,
    };
    operation | if set_flags { 1 << 29 } else { 0 }
}

const fn logical_bits(operation: Arm64Logical) -> u32 {
    match operation {
        Arm64Logical::And => 0,
        Arm64Logical::Or => 1 << 29,
        Arm64Logical::ExclusiveOr => 2 << 29,
        Arm64Logical::AndSetFlags => 3 << 29,
    }
}

const fn variable_shift_bits(operation: Arm64Shift) -> u32 {
    match operation {
        Arm64Shift::Left => 0x1ac0_2000,
        Arm64Shift::RightLogical => 0x1ac0_2400,
        Arm64Shift::RightArithmetic => 0x1ac0_2800,
    }
}

const fn validate_add_subtract_destination(
    destination: Arm64AddSubtractDestination,
    set_flags: bool,
) -> Result<(), Arm64EncodingError> {
    match (destination, set_flags) {
        (Arm64AddSubtractDestination::StackPointer, true)
        | (Arm64AddSubtractDestination::Zero, false) => {
            Err(Arm64EncodingError::InvalidRegisterRole)
        }
        (Arm64AddSubtractDestination::General(_), _)
        | (Arm64AddSubtractDestination::StackPointer, false)
        | (Arm64AddSubtractDestination::Zero, true) => Ok(()),
    }
}

const fn move_wide_halfword(size: Arm64DataSize, shift: u8) -> Result<u8, Arm64EncodingError> {
    if !shift.is_multiple_of(16) {
        return Err(Arm64EncodingError::InvalidShift);
    }
    let halfword = shift / 16;
    match size {
        Arm64DataSize::Bits32 if halfword < 2 => Ok(halfword),
        Arm64DataSize::Bits64 if halfword < 4 => Ok(halfword),
        Arm64DataSize::Bits32 | Arm64DataSize::Bits64 => Err(Arm64EncodingError::InvalidShift),
    }
}

fn encode_load_store(
    size: Arm64LoadStoreSize,
    load: bool,
    data: u32,
    base: u32,
    offset: u32,
) -> Result<u32, Arm64EncodingError> {
    let (bytes, instruction) = match (size, load) {
        (Arm64LoadStoreSize::Byte, false) => (1, 0x3900_0000),
        (Arm64LoadStoreSize::Byte, true) => (1, 0x3940_0000),
        (Arm64LoadStoreSize::Half, false) => (2, 0x7900_0000),
        (Arm64LoadStoreSize::Half, true) => (2, 0x7940_0000),
        (Arm64LoadStoreSize::Word, false) => (4, 0xb900_0000),
        (Arm64LoadStoreSize::Word, true) => (4, 0xb940_0000),
        (Arm64LoadStoreSize::Double, false) => (8, 0xf900_0000),
        (Arm64LoadStoreSize::Double, true) => (8, 0xf940_0000),
    };
    if !offset.is_multiple_of(bytes) || offset / bytes > 0x0fff {
        return Err(Arm64EncodingError::OffsetOutOfRange);
    }
    Ok(instruction | (offset / bytes) << 10 | base << 5 | data)
}

fn signed_scaled(displacement: i64, bits: u8) -> Result<u32, Arm64EncodingError> {
    if displacement % 4 != 0 {
        return Err(Arm64EncodingError::MisalignedBranch);
    }
    let words = displacement / 4;
    let minimum = -(1_i64 << (bits - 1));
    let maximum = (1_i64 << (bits - 1)) - 1;
    if words < minimum || words > maximum {
        return Err(Arm64EncodingError::BranchOutOfRange);
    }
    let encoded = if words < 0 {
        (1_i64 << bits) + words
    } else {
        words
    };
    u32::try_from(encoded).map_err(|_| Arm64EncodingError::BranchOutOfRange)
}

impl Arm64BranchCondition {
    const fn encoding(self) -> u8 {
        match self {
            Self::Equal => 0,
            Self::NotEqual => 1,
            Self::CarrySet => 2,
            Self::CarryClear => 3,
            Self::Minus => 4,
            Self::Plus => 5,
            Self::Overflow => 6,
            Self::NoOverflow => 7,
            Self::UnsignedHigher => 8,
            Self::UnsignedLowerOrSame => 9,
            Self::SignedGreaterOrEqual => 10,
            Self::SignedLess => 11,
            Self::SignedGreater => 12,
            Self::SignedLessOrEqual => 13,
        }
    }

    const fn invert(self) -> Self {
        match self {
            Self::Equal => Self::NotEqual,
            Self::NotEqual => Self::Equal,
            Self::CarrySet => Self::CarryClear,
            Self::CarryClear => Self::CarrySet,
            Self::Minus => Self::Plus,
            Self::Plus => Self::Minus,
            Self::Overflow => Self::NoOverflow,
            Self::NoOverflow => Self::Overflow,
            Self::UnsignedHigher => Self::UnsignedLowerOrSame,
            Self::UnsignedLowerOrSame => Self::UnsignedHigher,
            Self::SignedGreaterOrEqual => Self::SignedLess,
            Self::SignedLess => Self::SignedGreaterOrEqual,
            Self::SignedGreater => Self::SignedLessOrEqual,
            Self::SignedLessOrEqual => Self::SignedGreater,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64EncodingError {
    InvalidRegisterRole,
    ImmediateOutOfRange,
    InvalidShift,
    OffsetOutOfRange,
    MisalignedBranch,
    BranchOutOfRange,
}
