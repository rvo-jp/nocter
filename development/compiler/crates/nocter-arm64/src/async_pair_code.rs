//! Compiler-owned two-child composition lifecycles shared by join and race.

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64AsyncPairTargets, Arm64BaseRegister,
    Arm64BranchCondition, Arm64Code, Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize,
    Arm64Instruction, Arm64LoadStoreSize, Arm64NocterAbi,
};
use nocter_runtime_contract::RuntimeAsyncStateTags;

const FIRST_CHILD_OFFSET: u64 = Arm64NocterAbi::asynchronous().fixed_header_size();
const SECOND_CHILD_OFFSET: u64 = FIRST_CHILD_OFFSET + Arm64NocterAbi::word_size();
const FIRST_DONE_OFFSET: u64 = SECOND_CHILD_OFFSET + Arm64NocterAbi::word_size();
const SECOND_DONE_OFFSET: u64 = FIRST_DONE_OFFSET + Arm64NocterAbi::word_size();
const FIRST_OUTPUT_OFFSET: u64 = SECOND_DONE_OFFSET + Arm64NocterAbi::word_size();
const SECOND_OUTPUT_OFFSET: u64 = FIRST_OUTPUT_OFFSET + Arm64NocterAbi::word_size();
const INTEREST_BUFFER_OFFSET: u64 = SECOND_OUTPUT_OFFSET + Arm64NocterAbi::word_size();
const INTEREST_BUFFER_SIZE_OFFSET: u64 = INTEREST_BUFFER_OFFSET + Arm64NocterAbi::word_size();
const FIRST_INTEREST_POINTER_OFFSET: u64 =
    INTEREST_BUFFER_SIZE_OFFSET + Arm64NocterAbi::word_size();
const FIRST_INTEREST_COUNT_OFFSET: u64 =
    FIRST_INTEREST_POINTER_OFFSET + Arm64NocterAbi::word_size();
const SECOND_INTEREST_POINTER_OFFSET: u64 =
    FIRST_INTEREST_COUNT_OFFSET + Arm64NocterAbi::word_size();
const SECOND_INTEREST_COUNT_OFFSET: u64 =
    SECOND_INTEREST_POINTER_OFFSET + Arm64NocterAbi::word_size();
pub(super) const PAIR_FRAME_SIZE: u64 = SECOND_INTEREST_COUNT_OFFSET + Arm64NocterAbi::word_size();
const MAX_INTEREST_COUNT: u64 = u32::MAX as u64;

const FRAME_STACK_OFFSET: u64 = 0;
const LINK_STACK_OFFSET: u64 = 8;
const ALLOCATION_CONTEXT_STACK_OFFSET: u64 = 16;
const PROCESS_CONTEXT_STACK_OFFSET: u64 = 24;
const CALL_STACK_SIZE: u64 = 32;

const OUTPUT_STACK_OFFSET: u64 = 8;
const CONSUME_LINK_STACK_OFFSET: u64 = 16;
const CONSUME_ALLOCATION_CONTEXT_STACK_OFFSET: u64 = 24;
const CONSUME_PROCESS_CONTEXT_STACK_OFFSET: u64 = 32;
const CONSUME_STACK_SIZE: u64 = 48;

pub(super) fn initialize_pair_frame(
    frame: crate::Arm64Register,
    targets: Arm64AsyncPairTargets,
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
    load_stack(
        crate::async_pair_constructor_code::ALLOCATION_CONTEXT_STACK_OFFSET,
        argument(4),
        code,
    );
    store(frame, schema.allocation_context_offset(), argument(4), code);
    initialize_from_stack(
        frame,
        FIRST_CHILD_OFFSET,
        crate::async_pair_constructor_code::FIRST_STACK_OFFSET,
        code,
    );
    initialize_from_stack(
        frame,
        SECOND_CHILD_OFFSET,
        crate::async_pair_constructor_code::SECOND_STACK_OFFSET,
        code,
    );
    initialize_from_stack(
        frame,
        FIRST_OUTPUT_OFFSET,
        crate::async_pair_constructor_code::FIRST_OUTPUT_STACK_OFFSET,
        code,
    );
    initialize_from_stack(
        frame,
        SECOND_OUTPUT_OFFSET,
        crate::async_pair_constructor_code::SECOND_OUTPUT_STACK_OFFSET,
        code,
    );
    for offset in [
        FIRST_DONE_OFFSET,
        SECOND_DONE_OFFSET,
        INTEREST_BUFFER_OFFSET,
        INTEREST_BUFFER_SIZE_OFFSET,
        FIRST_INTEREST_POINTER_OFFSET,
        FIRST_INTEREST_COUNT_OFFSET,
        SECOND_INTEREST_POINTER_OFFSET,
        SECOND_INTEREST_COUNT_OFFSET,
    ] {
        store_immediate(frame, offset, 0, code);
    }
    Ok(())
}

/// Polls both owned children from left to right and returns their combined wait-interest slice.
pub(crate) fn materialize_join_resume() -> Result<Arm64Code, crate::Arm64CodeError> {
    let schema = Arm64NocterAbi::asynchronous();
    let states = lifecycle_states()?;
    let mut code = Arm64CodeBuilder::new();
    emit_call_prologue(argument(0), &mut code);
    validate_state(states.initial(), &mut code);
    release_interest_buffer(&mut code)?;
    clear_pending_interests(&mut code);
    poll_child(
        FIRST_CHILD_OFFSET,
        FIRST_DONE_OFFSET,
        FIRST_INTEREST_POINTER_OFFSET,
        FIRST_INTEREST_COUNT_OFFSET,
        &mut code,
    )?;
    poll_child(
        SECOND_CHILD_OFFSET,
        SECOND_DONE_OFFSET,
        SECOND_INTEREST_POINTER_OFFSET,
        SECOND_INTEREST_COUNT_OFFSET,
        &mut code,
    )?;

    let pending = code.create_label();
    load_frame(argument(3), &mut code);
    load(argument(3), FIRST_DONE_OFFSET, argument(4), &mut code);
    compare_immediate(argument(4), 1, &mut code);
    code.branch_conditional(pending, Arm64BranchCondition::NotEqual);
    load(argument(3), SECOND_DONE_OFFSET, argument(4), &mut code);
    compare_immediate(argument(4), 1, &mut code);
    code.branch_conditional(pending, Arm64BranchCondition::NotEqual);
    store_immediate(
        argument(3),
        schema.state_tag_offset(),
        states.completed(),
        &mut code,
    );
    crate::frame_access::load_immediate(
        &mut code,
        argument(0),
        schema.completed_status(),
        Arm64DataSize::Bits64,
    );
    zero(argument(1), &mut code);
    zero(argument(2), &mut code);
    emit_call_epilogue(&mut code);

    code.bind(pending)?;
    emit_combined_pending(&mut code)?;
    code.finish()
}

/// Polls two same-output children from left to right and selects one deterministic winner.
pub(crate) fn materialize_race_resume() -> Result<Arm64Code, crate::Arm64CodeError> {
    let states = lifecycle_states()?;
    let mut code = Arm64CodeBuilder::new();
    emit_call_prologue(argument(0), &mut code);
    validate_state(states.initial(), &mut code);
    release_interest_buffer(&mut code)?;
    clear_pending_interests(&mut code);

    poll_child(
        FIRST_CHILD_OFFSET,
        FIRST_DONE_OFFSET,
        FIRST_INTEREST_POINTER_OFFSET,
        FIRST_INTEREST_COUNT_OFFSET,
        &mut code,
    )?;
    let first_wins = code.create_label();
    load_frame(argument(3), &mut code);
    load(argument(3), FIRST_DONE_OFFSET, argument(4), &mut code);
    compare_immediate(argument(4), 1, &mut code);
    code.branch_conditional(first_wins, Arm64BranchCondition::Equal);

    poll_child(
        SECOND_CHILD_OFFSET,
        SECOND_DONE_OFFSET,
        SECOND_INTEREST_POINTER_OFFSET,
        SECOND_INTEREST_COUNT_OFFSET,
        &mut code,
    )?;
    let second_wins = code.create_label();
    load_frame(argument(3), &mut code);
    load(argument(3), SECOND_DONE_OFFSET, argument(4), &mut code);
    compare_immediate(argument(4), 1, &mut code);
    code.branch_conditional(second_wins, Arm64BranchCondition::Equal);

    emit_combined_pending(&mut code)?;

    code.bind(first_wins)?;
    release_child(SECOND_CHILD_OFFSET, false, &mut code);
    complete_pair(states.completed(), &mut code);

    code.bind(second_wins)?;
    release_child(FIRST_CHILD_OFFSET, false, &mut code);
    complete_pair(states.completed(), &mut code);
    code.finish()
}

fn emit_combined_pending(code: &mut Arm64CodeBuilder) -> Result<(), crate::Arm64CodeError> {
    let schema = Arm64NocterAbi::asynchronous();
    allocate_combined_interests(code)?;
    load_frame(argument(3), code);
    load(argument(3), INTEREST_BUFFER_OFFSET, argument(3), code);
    copy_interest_records(
        FIRST_INTEREST_POINTER_OFFSET,
        FIRST_INTEREST_COUNT_OFFSET,
        code,
    )?;
    copy_interest_records(
        SECOND_INTEREST_POINTER_OFFSET,
        SECOND_INTEREST_COUNT_OFFSET,
        code,
    )?;
    load_frame(argument(4), code);
    load(argument(4), INTEREST_BUFFER_OFFSET, argument(1), code);
    load(argument(4), FIRST_INTEREST_COUNT_OFFSET, argument(2), code);
    load(argument(4), SECOND_INTEREST_COUNT_OFFSET, argument(3), code);
    add_register(argument(2), argument(2), argument(3), false, code);
    crate::frame_access::load_immediate(
        code,
        argument(0),
        schema.pending_status(),
        Arm64DataSize::Bits64,
    );
    emit_call_epilogue(code);
    Ok(())
}

fn complete_pair(completed_state: u64, code: &mut Arm64CodeBuilder) {
    let schema = Arm64NocterAbi::asynchronous();
    load_frame(argument(3), code);
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

/// Cancels both still-owned children exactly once, then releases the join frame.
pub(crate) fn materialize_join_cancel() -> Result<Arm64Code, crate::Arm64CodeError> {
    let states = lifecycle_states()?;
    let mut code = Arm64CodeBuilder::new();
    emit_call_prologue(argument(0), &mut code);
    validate_state_any(&[states.initial(), states.completed()], &mut code)?;
    release_interest_buffer(&mut code)?;
    release_child(FIRST_CHILD_OFFSET, false, &mut code);
    release_child(SECOND_CHILD_OFFSET, false, &mut code);
    release_pair_frame(&mut code)?;
    emit_call_epilogue(&mut code);
    code.finish()
}

/// Cancels both unselected children, or the retained winner after completion.
pub(crate) fn materialize_race_cancel() -> Result<Arm64Code, crate::Arm64CodeError> {
    let states = lifecycle_states()?;
    let mut code = Arm64CodeBuilder::new();
    emit_call_prologue(argument(0), &mut code);

    load_frame(argument(3), &mut code);
    load(
        argument(3),
        Arm64NocterAbi::asynchronous().state_tag_offset(),
        argument(4),
        &mut code,
    );
    let initial = code.create_label();
    let completed = code.create_label();
    compare_immediate(argument(4), states.initial(), &mut code);
    code.branch_conditional(initial, Arm64BranchCondition::Equal);
    compare_immediate(argument(4), states.completed(), &mut code);
    code.branch_conditional(completed, Arm64BranchCondition::Equal);
    trap_state(&mut code);

    code.bind(initial)?;
    release_interest_buffer(&mut code)?;
    release_child(FIRST_CHILD_OFFSET, false, &mut code);
    release_child(SECOND_CHILD_OFFSET, false, &mut code);
    release_pair_frame(&mut code)?;
    emit_call_epilogue(&mut code);

    code.bind(completed)?;
    let second = code.create_label();
    load_frame(argument(3), &mut code);
    load(argument(3), FIRST_DONE_OFFSET, argument(4), &mut code);
    compare_immediate(argument(4), 1, &mut code);
    code.branch_conditional(second, Arm64BranchCondition::NotEqual);
    release_child(FIRST_CHILD_OFFSET, false, &mut code);
    release_pair_frame(&mut code)?;
    emit_call_epilogue(&mut code);

    code.bind(second)?;
    load_frame(argument(3), &mut code);
    load(argument(3), SECOND_DONE_OFFSET, argument(4), &mut code);
    compare_immediate(argument(4), 1, &mut code);
    let valid_second = code.create_label();
    code.branch_conditional(valid_second, Arm64BranchCondition::Equal);
    trap_state(&mut code);
    code.bind(valid_second)?;
    release_child(SECOND_CHILD_OFFSET, false, &mut code);
    release_pair_frame(&mut code)?;
    emit_call_epilogue(&mut code);
    code.finish()
}

/// Consumes both completed child outputs directly into their frozen tuple placements.
pub(crate) fn materialize_join_consume() -> Result<Arm64Code, crate::Arm64CodeError> {
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
    release_child(FIRST_CHILD_OFFSET, true, &mut code);
    release_child(SECOND_CHILD_OFFSET, true, &mut code);
    release_pair_frame(&mut code)?;
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

/// Writes the winning branch tag and consumes its output into the structural race payload.
pub(crate) fn materialize_race_consume() -> Result<Arm64Code, crate::Arm64CodeError> {
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

    let second = code.create_label();
    let finish = code.create_label();
    load_frame(argument(3), &mut code);
    load(argument(3), FIRST_DONE_OFFSET, argument(4), &mut code);
    compare_immediate(argument(4), 1, &mut code);
    code.branch_conditional(second, Arm64BranchCondition::NotEqual);
    store_race_winner(false, &mut code);
    release_child_to(FIRST_CHILD_OFFSET, SECOND_OUTPUT_OFFSET, &mut code);
    code.branch(finish, false);

    code.bind(second)?;
    load_frame(argument(3), &mut code);
    load(argument(3), SECOND_DONE_OFFSET, argument(4), &mut code);
    compare_immediate(argument(4), 1, &mut code);
    let valid_second = code.create_label();
    code.branch_conditional(valid_second, Arm64BranchCondition::Equal);
    trap_state(&mut code);
    code.bind(valid_second)?;
    store_race_winner(true, &mut code);
    release_child_to(SECOND_CHILD_OFFSET, SECOND_OUTPUT_OFFSET, &mut code);

    code.bind(finish)?;
    release_pair_frame(&mut code)?;
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

fn lifecycle_states() -> Result<RuntimeAsyncStateTags, crate::Arm64CodeError> {
    Arm64NocterAbi::asynchronous()
        .state_tags(0)
        .ok_or(crate::Arm64CodeError::AsyncStateTagExhausted)
}

fn poll_child(
    child_offset: u64,
    done_offset: u64,
    interests_offset: u64,
    count_offset: u64,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let skip = code.create_label();
    let poll = code.create_label();
    load_frame(argument(3), code);
    load(argument(3), done_offset, argument(4), code);
    compare_immediate(argument(4), 1, code);
    code.branch_conditional(skip, Arm64BranchCondition::Equal);
    compare_immediate(argument(4), 0, code);
    code.branch_conditional(poll, Arm64BranchCondition::Equal);
    trap_state(code);

    code.bind(poll)?;
    load(argument(3), child_offset, argument(0), code);
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
    store_immediate(argument(3), done_offset, 1, code);
    code.branch(skip, false);

    code.bind(pending)?;
    validate_pending_result(code);
    load_frame(argument(3), code);
    store(argument(3), interests_offset, argument(1), code);
    store(argument(3), count_offset, argument(2), code);
    code.bind(skip)?;
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
    crate::frame_access::load_immediate(
        code,
        argument(4),
        MAX_INTEREST_COUNT,
        Arm64DataSize::Bits64,
    );
    compare_register(argument(2), argument(4), code);
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::UnsignedLowerOrSame);
    trap_wait(code);
    code.bind(valid)
        .expect("fresh join pending-result label binds once");
}

fn allocate_combined_interests(code: &mut Arm64CodeBuilder) -> Result<(), crate::Arm64CodeError> {
    let schema = Arm64NocterAbi::asynchronous();
    load_frame(argument(3), code);
    load(argument(3), FIRST_INTEREST_COUNT_OFFSET, argument(2), code);
    load(argument(3), SECOND_INTEREST_COUNT_OFFSET, argument(4), code);
    add_register(argument(2), argument(2), argument(4), true, code);
    let no_overflow = code.create_label();
    code.branch_conditional(no_overflow, Arm64BranchCondition::CarryClear);
    trap_wait(code);
    code.bind(no_overflow)?;
    validate_nonzero_with_trap(
        argument(2),
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitRecordCorruption,
        code,
    );
    crate::frame_access::load_immediate(
        code,
        argument(4),
        MAX_INTEREST_COUNT,
        Arm64DataSize::Bits64,
    );
    compare_register(argument(2), argument(4), code);
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::UnsignedLowerOrSame);
    trap_wait(code);
    code.bind(valid)?;

    crate::frame_access::load_immediate(
        code,
        argument(4),
        schema.interest_record_size(),
        Arm64DataSize::Bits64,
    );
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination: argument(1),
        left: argument(2),
        right: argument(4),
        addend: Arm64DataRegister::Zero,
        subtract_product: false,
    });
    store(argument(3), INTEREST_BUFFER_SIZE_OFFSET, argument(1), code);
    crate::darwin_memory_code::emit_map(code)?;
    load_frame(argument(3), code);
    store(argument(3), INTEREST_BUFFER_OFFSET, argument(0), code);
    Ok(())
}

fn copy_interest_records(
    pointer_offset: u64,
    count_offset: u64,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let schema = Arm64NocterAbi::asynchronous();
    load_frame(argument(4), code);
    load(argument(4), pointer_offset, argument(4), code);
    load_frame(argument(5), code);
    load(argument(5), count_offset, argument(5), code);
    let scan = code.create_label();
    let complete = code.create_label();
    code.bind(scan)?;
    compare_immediate(argument(5), 0, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    for offset in [
        schema.interest_kind_offset(),
        schema.interest_subject_offset(),
        schema.interest_detail_offset(),
        schema.interest_readiness_pointer_offset(),
    ] {
        load(argument(4), offset, argument(6), code);
        store(argument(3), offset, argument(6), code);
    }
    add_immediate(argument(4), schema.interest_record_size(), code);
    add_immediate(argument(3), schema.interest_record_size(), code);
    subtract_immediate(argument(5), 1, code);
    code.branch(scan, false);
    code.bind(complete)
}

fn release_interest_buffer(code: &mut Arm64CodeBuilder) -> Result<(), crate::Arm64CodeError> {
    load_frame(argument(3), code);
    load(argument(3), INTEREST_BUFFER_OFFSET, argument(0), code);
    load(argument(3), INTEREST_BUFFER_SIZE_OFFSET, argument(1), code);
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
    store_immediate(argument(3), INTEREST_BUFFER_OFFSET, 0, code);
    store_immediate(argument(3), INTEREST_BUFFER_SIZE_OFFSET, 0, code);
    code.bind(complete)
}

fn clear_pending_interests(code: &mut Arm64CodeBuilder) {
    load_frame(argument(3), code);
    for offset in [
        FIRST_INTEREST_POINTER_OFFSET,
        FIRST_INTEREST_COUNT_OFFSET,
        SECOND_INTEREST_POINTER_OFFSET,
        SECOND_INTEREST_COUNT_OFFSET,
    ] {
        store_immediate(argument(3), offset, 0, code);
    }
}

fn release_child(child_offset: u64, consume: bool, code: &mut Arm64CodeBuilder) {
    load_frame(argument(3), code);
    load(argument(3), child_offset, argument(0), code);
    validate_nonzero(argument(0), code);
    store_immediate(argument(3), child_offset, 0, code);
    let entry_offset = if consume {
        Arm64NocterAbi::asynchronous().consume_function_offset()
    } else {
        Arm64NocterAbi::asynchronous().cancel_function_offset()
    };
    load(argument(0), entry_offset, argument(4), code);
    if consume {
        load_stack(OUTPUT_STACK_OFFSET, argument(1), code);
        let placement_offset = if child_offset == FIRST_CHILD_OFFSET {
            FIRST_OUTPUT_OFFSET
        } else {
            SECOND_OUTPUT_OFFSET
        };
        load_frame(argument(3), code);
        load(argument(3), placement_offset, argument(5), code);
        add_register(argument(1), argument(1), argument(5), false, code);
    }
    code.append(Arm64Instruction::BranchRegister {
        target: argument(4),
        link: true,
    });
}

fn release_child_to(child_offset: u64, placement_offset: u64, code: &mut Arm64CodeBuilder) {
    load_frame(argument(3), code);
    load(argument(3), child_offset, argument(0), code);
    validate_nonzero(argument(0), code);
    store_immediate(argument(3), child_offset, 0, code);
    load(
        argument(0),
        Arm64NocterAbi::asynchronous().consume_function_offset(),
        argument(4),
        code,
    );
    load_stack(OUTPUT_STACK_OFFSET, argument(1), code);
    load_frame(argument(3), code);
    load(argument(3), placement_offset, argument(5), code);
    add_register(argument(1), argument(1), argument(5), false, code);
    code.append(Arm64Instruction::BranchRegister {
        target: argument(4),
        link: true,
    });
}

fn store_race_winner(second: bool, code: &mut Arm64CodeBuilder) {
    load_stack(OUTPUT_STACK_OFFSET, argument(3), code);
    load_frame(argument(4), code);
    load(argument(4), FIRST_OUTPUT_OFFSET, argument(5), code);
    add_register(argument(3), argument(3), argument(5), false, code);
    crate::frame_access::load_immediate(
        code,
        argument(4),
        u64::from(second),
        Arm64DataSize::Bits64,
    );
    crate::address_code::store_native(code, Arm64LoadStoreSize::Byte, argument(4), argument(3), 0);
}

fn release_pair_frame(code: &mut Arm64CodeBuilder) -> Result<(), crate::Arm64CodeError> {
    load_frame(argument(0), code);
    crate::frame_access::load_immediate(code, argument(1), PAIR_FRAME_SIZE, Arm64DataSize::Bits64);
    crate::darwin_memory_code::emit_unmap(
        code,
        crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameReleaseFailure,
    )
}

fn emit_call_prologue(frame: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
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

fn emit_call_epilogue(code: &mut Arm64CodeBuilder) {
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

fn validate_state(expected: u64, code: &mut Arm64CodeBuilder) {
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
        .expect("fresh join state-validation label binds once");
}

fn validate_state_any(
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

fn validate_nonzero_with_trap(
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
        .expect("fresh join pointer-validation label binds once");
}

fn load_frame(destination: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    load_stack(FRAME_STACK_OFFSET, destination, code);
}

fn initialize_entry(
    frame: crate::Arm64Register,
    offset: u64,
    target: crate::Arm64FunctionId,
    code: &mut Arm64CodeBuilder,
) {
    code.load_function_address(target, argument(4));
    store(frame, offset, argument(4), code);
}

fn initialize_from_stack(
    frame: crate::Arm64Register,
    frame_offset: u64,
    stack_offset: u64,
    code: &mut Arm64CodeBuilder,
) {
    load_stack(stack_offset, argument(4), code);
    store(frame, frame_offset, argument(4), code);
}

fn load(
    frame: crate::Arm64Register,
    offset: u64,
    destination: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        destination,
        frame,
        offset,
    );
}

fn store(
    frame: crate::Arm64Register,
    offset: u64,
    source: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    crate::address_code::store_native(code, Arm64LoadStoreSize::Double, source, frame, offset);
}

fn store_immediate(
    frame: crate::Arm64Register,
    offset: u64,
    value: u64,
    code: &mut Arm64CodeBuilder,
) {
    crate::frame_access::load_immediate(code, argument(7), value, Arm64DataSize::Bits64);
    store(frame, offset, argument(7), code);
}

pub(super) fn store_stack(offset: u64, source: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    crate::frame_access::store_at_stack_offset(code, Arm64LoadStoreSize::Double, source, offset);
}

fn load_stack(offset: u64, destination: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    crate::frame_access::load_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        destination,
        offset,
    );
}

fn compare_immediate(value: crate::Arm64Register, expected: u64, code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: u16::try_from(expected).expect("join state and status values fit an immediate"),
        shift_12: false,
    });
}

fn compare_register(
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

fn add_register(
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

fn add_immediate(value: crate::Arm64Register, immediate: u64, code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(value),
        source: Arm64BaseRegister::General(value),
        immediate: u16::try_from(immediate).expect("join interest stride fits an immediate"),
        shift_12: false,
    });
}

fn subtract_immediate(value: crate::Arm64Register, immediate: u64, code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(value),
        source: Arm64BaseRegister::General(value),
        immediate: u16::try_from(immediate).expect("join loop decrement fits an immediate"),
        shift_12: false,
    });
}

fn zero(register: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    crate::frame_access::load_immediate(code, register, 0, Arm64DataSize::Bits64);
}

fn trap_state(code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameStateCorruption.immediate(),
    });
}

fn trap_wait(code: &mut Arm64CodeBuilder) {
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
        None => panic!("async pair composition uses only ABI argument registers"),
    }
}
