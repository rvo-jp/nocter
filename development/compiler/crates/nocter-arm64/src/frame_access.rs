use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64CodeBuilder,
    Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize, Arm64MoveWide,
    Arm64NocterAbi, Arm64Register,
};

pub(crate) fn adjust_stack(code: &mut Arm64CodeBuilder, amount: u64, operation: Arm64AddSubtract) {
    if let Some((immediate, shift_12)) = add_immediate(amount) {
        code.append(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation,
            set_flags: false,
            destination: Arm64AddSubtractDestination::StackPointer,
            source: Arm64BaseRegister::StackPointer,
            immediate,
            shift_12,
        });
        return;
    }
    let scratch = scratch(0);
    load_immediate(code, scratch, amount, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::AddSubtractExtendedRegister {
        operation,
        set_flags: false,
        destination: Arm64AddSubtractDestination::StackPointer,
        left: Arm64BaseRegister::StackPointer,
        right: scratch,
        shift: 0,
    });
}

pub(crate) fn form_stack_address(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    offset: u64,
) {
    if let Some((immediate, shift_12)) = add_immediate(offset) {
        code.append(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Add,
            set_flags: false,
            destination: Arm64AddSubtractDestination::General(destination),
            source: Arm64BaseRegister::StackPointer,
            immediate,
            shift_12,
        });
        return;
    }
    let offset_register = scratch(0);
    load_immediate(code, offset_register, offset, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::AddSubtractExtendedRegister {
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        left: Arm64BaseRegister::StackPointer,
        right: offset_register,
        shift: 0,
    });
}

pub(crate) fn store_at_stack_offset(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    source: Arm64Register,
    offset: u64,
) {
    let address_scratch = if source == scratch(0) {
        scratch(1)
    } else {
        scratch(0)
    };
    let (base, offset) = memory_base(code, size, offset, address_scratch);
    code.append(Arm64Instruction::StoreUnsigned {
        size,
        source: Arm64DataRegister::General(source),
        base,
        offset,
    });
}

pub(crate) fn load_at_stack_offset(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    destination: Arm64Register,
    offset: u64,
) {
    let (base, offset) = memory_base(code, size, offset, destination);
    code.append(Arm64Instruction::LoadUnsigned {
        size,
        destination: Arm64DataRegister::General(destination),
        base,
        offset,
    });
}

fn memory_base(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    offset: u64,
    scratch: Arm64Register,
) -> (Arm64BaseRegister, u32) {
    let scale = load_store_bytes(size);
    if offset <= 0x0fff * scale && offset.is_multiple_of(scale) {
        return (
            Arm64BaseRegister::StackPointer,
            u32::try_from(offset).expect("scaled offset is bounded"),
        );
    }
    form_stack_address(code, scratch, offset);
    (Arm64BaseRegister::General(scratch), 0)
}

fn add_immediate(value: u64) -> Option<(u16, bool)> {
    if value <= 0x0fff {
        return Some((u16::try_from(value).ok()?, false));
    }
    if value.is_multiple_of(1 << 12) && value >> 12 <= 0x0fff {
        return Some((u16::try_from(value >> 12).ok()?, true));
    }
    None
}

pub(crate) fn load_immediate(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    value: u64,
    size: Arm64DataSize,
) {
    let shifts: &[u8] = match size {
        Arm64DataSize::Bits32 => &[0, 16],
        Arm64DataSize::Bits64 => &[0, 16, 32, 48],
    };
    let mut wrote_zero = false;
    for shift in shifts {
        let part = u16::try_from((value >> shift) & 0xffff).expect("masked part fits u16");
        if part == 0 {
            continue;
        }
        code.append(Arm64Instruction::MoveWide {
            size,
            operation: if wrote_zero {
                Arm64MoveWide::Keep
            } else {
                Arm64MoveWide::Zero
            },
            destination,
            immediate: part,
            shift: *shift,
        });
        wrote_zero = true;
    }
    if !wrote_zero {
        code.append(Arm64Instruction::MoveWide {
            size,
            operation: Arm64MoveWide::Zero,
            destination,
            immediate: 0,
            shift: 0,
        });
    }
}

pub(crate) const fn scratch(index: u8) -> Arm64Register {
    match Arm64NocterAbi::compiler_scratch_register(index) {
        Some(register) => register,
        None => panic!("the ABI reserves exactly two compiler scratch registers"),
    }
}

const fn load_store_bytes(size: Arm64LoadStoreSize) -> u64 {
    match size {
        Arm64LoadStoreSize::Byte => 1,
        Arm64LoadStoreSize::Half => 2,
        Arm64LoadStoreSize::Word => 4,
        Arm64LoadStoreSize::Double => 8,
    }
}
