//! Shared native mechanics for compiler-owned computation compositions.

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64NocterAbi,
};
use nocter_runtime_contract::RuntimeAsyncStateTags;

pub(super) const FRAME_STACK_OFFSET: u64 = 0;
pub(super) const LINK_STACK_OFFSET: u64 = 8;
pub(super) const ALLOCATION_CONTEXT_STACK_OFFSET: u64 = 16;
pub(super) const PROCESS_CONTEXT_STACK_OFFSET: u64 = 24;
pub(super) const CALL_STACK_SIZE: u64 = 32;

pub(super) const OUTPUT_STACK_OFFSET: u64 = 8;
pub(super) const CONSUME_LINK_STACK_OFFSET: u64 = 16;
pub(super) const CONSUME_ALLOCATION_CONTEXT_STACK_OFFSET: u64 = 24;
pub(super) const CONSUME_PROCESS_CONTEXT_STACK_OFFSET: u64 = 32;
pub(super) const CONSUME_STACK_SIZE: u64 = 48;

pub(super) fn lifecycle_states() -> Result<RuntimeAsyncStateTags, crate::Arm64CodeError> {
    Arm64NocterAbi::asynchronous()
        .state_tags(0)
        .ok_or(crate::Arm64CodeError::AsyncStateTagExhausted)
}

pub(super) fn emit_call_prologue(frame: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    crate::frame_access::adjust_stack(code, CALL_STACK_SIZE, Arm64AddSubtract::Subtract);
    store_stack(FRAME_STACK_OFFSET, frame, code);
    store_stack(LINK_STACK_OFFSET, Arm64NocterAbi::link_register(), code);
    store_stack(
        ALLOCATION_CONTEXT_STACK_OFFSET,
        Arm64NocterAbi::allocation_context_register(),
        code,
    );
    store_stack(
        PROCESS_CONTEXT_STACK_OFFSET,
        Arm64NocterAbi::process_context_register(),
        code,
    );
}

pub(super) fn emit_call_epilogue(code: &mut Arm64CodeBuilder) {
    load_stack(
        ALLOCATION_CONTEXT_STACK_OFFSET,
        Arm64NocterAbi::allocation_context_register(),
        code,
    );
    load_stack(
        PROCESS_CONTEXT_STACK_OFFSET,
        Arm64NocterAbi::process_context_register(),
        code,
    );
    load_stack(LINK_STACK_OFFSET, Arm64NocterAbi::link_register(), code);
    crate::frame_access::adjust_stack(code, CALL_STACK_SIZE, Arm64AddSubtract::Add);
    return_to_caller(code);
}

pub(super) fn validate_state(expected: u64, code: &mut Arm64CodeBuilder) {
    load_frame(argument(3), code);
    load(
        argument(3),
        Arm64NocterAbi::asynchronous().state_tag_offset(),
        argument(4),
        code,
    );
    compare_immediate(argument(4), expected, code);
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::Equal);
    trap_state(code);
    code.bind(valid)
        .expect("fresh composition state-validation label binds once");
}

/// Continues an initial composition poll or repeats its terminal observation without rerunning it.
pub(super) fn accept_initial_or_return_completed(
    initial: u64,
    completed: u64,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    load_frame(argument(3), code);
    load(
        argument(3),
        Arm64NocterAbi::asynchronous().state_tag_offset(),
        argument(4),
        code,
    );
    let proceed = code.create_label();
    let return_completed = code.create_label();
    compare_immediate(argument(4), initial, code);
    code.branch_conditional(proceed, Arm64BranchCondition::Equal);
    compare_immediate(argument(4), completed, code);
    code.branch_conditional(return_completed, Arm64BranchCondition::Equal);
    trap_state(code);
    code.bind(return_completed)?;
    let schema = Arm64NocterAbi::asynchronous();
    crate::frame_access::load_immediate(
        code,
        argument(0),
        schema.completed_status(),
        Arm64DataSize::Bits64,
    );
    zero(argument(1), code);
    zero(argument(2), code);
    emit_call_epilogue(code);
    code.bind(proceed)
}

pub(super) fn validate_state_any(
    expected: &[u64],
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    load_frame(argument(3), code);
    load(
        argument(3),
        Arm64NocterAbi::asynchronous().state_tag_offset(),
        argument(4),
        code,
    );
    let valid = code.create_label();
    for state in expected {
        compare_immediate(argument(4), *state, code);
        code.branch_conditional(valid, Arm64BranchCondition::Equal);
    }
    trap_state(code);
    code.bind(valid)
}

pub(super) fn validate_nonzero(value: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    validate_nonzero_with_trap(
        value,
        crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameStateCorruption,
        code,
    );
}

pub(super) fn validate_nonzero_with_trap(
    value: crate::Arm64Register,
    trap: crate::runtime_trap::Arm64RuntimeTrap,
    code: &mut Arm64CodeBuilder,
) {
    compare_immediate(value, 0, code);
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::NotEqual);
    code.append(Arm64Instruction::Break {
        immediate: trap.immediate(),
    });
    code.bind(valid)
        .expect("fresh composition pointer-validation label binds once");
}

pub(super) fn load_frame(destination: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    load_stack(FRAME_STACK_OFFSET, destination, code);
}

pub(super) fn initialize_entry(
    frame: crate::Arm64Register,
    offset: u64,
    target: crate::Arm64FunctionId,
    code: &mut Arm64CodeBuilder,
) {
    code.load_function_address(target, argument(4));
    store(frame, offset, argument(4), code);
}

pub(super) fn initialize_from_stack(
    frame: crate::Arm64Register,
    frame_offset: u64,
    stack_offset: u64,
    code: &mut Arm64CodeBuilder,
) {
    load_stack(stack_offset, argument(4), code);
    store(frame, frame_offset, argument(4), code);
}

pub(super) fn load(
    base: crate::Arm64Register,
    offset: u64,
    destination: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        destination,
        base,
        offset,
    );
}

pub(super) fn store(
    base: crate::Arm64Register,
    offset: u64,
    source: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    crate::address_code::store_native(code, Arm64LoadStoreSize::Double, source, base, offset);
}

pub(super) fn store_immediate(
    base: crate::Arm64Register,
    offset: u64,
    value: u64,
    code: &mut Arm64CodeBuilder,
) {
    crate::frame_access::load_immediate(code, argument(7), value, Arm64DataSize::Bits64);
    store(base, offset, argument(7), code);
}

pub(super) fn store_stack(offset: u64, source: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    crate::frame_access::store_at_stack_offset(code, Arm64LoadStoreSize::Double, source, offset);
}

pub(super) fn load_stack(
    offset: u64,
    destination: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    crate::frame_access::load_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        destination,
        offset,
    );
}

pub(super) fn compare_immediate(
    value: crate::Arm64Register,
    expected: u64,
    code: &mut Arm64CodeBuilder,
) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: u16::try_from(expected).expect("composition tags fit an immediate"),
        shift_12: false,
    });
}

pub(super) fn compare_register(
    left: crate::Arm64Register,
    right: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(left),
        right: Arm64DataRegister::General(right),
    });
}

pub(super) fn add_register(
    destination: crate::Arm64Register,
    left: crate::Arm64Register,
    right: crate::Arm64Register,
    set_flags: bool,
    code: &mut Arm64CodeBuilder,
) {
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags,
        destination: Arm64DataRegister::General(destination),
        left: Arm64DataRegister::General(left),
        right: Arm64DataRegister::General(right),
    });
}

pub(super) fn add_immediate(
    value: crate::Arm64Register,
    immediate: u64,
    code: &mut Arm64CodeBuilder,
) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(value),
        source: Arm64BaseRegister::General(value),
        immediate: u16::try_from(immediate).expect("composition stride fits an immediate"),
        shift_12: false,
    });
}

pub(super) fn subtract_immediate(
    value: crate::Arm64Register,
    immediate: u64,
    code: &mut Arm64CodeBuilder,
) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(value),
        source: Arm64BaseRegister::General(value),
        immediate: u16::try_from(immediate).expect("composition decrement fits an immediate"),
        shift_12: false,
    });
}

pub(super) fn zero(register: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    crate::frame_access::load_immediate(code, register, 0, Arm64DataSize::Bits64);
}

pub(super) fn trap_state(code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameStateCorruption.immediate(),
    });
}

pub(super) fn trap_wait(code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitRecordCorruption.immediate(),
    });
}

pub(super) fn return_to_caller(code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::Return {
        target: Arm64NocterAbi::link_register(),
    });
}

pub(super) const fn argument(index: u8) -> crate::Arm64Register {
    match Arm64NocterAbi::argument_register(index) {
        Some(register) => register,
        None => panic!("computation composition uses only ABI argument registers"),
    }
}
