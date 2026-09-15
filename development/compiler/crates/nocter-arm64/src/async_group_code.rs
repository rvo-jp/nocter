//! Borrowed runtime-sized child-set readiness lifecycle.
//!
//! The frame observes a mutable slice of uniform future handles. It never takes ownership of a
//! child: completion returns an index, cancellation merely ends the borrow, and ordinary source
//! destruction remains the sole authority that cancels children retained by `TaskGroup`.

use crate::{
    Arm64AddSubtract, Arm64AsyncGroupTargets, Arm64BranchCondition, Arm64Code, Arm64CodeBuilder,
    Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize, Arm64NocterAbi,
};

use crate::async_composition_code::{
    CONSUME_ALLOCATION_CONTEXT_STACK_OFFSET, CONSUME_LINK_STACK_OFFSET,
    CONSUME_PROCESS_CONTEXT_STACK_OFFSET, CONSUME_STACK_SIZE, FRAME_STACK_OFFSET,
    OUTPUT_STACK_OFFSET, accept_initial_or_return_completed, add_immediate, add_register, argument,
    compare_immediate, compare_register, emit_call_epilogue, emit_call_prologue, initialize_entry,
    initialize_from_stack, lifecycle_states, load, load_frame, load_stack, return_to_caller, store,
    store_immediate, store_stack, subtract_immediate, trap_state, trap_wait, validate_nonzero,
    validate_nonzero_with_trap, validate_state, validate_state_any, zero,
};

const CHILD_POINTER_OFFSET: u64 = Arm64NocterAbi::asynchronous().fixed_header_size();
const CHILD_COUNT_OFFSET: u64 = CHILD_POINTER_OFFSET + 8;
const AVAILABLE_OUTPUT_OFFSET: u64 = CHILD_COUNT_OFFSET + 8;
const INDEX_OUTPUT_OFFSET: u64 = AVAILABLE_OUTPUT_OFFSET + 8;
const AVAILABLE_VALUE_OFFSET: u64 = INDEX_OUTPUT_OFFSET + 8;
const INDEX_VALUE_OFFSET: u64 = AVAILABLE_VALUE_OFFSET + 8;
const METADATA_POINTER_OFFSET: u64 = INDEX_VALUE_OFFSET + 8;
const METADATA_SIZE_OFFSET: u64 = METADATA_POINTER_OFFSET + 8;
const INTEREST_POINTER_OFFSET: u64 = METADATA_SIZE_OFFSET + 8;
const INTEREST_SIZE_OFFSET: u64 = INTEREST_POINTER_OFFSET + 8;
const INTEREST_COUNT_OFFSET: u64 = INTEREST_SIZE_OFFSET + 8;
const SCAN_INDEX_OFFSET: u64 = INTEREST_COUNT_OFFSET + 8;
pub(super) const GROUP_FRAME_SIZE: u64 = SCAN_INDEX_OFFSET + 8;

const METADATA_RECORD_SIZE: u64 = 16;
const MAX_DYNAMIC_COUNT: u64 = i32::MAX as u64;

pub(super) fn initialize_group_frame(
    frame: crate::Arm64Register,
    targets: Arm64AsyncGroupTargets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let schema = Arm64NocterAbi::asynchronous();
    let states = lifecycle_states()?;
    initialize_entry(
        frame,
        schema.resume_function_offset(),
        targets.resume(),
        code,
    );
    initialize_entry(
        frame,
        schema.cancel_function_offset(),
        targets.cancel(),
        code,
    );
    initialize_entry(
        frame,
        schema.consume_function_offset(),
        targets.consume(),
        code,
    );
    store_immediate(frame, schema.state_tag_offset(), states.initial(), code);
    initialize_from_stack(
        frame,
        CHILD_POINTER_OFFSET,
        crate::async_group_constructor_code::CHILD_POINTER_STACK_OFFSET,
        code,
    );
    initialize_from_stack(
        frame,
        CHILD_COUNT_OFFSET,
        crate::async_group_constructor_code::CHILD_COUNT_STACK_OFFSET,
        code,
    );
    initialize_from_stack(
        frame,
        AVAILABLE_OUTPUT_OFFSET,
        crate::async_group_constructor_code::AVAILABLE_OUTPUT_STACK_OFFSET,
        code,
    );
    initialize_from_stack(
        frame,
        INDEX_OUTPUT_OFFSET,
        crate::async_group_constructor_code::INDEX_OUTPUT_STACK_OFFSET,
        code,
    );
    initialize_from_stack(
        frame,
        schema.allocation_context_offset(),
        crate::async_group_constructor_code::ALLOCATION_CONTEXT_STACK_OFFSET,
        code,
    );
    for offset in [
        METADATA_POINTER_OFFSET,
        METADATA_SIZE_OFFSET,
        INTEREST_POINTER_OFFSET,
        INTEREST_SIZE_OFFSET,
        INTEREST_COUNT_OFFSET,
        SCAN_INDEX_OFFSET,
        AVAILABLE_VALUE_OFFSET,
        INDEX_VALUE_OFFSET,
    ] {
        store_immediate(frame, offset, 0, code);
    }
    Ok(())
}

/// Polls every child in source order and completes with the first ready index.
pub(crate) fn materialize_resume() -> Result<Arm64Code, crate::Arm64CodeError> {
    let states = lifecycle_states()?;
    let schema = Arm64NocterAbi::asynchronous();
    let mut code = Arm64CodeBuilder::new();
    emit_call_prologue(argument(0), &mut code);
    accept_initial_or_return_completed(states.initial(), states.completed(), &mut code)?;

    load_frame(argument(3), &mut code);
    load(argument(3), CHILD_COUNT_OFFSET, argument(4), &mut code);
    let has_children = code.create_label();
    compare_immediate(argument(4), 0, &mut code);
    code.branch_conditional(has_children, Arm64BranchCondition::NotEqual);
    complete(false, 0, states.completed(), &mut code);

    code.bind(has_children)?;
    validate_dynamic_count(argument(4), &mut code);
    ensure_metadata_buffer(&mut code)?;
    load_frame(argument(3), &mut code);
    store_immediate(argument(3), SCAN_INDEX_OFFSET, 0, &mut code);
    store_immediate(argument(3), INTEREST_COUNT_OFFSET, 0, &mut code);

    let scan = code.create_label();
    let all_pending = code.create_label();
    code.bind(scan)?;
    load_frame(argument(3), &mut code);
    load(argument(3), SCAN_INDEX_OFFSET, argument(5), &mut code);
    load(argument(3), CHILD_COUNT_OFFSET, argument(6), &mut code);
    compare_register(argument(5), argument(6), &mut code);
    code.branch_conditional(all_pending, Arm64BranchCondition::Equal);
    poll_child(&mut code)?;
    add_immediate(argument(5), 1, &mut code);
    load_frame(argument(3), &mut code);
    store(argument(3), SCAN_INDEX_OFFSET, argument(5), &mut code);
    code.branch(scan, false);

    code.bind(all_pending)?;
    ensure_interest_buffer(&mut code)?;
    copy_interests(&mut code)?;
    load_frame(argument(3), &mut code);
    load(argument(3), INTEREST_POINTER_OFFSET, argument(1), &mut code);
    load(argument(3), INTEREST_COUNT_OFFSET, argument(2), &mut code);
    crate::frame_access::load_immediate(
        &mut code,
        argument(0),
        schema.pending_status(),
        Arm64DataSize::Bits64,
    );
    emit_call_epilogue(&mut code);
    code.finish()
}

/// Ends the readiness borrow without touching any caller-owned child handle.
pub(crate) fn materialize_cancel() -> Result<Arm64Code, crate::Arm64CodeError> {
    let states = lifecycle_states()?;
    let mut code = Arm64CodeBuilder::new();
    emit_call_prologue(argument(0), &mut code);
    validate_state_any(&[states.initial(), states.completed()], &mut code)?;
    release_temporary_buffers(&mut code)?;
    release_group_frame(&mut code)?;
    emit_call_epilogue(&mut code);
    code.finish()
}

/// Copies `(available, index)` to its frozen tuple placements and releases only the wait frame.
pub(crate) fn materialize_consume() -> Result<Arm64Code, crate::Arm64CodeError> {
    let states = lifecycle_states()?;
    let mut code = Arm64CodeBuilder::new();
    crate::frame_access::adjust_stack(&mut code, CONSUME_STACK_SIZE, Arm64AddSubtract::Subtract);
    store_stack(FRAME_STACK_OFFSET, argument(0), &mut code);
    store_stack(OUTPUT_STACK_OFFSET, argument(1), &mut code);
    store_stack(
        CONSUME_LINK_STACK_OFFSET,
        Arm64NocterAbi::link_register(),
        &mut code,
    );
    store_stack(
        CONSUME_ALLOCATION_CONTEXT_STACK_OFFSET,
        Arm64NocterAbi::allocation_context_register(),
        &mut code,
    );
    store_stack(
        CONSUME_PROCESS_CONTEXT_STACK_OFFSET,
        Arm64NocterAbi::process_context_register(),
        &mut code,
    );
    validate_state(states.completed(), &mut code);
    write_output(&mut code);
    release_temporary_buffers(&mut code)?;
    release_group_frame(&mut code)?;
    load_stack(
        CONSUME_ALLOCATION_CONTEXT_STACK_OFFSET,
        Arm64NocterAbi::allocation_context_register(),
        &mut code,
    );
    load_stack(
        CONSUME_PROCESS_CONTEXT_STACK_OFFSET,
        Arm64NocterAbi::process_context_register(),
        &mut code,
    );
    load_stack(
        CONSUME_LINK_STACK_OFFSET,
        Arm64NocterAbi::link_register(),
        &mut code,
    );
    crate::frame_access::adjust_stack(&mut code, CONSUME_STACK_SIZE, Arm64AddSubtract::Add);
    return_to_caller(&mut code);
    code.finish()
}

fn poll_child(code: &mut Arm64CodeBuilder) -> Result<(), crate::Arm64CodeError> {
    load_frame(argument(3), code);
    load(argument(3), CHILD_POINTER_OFFSET, argument(4), code);
    load(argument(3), SCAN_INDEX_OFFSET, argument(5), code);
    crate::frame_access::load_immediate(code, argument(6), 8, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination: argument(4),
        left: argument(5),
        right: argument(6),
        addend: Arm64DataRegister::General(argument(4)),
        subtract_product: false,
    });
    load(argument(4), 0, argument(0), code);
    validate_nonzero(argument(0), code);
    load(
        argument(0),
        Arm64NocterAbi::asynchronous().resume_function_offset(),
        argument(4),
        code,
    );
    code.append(Arm64Instruction::BranchRegister {
        target: argument(4),
        link: true,
    });

    let completed = code.create_label();
    let pending = code.create_label();
    compare_immediate(
        argument(0),
        Arm64NocterAbi::asynchronous().completed_status(),
        code,
    );
    code.branch_conditional(completed, Arm64BranchCondition::Equal);
    compare_immediate(
        argument(0),
        Arm64NocterAbi::asynchronous().pending_status(),
        code,
    );
    code.branch_conditional(pending, Arm64BranchCondition::Equal);
    trap_state(code);

    code.bind(completed)?;
    load_frame(argument(3), code);
    load(argument(3), SCAN_INDEX_OFFSET, argument(5), code);
    complete(true, 0, lifecycle_states()?.completed(), code);

    code.bind(pending)?;
    validate_pending_result(code);
    record_pending_result(code);
    load_frame(argument(3), code);
    load(argument(3), SCAN_INDEX_OFFSET, argument(5), code);
    Ok(())
}

fn validate_pending_result(code: &mut Arm64CodeBuilder) {
    validate_nonzero_with_trap(
        argument(1),
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitRecordCorruption,
        code,
    );
    validate_nonzero_with_trap(
        argument(2),
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitRecordCorruption,
        code,
    );
    validate_dynamic_count(argument(2), code);
}

fn record_pending_result(code: &mut Arm64CodeBuilder) {
    load_frame(argument(3), code);
    load(argument(3), METADATA_POINTER_OFFSET, argument(4), code);
    load(argument(3), SCAN_INDEX_OFFSET, argument(5), code);
    crate::frame_access::load_immediate(
        code,
        argument(6),
        METADATA_RECORD_SIZE,
        Arm64DataSize::Bits64,
    );
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination: argument(4),
        left: argument(5),
        right: argument(6),
        addend: Arm64DataRegister::General(argument(4)),
        subtract_product: false,
    });
    store(argument(4), 0, argument(1), code);
    store(argument(4), 8, argument(2), code);
    load(argument(3), INTEREST_COUNT_OFFSET, argument(4), code);
    add_register(argument(4), argument(4), argument(2), true, code);
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::CarryClear);
    trap_wait(code);
    code.bind(valid)
        .expect("fresh dynamic-interest overflow label binds once");
    validate_dynamic_count(argument(4), code);
    load_frame(argument(3), code);
    store(argument(3), INTEREST_COUNT_OFFSET, argument(4), code);
}

fn ensure_metadata_buffer(code: &mut Arm64CodeBuilder) -> Result<(), crate::Arm64CodeError> {
    load_frame(argument(3), code);
    load(argument(3), CHILD_COUNT_OFFSET, argument(1), code);
    multiply_size(argument(1), METADATA_RECORD_SIZE, code);
    load(argument(3), METADATA_POINTER_OFFSET, argument(4), code);
    load(argument(3), METADATA_SIZE_OFFSET, argument(5), code);
    let allocate = code.create_label();
    let ready = code.create_label();
    compare_immediate(argument(4), 0, code);
    code.branch_conditional(allocate, Arm64BranchCondition::Equal);
    compare_register(argument(5), argument(1), code);
    code.branch_conditional(ready, Arm64BranchCondition::Equal);
    trap_state(code);

    code.bind(allocate)?;
    compare_immediate(argument(5), 0, code);
    let size_absent = code.create_label();
    code.branch_conditional(size_absent, Arm64BranchCondition::Equal);
    trap_state(code);
    code.bind(size_absent)?;
    load_frame(argument(3), code);
    store(argument(3), METADATA_SIZE_OFFSET, argument(1), code);
    crate::darwin_memory_code::emit_map(code)?;
    load_frame(argument(3), code);
    store(argument(3), METADATA_POINTER_OFFSET, argument(0), code);
    code.bind(ready)
}

fn ensure_interest_buffer(code: &mut Arm64CodeBuilder) -> Result<(), crate::Arm64CodeError> {
    let schema = Arm64NocterAbi::asynchronous();
    load_frame(argument(3), code);
    load(argument(3), INTEREST_COUNT_OFFSET, argument(1), code);
    validate_nonzero_with_trap(
        argument(1),
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitRecordCorruption,
        code,
    );
    multiply_size(argument(1), schema.interest_record_size(), code);
    load(argument(3), INTEREST_POINTER_OFFSET, argument(4), code);
    load(argument(3), INTEREST_SIZE_OFFSET, argument(5), code);
    let allocate = code.create_label();
    let release = code.create_label();
    let ready = code.create_label();
    compare_immediate(argument(4), 0, code);
    code.branch_conditional(allocate, Arm64BranchCondition::Equal);
    compare_immediate(argument(5), 0, code);
    code.branch_conditional(release, Arm64BranchCondition::Equal);
    compare_register(argument(1), argument(5), code);
    code.branch_conditional(ready, Arm64BranchCondition::UnsignedLowerOrSame);
    code.branch(release, false);

    code.bind(release)?;
    release_buffer(INTEREST_POINTER_OFFSET, INTEREST_SIZE_OFFSET, code)?;

    code.bind(allocate)?;
    load_frame(argument(3), code);
    load(argument(3), INTEREST_POINTER_OFFSET, argument(4), code);
    load(argument(3), INTEREST_SIZE_OFFSET, argument(5), code);
    compare_immediate(argument(4), 0, code);
    let absent = code.create_label();
    code.branch_conditional(absent, Arm64BranchCondition::Equal);
    trap_state(code);
    code.bind(absent)?;
    compare_immediate(argument(5), 0, code);
    let size_absent = code.create_label();
    code.branch_conditional(size_absent, Arm64BranchCondition::Equal);
    trap_state(code);
    code.bind(size_absent)?;
    load(argument(3), INTEREST_COUNT_OFFSET, argument(1), code);
    multiply_size(argument(1), schema.interest_record_size(), code);
    store(argument(3), INTEREST_SIZE_OFFSET, argument(1), code);
    crate::darwin_memory_code::emit_map(code)?;
    load_frame(argument(3), code);
    store(argument(3), INTEREST_POINTER_OFFSET, argument(0), code);
    code.bind(ready)
}

fn multiply_size(value: crate::Arm64Register, stride: u64, code: &mut Arm64CodeBuilder) {
    crate::frame_access::load_immediate(code, argument(2), stride, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination: value,
        left: value,
        right: argument(2),
        addend: Arm64DataRegister::Zero,
        subtract_product: false,
    });
}

fn copy_interests(code: &mut Arm64CodeBuilder) -> Result<(), crate::Arm64CodeError> {
    let schema = Arm64NocterAbi::asynchronous();
    load_frame(argument(3), code);
    load(argument(3), INTEREST_POINTER_OFFSET, argument(3), code);
    load_frame(argument(4), code);
    load(argument(4), METADATA_POINTER_OFFSET, argument(4), code);
    load_frame(argument(5), code);
    load(argument(5), CHILD_COUNT_OFFSET, argument(5), code);
    let next_child = code.create_label();
    let complete = code.create_label();
    code.bind(next_child)?;
    compare_immediate(argument(5), 0, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    load(argument(4), 0, argument(6), code);
    load(argument(4), 8, argument(7), code);
    let next_interest = code.create_label();
    let child_complete = code.create_label();
    code.bind(next_interest)?;
    compare_immediate(argument(7), 0, code);
    code.branch_conditional(child_complete, Arm64BranchCondition::Equal);
    for offset in [
        schema.interest_kind_offset(),
        schema.interest_subject_offset(),
        schema.interest_detail_offset(),
        schema.interest_readiness_pointer_offset(),
    ] {
        load(argument(6), offset, argument(0), code);
        store(argument(3), offset, argument(0), code);
    }
    add_immediate(argument(6), schema.interest_record_size(), code);
    add_immediate(argument(3), schema.interest_record_size(), code);
    subtract_immediate(argument(7), 1, code);
    code.branch(next_interest, false);
    code.bind(child_complete)?;
    add_immediate(argument(4), METADATA_RECORD_SIZE, code);
    subtract_immediate(argument(5), 1, code);
    code.branch(next_child, false);
    code.bind(complete)
}

fn complete(
    available: bool,
    fallback_index: u64,
    completed_state: u64,
    code: &mut Arm64CodeBuilder,
) {
    let schema = Arm64NocterAbi::asynchronous();
    load_frame(argument(3), code);
    store_immediate(
        argument(3),
        AVAILABLE_VALUE_OFFSET,
        u64::from(available),
        code,
    );
    if available {
        load(argument(3), SCAN_INDEX_OFFSET, argument(4), code);
        store(argument(3), INDEX_VALUE_OFFSET, argument(4), code);
    } else {
        store_immediate(argument(3), INDEX_VALUE_OFFSET, fallback_index, code);
    }
    store_immediate(
        argument(3),
        schema.state_tag_offset(),
        completed_state,
        code,
    );
    crate::frame_access::load_immediate(
        code,
        argument(0),
        schema.completed_status(),
        Arm64DataSize::Bits64,
    );
    zero(argument(1), code);
    zero(argument(2), code);
    emit_call_epilogue(code);
}

fn write_output(code: &mut Arm64CodeBuilder) {
    load_stack(OUTPUT_STACK_OFFSET, argument(3), code);
    load_frame(argument(4), code);
    load(argument(4), AVAILABLE_OUTPUT_OFFSET, argument(5), code);
    load(argument(4), INDEX_OUTPUT_OFFSET, argument(6), code);
    load(argument(4), AVAILABLE_VALUE_OFFSET, argument(7), code);
    add_register(argument(5), argument(3), argument(5), false, code);
    crate::address_code::store_native(code, Arm64LoadStoreSize::Byte, argument(7), argument(5), 0);
    load(argument(4), INDEX_VALUE_OFFSET, argument(7), code);
    add_register(argument(6), argument(3), argument(6), false, code);
    crate::address_code::store_native(
        code,
        Arm64LoadStoreSize::Double,
        argument(7),
        argument(6),
        0,
    );
}

fn release_temporary_buffers(code: &mut Arm64CodeBuilder) -> Result<(), crate::Arm64CodeError> {
    release_buffer(METADATA_POINTER_OFFSET, METADATA_SIZE_OFFSET, code)?;
    release_buffer(INTEREST_POINTER_OFFSET, INTEREST_SIZE_OFFSET, code)
}

fn release_buffer(
    pointer_offset: u64,
    size_offset: u64,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    load_frame(argument(3), code);
    load(argument(3), pointer_offset, argument(0), code);
    load(argument(3), size_offset, argument(1), code);
    let absent = code.create_label();
    let release = code.create_label();
    let complete = code.create_label();
    compare_immediate(argument(0), 0, code);
    code.branch_conditional(absent, Arm64BranchCondition::Equal);
    compare_immediate(argument(1), 0, code);
    code.branch_conditional(release, Arm64BranchCondition::NotEqual);
    trap_state(code);
    code.bind(absent)?;
    compare_immediate(argument(1), 0, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    trap_state(code);
    code.bind(release)?;
    crate::darwin_memory_code::emit_unmap(
        code,
        crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameReleaseFailure,
    )?;
    load_frame(argument(3), code);
    store_immediate(argument(3), pointer_offset, 0, code);
    store_immediate(argument(3), size_offset, 0, code);
    code.bind(complete)
}

fn release_group_frame(code: &mut Arm64CodeBuilder) -> Result<(), crate::Arm64CodeError> {
    load_frame(argument(0), code);
    crate::frame_access::load_immediate(code, argument(1), GROUP_FRAME_SIZE, Arm64DataSize::Bits64);
    crate::darwin_memory_code::emit_unmap(
        code,
        crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameReleaseFailure,
    )
}

fn validate_dynamic_count(value: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    crate::frame_access::load_immediate(
        code,
        argument(7),
        MAX_DYNAMIC_COUNT,
        Arm64DataSize::Bits64,
    );
    compare_register(value, argument(7), code);
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::UnsignedLowerOrSame);
    trap_wait(code);
    code.bind(valid)
        .expect("fresh dynamic-count validation label binds once");
}
