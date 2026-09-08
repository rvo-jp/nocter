use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64AsyncInterestLifecycleTargets,
    Arm64BaseRegister, Arm64BranchCondition, Arm64Code, Arm64CodeBuilder, Arm64DataSize,
    Arm64Instruction, Arm64LoadStoreSize, Arm64NocterAbi,
};
use nocter_runtime_contract::RuntimeAsyncStateTags;

const CAPTURE_STACK_SIZE: u64 = 32;
const SUBJECT_STACK_OFFSET: u64 = 0;
const DETAIL_STACK_OFFSET: u64 = 8;
const DEADLINE_STACK_OFFSET: u64 = 16;
const ALLOCATION_CONTEXT_STACK_OFFSET: u64 = 24;

#[derive(Clone, Copy)]
enum InterestConstructor {
    DescriptorReadiness,
    DescriptorReadinessOrDeadline,
    MonotonicDeadline,
}

impl InterestConstructor {
    const fn interest_count(self) -> u64 {
        match self {
            Self::DescriptorReadiness | Self::MonotonicDeadline => 1,
            Self::DescriptorReadinessOrDeadline => 2,
        }
    }
}

pub(crate) fn materialize_descriptor_constructor(
    lifecycle: Arm64AsyncInterestLifecycleTargets,
) -> Result<Arm64Code, crate::Arm64CodeError> {
    materialize_constructor(InterestConstructor::DescriptorReadiness, lifecycle)
}

pub(crate) fn materialize_deadline_constructor(
    lifecycle: Arm64AsyncInterestLifecycleTargets,
) -> Result<Arm64Code, crate::Arm64CodeError> {
    materialize_constructor(InterestConstructor::MonotonicDeadline, lifecycle)
}

pub(crate) fn materialize_descriptor_or_deadline_constructor(
    lifecycle: Arm64AsyncInterestLifecycleTargets,
) -> Result<Arm64Code, crate::Arm64CodeError> {
    materialize_constructor(
        InterestConstructor::DescriptorReadinessOrDeadline,
        lifecycle,
    )
}

fn materialize_constructor(
    kind: InterestConstructor,
    lifecycle: Arm64AsyncInterestLifecycleTargets,
) -> Result<Arm64Code, crate::Arm64CodeError> {
    let schema = Arm64NocterAbi::asynchronous();
    let states = lifecycle_states()?;
    let mut code = Arm64CodeBuilder::new();
    crate::frame_access::adjust_stack(&mut code, CAPTURE_STACK_SIZE, Arm64AddSubtract::Subtract);
    store_stack(SUBJECT_STACK_OFFSET, argument(0), &mut code);
    if matches!(
        kind,
        InterestConstructor::DescriptorReadiness
            | InterestConstructor::DescriptorReadinessOrDeadline
    ) {
        store_stack(DETAIL_STACK_OFFSET, argument(1), &mut code);
    }
    if matches!(kind, InterestConstructor::DescriptorReadinessOrDeadline) {
        store_stack(DEADLINE_STACK_OFFSET, argument(2), &mut code);
    }
    store_stack(
        ALLOCATION_CONTEXT_STACK_OFFSET,
        Arm64NocterAbi::allocation_context_register(),
        &mut code,
    );
    crate::frame_access::load_immediate(
        &mut code,
        argument(1),
        frame_size(kind.interest_count()),
        Arm64DataSize::Bits64,
    );
    crate::darwin_memory_code::emit_map(&mut code)?;
    crate::address_code::move_register(&mut code, argument(0), argument(3));
    initialize_entry(
        argument(3),
        schema.resume_function_offset(),
        lifecycle.resume(),
        &mut code,
    );
    initialize_entry(
        argument(3),
        schema.cancel_function_offset(),
        lifecycle.cancel(),
        &mut code,
    );
    initialize_entry(
        argument(3),
        schema.consume_function_offset(),
        lifecycle.consume(),
        &mut code,
    );
    store_immediate(
        argument(3),
        schema.state_tag_offset(),
        states.initial(),
        &mut code,
    );
    load_stack(ALLOCATION_CONTEXT_STACK_OFFSET, argument(4), &mut code);
    store(
        argument(3),
        schema.allocation_context_offset(),
        argument(4),
        &mut code,
    );
    store_immediate(
        argument(3),
        readiness_offset(kind.interest_count()),
        0,
        &mut code,
    );
    match kind {
        InterestConstructor::DescriptorReadiness => {
            write_descriptor_interest(argument(3), 0, &mut code)?;
            write_readiness_target(argument(3), 0, kind.interest_count(), &mut code);
        }
        InterestConstructor::DescriptorReadinessOrDeadline => {
            write_descriptor_interest(argument(3), 0, &mut code)?;
            write_timer_interest(argument(3), 1, DEADLINE_STACK_OFFSET, &mut code);
            write_readiness_target(argument(3), 0, kind.interest_count(), &mut code);
            write_readiness_target(argument(3), 1, kind.interest_count(), &mut code);
        }
        InterestConstructor::MonotonicDeadline => {
            write_timer_interest(argument(3), 0, SUBJECT_STACK_OFFSET, &mut code);
            write_readiness_target(argument(3), 0, kind.interest_count(), &mut code);
        }
    }
    crate::address_code::move_register(&mut code, argument(3), argument(0));
    crate::frame_access::adjust_stack(&mut code, CAPTURE_STACK_SIZE, Arm64AddSubtract::Add);
    return_to_caller(&mut code);
    code.finish()
}

fn write_readiness_target(
    frame: crate::Arm64Register,
    index: u64,
    interest_count: u64,
    code: &mut Arm64CodeBuilder,
) {
    let schema = Arm64NocterAbi::asynchronous();
    crate::address_code::move_register(code, frame, argument(4));
    add_immediate(argument(4), readiness_offset(interest_count), code);
    store(
        frame,
        interest_offset(index) + schema.interest_readiness_pointer_offset(),
        argument(4),
        code,
    );
}

fn write_descriptor_interest(
    frame: crate::Arm64Register,
    index: u64,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let schema = Arm64NocterAbi::asynchronous();
    let offset = interest_offset(index);
    store_immediate(
        frame,
        offset + schema.interest_kind_offset(),
        schema.descriptor_interest_kind(),
        code,
    );
    load_stack(SUBJECT_STACK_OFFSET, argument(4), code);
    store(
        frame,
        offset + schema.interest_subject_offset(),
        argument(4),
        code,
    );
    load_stack(DETAIL_STACK_OFFSET, argument(4), code);
    let readable = code.create_label();
    let direction_ready = code.create_label();
    compare_state(argument(4), 0, readable, code);
    crate::frame_access::load_immediate(
        code,
        argument(4),
        schema.writable_interest_detail(),
        Arm64DataSize::Bits64,
    );
    code.branch(direction_ready, false);
    code.bind(readable)?;
    crate::frame_access::load_immediate(
        code,
        argument(4),
        schema.readable_interest_detail(),
        Arm64DataSize::Bits64,
    );
    code.bind(direction_ready)?;
    store(
        frame,
        offset + schema.interest_detail_offset(),
        argument(4),
        code,
    );
    Ok(())
}

fn write_timer_interest(
    frame: crate::Arm64Register,
    index: u64,
    subject_stack_offset: u64,
    code: &mut Arm64CodeBuilder,
) {
    let schema = Arm64NocterAbi::asynchronous();
    let offset = interest_offset(index);
    store_immediate(
        frame,
        offset + schema.interest_kind_offset(),
        schema.timer_interest_kind(),
        code,
    );
    load_stack(subject_stack_offset, argument(4), code);
    store(
        frame,
        offset + schema.interest_subject_offset(),
        argument(4),
        code,
    );
    store_immediate(frame, offset + schema.interest_detail_offset(), 0, code);
}

pub(crate) fn materialize_resume(interest_count: u64) -> Result<Arm64Code, crate::Arm64CodeError> {
    let schema = Arm64NocterAbi::asynchronous();
    let states = lifecycle_states()?;
    let suspended_tag = states
        .suspension(0)
        .ok_or(crate::Arm64CodeError::AsyncStateTagExhausted)?;
    let mut code = Arm64CodeBuilder::new();
    crate::address_code::move_register(&mut code, argument(0), argument(3));
    crate::address_code::load_native(
        &mut code,
        Arm64LoadStoreSize::Double,
        None,
        argument(4),
        argument(3),
        schema.state_tag_offset(),
    );
    let initial = code.create_label();
    let suspended = code.create_label();
    compare_state(argument(4), states.initial(), initial, &mut code);
    compare_state(argument(4), suspended_tag, suspended, &mut code);
    trap_state(&mut code);

    code.bind(initial)?;
    store_immediate(
        argument(3),
        schema.state_tag_offset(),
        suspended_tag,
        &mut code,
    );
    emit_pending(argument(3), interest_count, &mut code);

    code.bind(suspended)?;
    crate::address_code::load_native(
        &mut code,
        Arm64LoadStoreSize::Double,
        None,
        argument(4),
        argument(3),
        readiness_offset(interest_count),
    );
    let not_ready = code.create_label();
    compare_state(argument(4), 0, not_ready, &mut code);
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
    crate::frame_access::load_immediate(&mut code, argument(1), 0, Arm64DataSize::Bits64);
    crate::frame_access::load_immediate(&mut code, argument(2), 0, Arm64DataSize::Bits64);
    return_to_caller(&mut code);
    code.bind(not_ready)?;
    emit_pending(argument(3), interest_count, &mut code);
    code.finish()
}

fn emit_pending(frame: crate::Arm64Register, interest_count: u64, code: &mut Arm64CodeBuilder) {
    let schema = Arm64NocterAbi::asynchronous();
    crate::frame_access::load_immediate(
        code,
        argument(0),
        schema.pending_status(),
        Arm64DataSize::Bits64,
    );
    crate::address_code::move_register(code, frame, argument(1));
    add_immediate(argument(1), schema.fixed_header_size(), code);
    crate::frame_access::load_immediate(code, argument(2), interest_count, Arm64DataSize::Bits64);
    return_to_caller(code);
}

pub(crate) fn materialize_cancel(interest_count: u64) -> Result<Arm64Code, crate::Arm64CodeError> {
    // Cancellation is the destruction entry for every still-owned computation handle. A caller
    // may release the handle after completion but before consuming its output.
    let states = lifecycle_states()?;
    let suspended = states
        .suspension(0)
        .ok_or(crate::Arm64CodeError::AsyncStateTagExhausted)?;
    materialize_release(
        &[states.initial(), suspended, states.completed()],
        interest_count,
    )
}

pub(crate) fn materialize_consume(interest_count: u64) -> Result<Arm64Code, crate::Arm64CodeError> {
    materialize_release(&[lifecycle_states()?.completed()], interest_count)
}

fn lifecycle_states() -> Result<RuntimeAsyncStateTags, crate::Arm64CodeError> {
    Arm64NocterAbi::asynchronous()
        .state_tags(1)
        .ok_or(crate::Arm64CodeError::AsyncStateTagExhausted)
}

fn materialize_release(
    accepted: &[u64],
    interest_count: u64,
) -> Result<Arm64Code, crate::Arm64CodeError> {
    let schema = Arm64NocterAbi::asynchronous();
    let mut code = Arm64CodeBuilder::new();
    crate::address_code::load_native(
        &mut code,
        Arm64LoadStoreSize::Double,
        None,
        argument(3),
        argument(0),
        schema.state_tag_offset(),
    );
    let release = code.create_label();
    for state in accepted {
        compare_state(argument(3), *state, release, &mut code);
    }
    trap_state(&mut code);
    code.bind(release)?;
    crate::frame_access::load_immediate(
        &mut code,
        argument(1),
        frame_size(interest_count),
        Arm64DataSize::Bits64,
    );
    crate::darwin_memory_code::emit_unmap(
        &mut code,
        crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameReleaseFailure,
    )?;
    return_to_caller(&mut code);
    code.finish()
}

const fn frame_size(interest_count: u64) -> u64 {
    readiness_offset(interest_count) + Arm64NocterAbi::word_size()
}

const fn interest_offset(index: u64) -> u64 {
    let schema = Arm64NocterAbi::asynchronous();
    schema.fixed_header_size() + schema.interest_record_size() * index
}

const fn readiness_offset(interest_count: u64) -> u64 {
    let schema = Arm64NocterAbi::asynchronous();
    schema.fixed_header_size() + schema.interest_record_size() * interest_count
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

fn store_immediate(
    frame: crate::Arm64Register,
    offset: u64,
    value: u64,
    code: &mut Arm64CodeBuilder,
) {
    crate::frame_access::load_immediate(code, argument(4), value, Arm64DataSize::Bits64);
    store(frame, offset, argument(4), code);
}

fn store(
    frame: crate::Arm64Register,
    offset: u64,
    source: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    crate::address_code::store_native(code, Arm64LoadStoreSize::Double, source, frame, offset);
}

fn store_stack(offset: u64, source: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
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

fn compare_state(
    actual: crate::Arm64Register,
    expected: u64,
    target: crate::Arm64LabelId,
    code: &mut Arm64CodeBuilder,
) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(actual),
        immediate: u16::try_from(expected).expect("async primitive states fit immediate"),
        shift_12: false,
    });
    code.branch_conditional(target, Arm64BranchCondition::Equal);
}

fn add_immediate(value: crate::Arm64Register, immediate: u64, code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(value),
        source: Arm64BaseRegister::General(value),
        immediate: u16::try_from(immediate).expect("async primitive offset fits immediate"),
        shift_12: false,
    });
}

fn trap_state(code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameStateCorruption.immediate(),
    });
}

fn return_to_caller(code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::Return {
        target: Arm64NocterAbi::link_register(),
    });
}

const fn argument(index: u8) -> crate::Arm64Register {
    match Arm64NocterAbi::argument_register(index) {
        Some(register) => register,
        None => panic!("async wait computation uses only ABI argument registers"),
    }
}
