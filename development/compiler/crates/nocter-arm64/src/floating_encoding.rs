use crate::{Arm64DataSize, Arm64EncodingError, Arm64FloatBinary, Arm64Instruction};

pub(crate) fn arithmetic(instruction: Arm64Instruction) -> u32 {
    match instruction {
        Arm64Instruction::FloatMove {
            size,
            destination,
            source,
        } => {
            size_base(size, 0x1e20_4000, 0x1e60_4000)
                | u32::from(source.number()) << 5
                | u32::from(destination.number())
        }
        Arm64Instruction::FloatMoveFromGeneral {
            size,
            destination,
            source,
        } => {
            size_base(size, 0x1e27_0000, 0x9e67_0000)
                | u32::from(source.number()) << 5
                | u32::from(destination.number())
        }
        Arm64Instruction::FloatNegate {
            size,
            destination,
            source,
        } => {
            size_base(size, 0x1e21_4000, 0x1e61_4000)
                | u32::from(source.number()) << 5
                | u32::from(destination.number())
        }
        Arm64Instruction::FloatBinary {
            size,
            operation,
            destination,
            left,
            right,
        } => {
            let (single, double) = match operation {
                Arm64FloatBinary::Add => (0x1e20_2800, 0x1e60_2800),
                Arm64FloatBinary::Subtract => (0x1e20_3800, 0x1e60_3800),
                Arm64FloatBinary::Multiply => (0x1e20_0800, 0x1e60_0800),
                Arm64FloatBinary::Divide => (0x1e20_1800, 0x1e60_1800),
            };
            size_base(size, single, double)
                | u32::from(right.number()) << 16
                | u32::from(left.number()) << 5
                | u32::from(destination.number())
        }
        Arm64Instruction::FloatCompare { size, left, right } => {
            size_base(size, 0x1e20_2000, 0x1e60_2000)
                | u32::from(right.number()) << 16
                | u32::from(left.number()) << 5
        }
        _ => unreachable!("floating arithmetic encoding received another instruction class"),
    }
}

pub(crate) fn memory(instruction: Arm64Instruction) -> Result<u32, Arm64EncodingError> {
    let (size, load, data, base, offset) = match instruction {
        Arm64Instruction::FloatLoad {
            size,
            destination,
            base,
            offset,
        } => (size, true, destination.number(), base.encoding(), offset),
        Arm64Instruction::FloatStore {
            size,
            source,
            base,
            offset,
        } => (size, false, source.number(), base.encoding(), offset),
        _ => unreachable!("floating memory encoding received another instruction class"),
    };
    let (bytes, base_word) = match (size, load) {
        (Arm64DataSize::Bits32, false) => (4, 0xbd00_0000),
        (Arm64DataSize::Bits32, true) => (4, 0xbd40_0000),
        (Arm64DataSize::Bits64, false) => (8, 0xfd00_0000),
        (Arm64DataSize::Bits64, true) => (8, 0xfd40_0000),
    };
    if !offset.is_multiple_of(bytes) || offset / bytes > 0x0fff {
        return Err(Arm64EncodingError::OffsetOutOfRange);
    }
    Ok(base_word | (offset / bytes) << 10 | base << 5 | u32::from(data))
}

const fn size_base(size: Arm64DataSize, single: u32, double: u32) -> u32 {
    match size {
        Arm64DataSize::Bits32 => single,
        Arm64DataSize::Bits64 => double,
    }
}
