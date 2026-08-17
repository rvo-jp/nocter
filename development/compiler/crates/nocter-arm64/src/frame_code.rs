use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64CodeBuilder,
    Arm64DataRegister, Arm64DataSize, Arm64FrameLayout, Arm64Instruction, Arm64LoadStoreSize,
    Arm64MoveWide, Arm64NocterAbi, Arm64Register,
};

const MAX_SCALED_DOUBLE_OFFSET: u64 = 0x0fff * Arm64NocterAbi::WORD_SIZE;

/// Materializes one validated fixed-frame layout without imposing an immediate-size frame limit.
pub struct Arm64FrameCode;

impl Arm64FrameCode {
    pub fn emit_prologue(frame: &Arm64FrameLayout, code: &mut Arm64CodeBuilder) {
        adjust_stack(code, frame.size(), Arm64AddSubtract::Subtract);
        store_at_stack_offset(code, frame_pointer(), frame.frame_record_offset());
        store_at_stack_offset(code, link_register(), frame.frame_record_offset() + 8);
        for saved in frame.saved_registers() {
            store_at_stack_offset(code, saved.register(), saved.offset());
        }
        form_stack_address(code, frame_pointer(), frame.frame_record_offset());
    }

    pub fn emit_epilogue(frame: &Arm64FrameLayout, code: &mut Arm64CodeBuilder) {
        for saved in frame.saved_registers() {
            load_at_stack_offset(code, saved.register(), saved.offset());
        }
        load_at_stack_offset(code, link_register(), frame.frame_record_offset() + 8);
        load_at_stack_offset(code, frame_pointer(), frame.frame_record_offset());
        adjust_stack(code, frame.size(), Arm64AddSubtract::Add);
        code.append(Arm64Instruction::Return {
            target: link_register(),
        });
    }
}

fn adjust_stack(code: &mut Arm64CodeBuilder, amount: u64, operation: Arm64AddSubtract) {
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

    let scratch = scratch();
    load_immediate(code, scratch, amount);
    code.append(Arm64Instruction::AddSubtractExtendedRegister {
        operation,
        set_flags: false,
        destination: Arm64AddSubtractDestination::StackPointer,
        left: Arm64BaseRegister::StackPointer,
        right: scratch,
        shift: 0,
    });
}

fn form_stack_address(code: &mut Arm64CodeBuilder, destination: Arm64Register, offset: u64) {
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

    let scratch = scratch();
    load_immediate(code, scratch, offset);
    code.append(Arm64Instruction::AddSubtractExtendedRegister {
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        left: Arm64BaseRegister::StackPointer,
        right: scratch,
        shift: 0,
    });
}

fn store_at_stack_offset(code: &mut Arm64CodeBuilder, source: Arm64Register, offset: u64) {
    let (base, offset) = memory_base(code, offset);
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::General(source),
        base,
        offset,
    });
}

fn load_at_stack_offset(code: &mut Arm64CodeBuilder, destination: Arm64Register, offset: u64) {
    let (base, offset) = memory_base(code, offset);
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base,
        offset,
    });
}

fn memory_base(code: &mut Arm64CodeBuilder, offset: u64) -> (Arm64BaseRegister, u32) {
    if offset <= MAX_SCALED_DOUBLE_OFFSET && offset.is_multiple_of(Arm64NocterAbi::WORD_SIZE) {
        return (
            Arm64BaseRegister::StackPointer,
            u32::try_from(offset).expect("scaled double offset is bounded"),
        );
    }

    let scratch = scratch();
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

fn load_immediate(code: &mut Arm64CodeBuilder, destination: Arm64Register, value: u64) {
    let mut wrote_zero = false;
    for shift in [0_u8, 16, 32, 48] {
        let part = u16::try_from((value >> shift) & 0xffff).expect("masked part fits u16");
        if part == 0 {
            continue;
        }
        code.append(Arm64Instruction::MoveWide {
            size: Arm64DataSize::Bits64,
            operation: if wrote_zero {
                Arm64MoveWide::Keep
            } else {
                Arm64MoveWide::Zero
            },
            destination,
            immediate: part,
            shift,
        });
        wrote_zero = true;
    }
    if !wrote_zero {
        code.append(Arm64Instruction::MoveWide {
            size: Arm64DataSize::Bits64,
            operation: Arm64MoveWide::Zero,
            destination,
            immediate: 0,
            shift: 0,
        });
    }
}

fn scratch() -> Arm64Register {
    Arm64NocterAbi::compiler_scratch_register(0).expect("the ABI reserves x16")
}

fn frame_pointer() -> Arm64Register {
    Arm64NocterAbi::frame_pointer_register()
}

fn link_register() -> Arm64Register {
    Arm64NocterAbi::link_register()
}
