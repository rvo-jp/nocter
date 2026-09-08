use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64NocterAbi,
    Arm64SystemRegister,
};

const MAX_TIMEOUT_MILLISECONDS: u64 = i32::MAX as u64;
const MILLISECONDS_PER_SECOND: u64 = 1_000;

/// Converts one absolute ARM counter deadline to Darwin's relative millisecond timeout.
/// Conversion rounds upward, so it cannot make the process root resume a timer early.
pub(crate) fn emit(
    deadline: crate::Arm64Register,
    output: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let no_deadline = code.create_label();
    let expired = code.create_label();
    let cap = code.create_label();
    let round = code.create_label();
    let complete = code.create_label();
    compare_immediate(deadline, u64::MAX, code);
    code.branch_conditional(no_deadline, Arm64BranchCondition::Equal);
    code.append(Arm64Instruction::InstructionSynchronizationBarrier);
    code.append(Arm64Instruction::ReadSystemRegister {
        destination: argument(4),
        register: Arm64SystemRegister::CounterVirtual,
    });
    compare(argument(4), deadline, code);
    code.branch_conditional(expired, Arm64BranchCondition::CarrySet);
    code.append(Arm64Instruction::ReadSystemRegister {
        destination: argument(5),
        register: Arm64SystemRegister::CounterFrequency,
    });
    require_valid_frequency(code)?;
    emit_finite(deadline, output, cap, round, complete, code);

    code.bind(round)?;
    add_immediate(output, 1, code);
    compare_immediate(output, MAX_TIMEOUT_MILLISECONDS, code);
    code.branch_conditional(cap, Arm64BranchCondition::UnsignedHigher);
    code.branch(complete, false);
    code.bind(no_deadline)?;
    crate::frame_access::load_immediate(code, output, u64::MAX, Arm64DataSize::Bits64);
    code.branch(complete, false);
    code.bind(expired)?;
    crate::frame_access::load_immediate(code, output, 0, Arm64DataSize::Bits64);
    code.branch(complete, false);
    code.bind(cap)?;
    crate::frame_access::load_immediate(
        code,
        output,
        MAX_TIMEOUT_MILLISECONDS,
        Arm64DataSize::Bits64,
    );
    code.bind(complete)
}

fn require_valid_frequency(code: &mut Arm64CodeBuilder) -> Result<(), crate::Arm64CodeError> {
    compare_small(argument(5), 0, code);
    let present = code.create_label();
    code.branch_conditional(present, Arm64BranchCondition::NotEqual);
    trap(code);
    code.bind(present)?;
    compare_immediate(argument(5), u64::MAX / MILLISECONDS_PER_SECOND, code);
    let safe = code.create_label();
    code.branch_conditional(safe, Arm64BranchCondition::UnsignedLowerOrSame);
    trap(code);
    code.bind(safe)
}

fn emit_finite(
    deadline: crate::Arm64Register,
    output: crate::Arm64Register,
    cap: crate::Arm64LabelId,
    round: crate::Arm64LabelId,
    complete: crate::Arm64LabelId,
    code: &mut Arm64CodeBuilder,
) {
    // x4 = remaining ticks; x6 = whole seconds; x7 = sub-second ticks.
    subtract(argument(4), deadline, argument(4), code);
    code.append(Arm64Instruction::Divide {
        size: Arm64DataSize::Bits64,
        destination: argument(6),
        left: argument(4),
        right: argument(5),
        signed: false,
    });
    compare_immediate(
        argument(6),
        MAX_TIMEOUT_MILLISECONDS / MILLISECONDS_PER_SECOND,
        code,
    );
    code.branch_conditional(cap, Arm64BranchCondition::UnsignedHigher);
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination: argument(7),
        left: argument(6),
        right: argument(5),
        addend: Arm64DataRegister::General(argument(4)),
        subtract_product: true,
    });
    crate::frame_access::load_immediate(
        code,
        scratch(0),
        MILLISECONDS_PER_SECOND,
        Arm64DataSize::Bits64,
    );
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination: output,
        left: argument(6),
        right: scratch(0),
        addend: Arm64DataRegister::Zero,
        subtract_product: false,
    });
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination: argument(7),
        left: argument(7),
        right: scratch(0),
        addend: Arm64DataRegister::Zero,
        subtract_product: false,
    });
    code.append(Arm64Instruction::Divide {
        size: Arm64DataSize::Bits64,
        destination: scratch(0),
        left: argument(7),
        right: argument(5),
        signed: false,
    });
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination: scratch(1),
        left: scratch(0),
        right: argument(5),
        addend: Arm64DataRegister::General(argument(7)),
        subtract_product: true,
    });
    add(output, scratch(0), code);
    compare_small(scratch(1), 0, code);
    code.branch_conditional(round, Arm64BranchCondition::NotEqual);
    compare_immediate(output, MAX_TIMEOUT_MILLISECONDS, code);
    code.branch_conditional(cap, Arm64BranchCondition::UnsignedHigher);
    code.branch(complete, false);
}

fn compare_small(value: crate::Arm64Register, expected: u16, code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: expected,
        shift_12: false,
    });
}

fn compare_immediate(value: crate::Arm64Register, expected: u64, code: &mut Arm64CodeBuilder) {
    let temporary = if value == scratch(0) {
        scratch(1)
    } else {
        scratch(0)
    };
    crate::frame_access::load_immediate(code, temporary, expected, Arm64DataSize::Bits64);
    compare(value, temporary, code);
}

fn compare(left: crate::Arm64Register, right: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(left),
        right: Arm64DataRegister::General(right),
    });
}

fn add_immediate(value: crate::Arm64Register, immediate: u16, code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(value),
        source: Arm64BaseRegister::General(value),
        immediate,
        shift_12: false,
    });
}

fn add(
    destination: crate::Arm64Register,
    right: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64DataRegister::General(destination),
        left: Arm64DataRegister::General(destination),
        right: Arm64DataRegister::General(right),
    });
}

fn subtract(
    destination: crate::Arm64Register,
    left: crate::Arm64Register,
    right: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64DataRegister::General(destination),
        left: Arm64DataRegister::General(left),
        right: Arm64DataRegister::General(right),
    });
}

fn trap(code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitFailure.immediate(),
    });
}

const fn argument(index: u8) -> crate::Arm64Register {
    match Arm64NocterAbi::argument_register(index) {
        Some(register) => register,
        None => panic!("async timeout conversion uses only ABI argument registers"),
    }
}

const fn scratch(index: u8) -> crate::Arm64Register {
    match Arm64NocterAbi::compiler_scratch_register(index) {
        Some(register) => register,
        None => panic!("async timeout conversion uses only reserved scratch registers"),
    }
}
