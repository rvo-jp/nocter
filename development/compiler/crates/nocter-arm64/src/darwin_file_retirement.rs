use std::fmt;

use nocter_runtime_contract::{
    DarwinFileCompletionAbiSchema, DarwinFileCompletionField, DarwinFileFailureKind,
    DarwinFileRetirementAbiSchema, DarwinFileRetirementAction, DarwinFileRetirementEvent,
    DarwinFileRetirementField, DarwinFileRetirementState, DarwinFileServiceAbiSchema,
    DarwinFileServiceAdmission, DarwinFileServiceField, DarwinFileServiceFunction,
    DarwinFileServiceState,
};

use crate::darwin_kernel_abi::{DarwinSystemCall, emit_system_call};
use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64AtomicUpdateRegisters, Arm64BaseRegister,
    Arm64BranchCondition, Arm64CodeBuilder, Arm64CodeError, Arm64DataRegister, Arm64DataSize,
    Arm64FunctionId, Arm64Instruction, Arm64LoadStoreSize, Arm64ProgramBuilder, Arm64ProgramError,
    Arm64Register,
};

const STACK_SIZE: u64 = 96;
const NOTIFICATION_BYTE_OFFSET: u64 = 0;

/// Generated lifecycle entries for pre-reserved Darwin file retirement.
///
/// Reservation, descriptor ownership, asynchronous close, abandonment, and explicit-close future
/// observation are one family so no caller can publish a state without its mandatory cleanup or
/// notification work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinFileRetirementTargets {
    reserve: Arm64FunctionId,
    abandon_reservation: Arm64FunctionId,
    publish_owner: Arm64FunctionId,
    drop_owner: Arm64FunctionId,
    begin_close: Arm64FunctionId,
    resume_close: Arm64FunctionId,
    cancel_close: Arm64FunctionId,
    consume_close: Arm64FunctionId,
    close_worker: Arm64FunctionId,
}

impl Arm64DarwinFileRetirementTargets {
    /// Declares and defines the complete retirement family.
    ///
    /// # Errors
    ///
    /// Propagates invalid runtime contracts, ARM64 encoding, lifecycle, and program errors.
    pub fn declare(
        program: &mut Arm64ProgramBuilder,
        imports: &crate::Arm64DarwinFileServiceImports,
    ) -> Result<Self, Arm64DarwinFileRetirementError> {
        let targets = Self {
            reserve: program.declare_function(),
            abandon_reservation: program.declare_function(),
            publish_owner: program.declare_function(),
            drop_owner: program.declare_function(),
            begin_close: program.declare_function(),
            resume_close: program.declare_function(),
            cancel_close: program.declare_function(),
            consume_close: program.declare_function(),
            close_worker: program.declare_function(),
        };
        program.define_function(targets.reserve, build_reserve()?.finish()?)?;
        program.define_function(
            targets.abandon_reservation,
            build_abandon_reservation(imports)?.finish()?,
        )?;
        program.define_function(
            targets.publish_owner,
            build_publish_owner(imports)?.finish()?,
        )?;
        program.define_function(
            targets.drop_owner,
            build_begin_retirement(targets.close_worker, false, imports)?.finish()?,
        )?;
        program.define_function(
            targets.begin_close,
            build_begin_retirement(targets.close_worker, true, imports)?.finish()?,
        )?;
        program.define_function(targets.resume_close, build_resume_close(imports)?.finish()?)?;
        program.define_function(targets.cancel_close, build_cancel_close(imports)?.finish()?)?;
        program.define_function(
            targets.consume_close,
            build_consume_close(imports)?.finish()?,
        )?;
        program.define_function(targets.close_worker, build_close_worker(imports)?.finish()?)?;
        Ok(targets)
    }

    #[must_use]
    pub const fn reserve(self) -> Arm64FunctionId {
        self.reserve
    }

    #[must_use]
    pub const fn abandon_reservation(self) -> Arm64FunctionId {
        self.abandon_reservation
    }

    #[must_use]
    pub const fn publish_owner(self) -> Arm64FunctionId {
        self.publish_owner
    }

    #[must_use]
    pub const fn drop_owner(self) -> Arm64FunctionId {
        self.drop_owner
    }

    #[must_use]
    pub const fn begin_close(self) -> Arm64FunctionId {
        self.begin_close
    }

    #[must_use]
    pub const fn resume_close(self) -> Arm64FunctionId {
        self.resume_close
    }

    #[must_use]
    pub const fn cancel_close(self) -> Arm64FunctionId {
        self.cancel_close
    }

    #[must_use]
    pub const fn consume_close(self) -> Arm64FunctionId {
        self.consume_close
    }

    #[must_use]
    pub const fn close_worker(self) -> Arm64FunctionId {
        self.close_worker
    }
}

fn build_reserve() -> Result<Arm64CodeBuilder, Arm64DarwinFileRetirementError> {
    let service_schema = DarwinFileServiceAbiSchema::ARM64_DARWIN;
    let record_schema = service_schema.retirement_record();
    let first = service_schema
        .retirement_record_offset(0)
        .ok_or(Arm64DarwinFileRetirementError::ContractLayout)?;
    let count = u64::try_from(service_schema.retirement_record_count())
        .map_err(|_| Arm64DarwinFileRetirementError::ContractLayout)?;
    if count == 0 {
        return Err(Arm64DarwinFileRetirementError::ContractLayout);
    }

    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code);
    move_register(&mut code, x(19), x(0));
    move_register(&mut code, x(25), x(1));
    address(
        &mut code,
        x(20),
        x(19),
        service_schema.offset(DarwinFileServiceField::AdmissionState),
    );
    code.append(Arm64Instruction::LoadAcquire {
        size: Arm64DataSize::Bits64,
        destination: Arm64DataRegister::General(x(21)),
        base: Arm64BaseRegister::General(x(20)),
    });
    let closed = code.create_label();
    compare_immediate(&mut code, x(21), DarwinFileServiceState::Accepting.code());
    code.branch_conditional(closed, Arm64BranchCondition::NotEqual);

    move_register(&mut code, x(20), x(19));
    crate::address_code::add_offset(&mut code, x(20), first);
    immediate(&mut code, x(21), count);
    let scan = code.create_label();
    let reserved = code.create_label();
    let occupied = code.create_label();
    code.bind(scan)?;
    address(
        &mut code,
        x(22),
        x(20),
        record_schema.offset(DarwinFileRetirementField::LifecycleState),
    );
    let action = crate::emit_darwin_file_retirement_transition(
        &mut code,
        atomic_registers(x(22), x(23), x(24)),
        DarwinFileRetirementState::Available,
        DarwinFileRetirementEvent::Reserve,
        reserved,
        occupied,
    )?;
    require_action(action, DarwinFileRetirementAction::RetainReservation)?;
    code.bind(occupied)?;
    crate::address_code::add_offset(&mut code, x(20), record_schema.size());
    subtract_immediate(&mut code, x(21), 1);
    compare_immediate(&mut code, x(21), 0);
    code.branch_conditional(scan, Arm64BranchCondition::NotEqual);
    immediate(&mut code, x(0), 0);
    immediate(
        &mut code,
        x(1),
        DarwinFileServiceAdmission::Saturated.code(),
    );
    epilogue(&mut code);

    code.bind(closed)?;
    immediate(&mut code, x(0), 0);
    immediate(&mut code, x(1), DarwinFileServiceAdmission::Closed.code());
    epilogue(&mut code);

    code.bind(reserved)?;
    reset_record_transient(&mut code, x(20), &record_schema);
    store(
        &mut code,
        x(20),
        record_schema.offset(DarwinFileRetirementField::AllocationContext),
        x(25),
    );
    move_register(&mut code, x(0), x(20));
    immediate(&mut code, x(1), DarwinFileServiceAdmission::Ready.code());
    epilogue(&mut code);
    Ok(code)
}

fn build_abandon_reservation(
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<Arm64CodeBuilder, Arm64DarwinFileRetirementError> {
    let record = DarwinFileRetirementAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code);
    move_register(&mut code, x(19), x(0));
    reset_record_transient(&mut code, x(19), &record);
    transition_exact(
        &mut code,
        x(19),
        &record,
        DarwinFileRetirementState::Reserved,
        DarwinFileRetirementEvent::AbandonReservation,
        DarwinFileRetirementAction::ReleaseReservation,
        imports,
    )?;
    signal_record(&mut code, x(19));
    epilogue(&mut code);
    Ok(code)
}

fn build_publish_owner(
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<Arm64CodeBuilder, Arm64DarwinFileRetirementError> {
    let record = DarwinFileRetirementAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code);
    move_register(&mut code, x(19), x(0));
    move_register(&mut code, x(20), x(1));
    store(
        &mut code,
        x(19),
        record.offset(DarwinFileRetirementField::Descriptor),
        x(20),
    );
    transition_exact(
        &mut code,
        x(19),
        &record,
        DarwinFileRetirementState::Reserved,
        DarwinFileRetirementEvent::PublishOwner,
        DarwinFileRetirementAction::PublishOwner,
        imports,
    )?;
    move_register(&mut code, x(0), x(19));
    epilogue(&mut code);
    Ok(code)
}

fn build_begin_retirement(
    worker: Arm64FunctionId,
    attached: bool,
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<Arm64CodeBuilder, Arm64DarwinFileRetirementError> {
    let record = DarwinFileRetirementAbiSchema::ARM64_DARWIN;
    let (event, action) = if attached {
        (
            DarwinFileRetirementEvent::BeginClose,
            DarwinFileRetirementAction::EnqueueAttachedClose,
        )
    } else {
        (
            DarwinFileRetirementEvent::DropOwner,
            DarwinFileRetirementAction::EnqueueDetachedClose,
        )
    };
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code);
    move_register(&mut code, x(19), x(0));
    immediate(&mut code, x(8), 0);
    store(
        &mut code,
        x(19),
        record.offset(DarwinFileRetirementField::Readiness),
        x(8),
    );
    transition_exact(
        &mut code,
        x(19),
        &record,
        DarwinFileRetirementState::Live,
        event,
        action,
        imports,
    )?;
    enqueue_retirement(&mut code, x(19), worker, imports);
    if attached {
        move_register(&mut code, x(0), x(19));
    }
    epilogue(&mut code);
    Ok(code)
}

fn build_resume_close(
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<Arm64CodeBuilder, Arm64DarwinFileRetirementError> {
    let record = DarwinFileRetirementAbiSchema::ARM64_DARWIN;
    let asynchronous = record.asynchronous();
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code);
    move_register(&mut code, x(19), x(0));
    load_state(&mut code, x(20), x(19), &record);
    let pending = code.create_label();
    let completed = code.create_label();
    compare_immediate(
        &mut code,
        x(20),
        DarwinFileRetirementState::ClosingAttached.code(),
    );
    code.branch_conditional(pending, Arm64BranchCondition::Equal);
    compare_immediate(
        &mut code,
        x(20),
        DarwinFileRetirementState::Completed.code(),
    );
    code.branch_conditional(completed, Arm64BranchCondition::Equal);
    abort(&mut code, imports);

    code.bind(pending)?;
    immediate(&mut code, x(0), asynchronous.pending_status());
    move_register(&mut code, x(1), x(19));
    crate::address_code::add_offset(
        &mut code,
        x(1),
        record.offset(DarwinFileRetirementField::InterestKind),
    );
    immediate(&mut code, x(2), 1);
    epilogue(&mut code);

    code.bind(completed)?;
    immediate(&mut code, x(0), asynchronous.completed_status());
    immediate(&mut code, x(1), 0);
    immediate(&mut code, x(2), 0);
    epilogue(&mut code);
    Ok(code)
}

fn build_cancel_close(
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<Arm64CodeBuilder, Arm64DarwinFileRetirementError> {
    let record = DarwinFileRetirementAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code);
    move_register(&mut code, x(19), x(0));
    let inspect = code.create_label();
    let detach = code.create_label();
    let release = code.create_label();
    code.bind(inspect)?;
    load_state(&mut code, x(20), x(19), &record);
    compare_immediate(
        &mut code,
        x(20),
        DarwinFileRetirementState::ClosingAttached.code(),
    );
    code.branch_conditional(detach, Arm64BranchCondition::Equal);
    compare_immediate(
        &mut code,
        x(20),
        DarwinFileRetirementState::Completed.code(),
    );
    code.branch_conditional(release, Arm64BranchCondition::Equal);
    abort(&mut code, imports);

    code.bind(detach)?;
    transition_retry(
        &mut code,
        x(19),
        &record,
        DarwinFileRetirementState::ClosingAttached,
        DarwinFileRetirementEvent::Cancel,
        DarwinFileRetirementAction::DetachClose,
        inspect,
    )?;
    epilogue(&mut code);

    code.bind(release)?;
    reset_record_transient(&mut code, x(19), &record);
    transition_exact(
        &mut code,
        x(19),
        &record,
        DarwinFileRetirementState::Completed,
        DarwinFileRetirementEvent::Cancel,
        DarwinFileRetirementAction::ReleaseCloseCompletion,
        imports,
    )?;
    signal_record(&mut code, x(19));
    epilogue(&mut code);
    Ok(code)
}

fn build_consume_close(
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<Arm64CodeBuilder, Arm64DarwinFileRetirementError> {
    let record = DarwinFileRetirementAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code);
    move_register(&mut code, x(19), x(0));
    move_register(&mut code, x(26), x(1));
    load(
        &mut code,
        x(25),
        x(19),
        record.offset(DarwinFileRetirementField::FailureKind),
    );
    load(
        &mut code,
        x(21),
        x(19),
        record.offset(DarwinFileRetirementField::FailureErrno),
    );
    reset_record_transient(&mut code, x(19), &record);
    transition_exact(
        &mut code,
        x(19),
        &record,
        DarwinFileRetirementState::Completed,
        DarwinFileRetirementEvent::Consume,
        DarwinFileRetirementAction::ConsumeCloseCompletion,
        imports,
    )?;
    signal_record(&mut code, x(19));
    let completion = DarwinFileCompletionAbiSchema::ARM64_DARWIN;
    immediate(&mut code, x(8), 0);
    for field in DarwinFileCompletionField::ALL.iter().copied() {
        store(&mut code, x(26), completion.offset(field), x(8));
    }
    store(
        &mut code,
        x(26),
        completion.offset(DarwinFileCompletionField::FailureKind),
        x(25),
    );
    store(
        &mut code,
        x(26),
        completion.offset(DarwinFileCompletionField::FailureErrno),
        x(21),
    );
    epilogue(&mut code);
    Ok(code)
}

fn build_close_worker(
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<Arm64CodeBuilder, Arm64DarwinFileRetirementError> {
    let record = DarwinFileRetirementAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code);
    move_register(&mut code, x(19), x(0));
    load(
        &mut code,
        x(0),
        x(19),
        record.offset(DarwinFileRetirementField::Descriptor),
    );
    emit_close_result(&mut code, x(19), &record)?;
    publish_close(&mut code, x(19), &record, imports)?;
    signal_record(&mut code, x(19));
    load(
        &mut code,
        x(20),
        x(19),
        record.offset(DarwinFileRetirementField::Service),
    );
    load(
        &mut code,
        x(0),
        x(20),
        DarwinFileServiceAbiSchema::ARM64_DARWIN.offset(DarwinFileServiceField::WorkerGroup),
    );
    call_import(
        &mut code,
        imports.function(DarwinFileServiceFunction::DispatchGroupLeave),
    );
    epilogue(&mut code);
    Ok(code)
}

fn emit_close_result(
    code: &mut Arm64CodeBuilder,
    record: Arm64Register,
    schema: &DarwinFileRetirementAbiSchema,
) -> Result<(), Arm64DarwinFileRetirementError> {
    emit_system_call(code, DarwinSystemCall::Close);
    let succeeded = code.create_label();
    let result_stored = code.create_label();
    code.branch_conditional(succeeded, Arm64BranchCondition::CarryClear);
    move_register(code, x(20), x(0));
    immediate(code, x(8), DarwinFileFailureKind::Target.code());
    store(
        code,
        record,
        schema.offset(DarwinFileRetirementField::FailureKind),
        x(8),
    );
    store(
        code,
        record,
        schema.offset(DarwinFileRetirementField::FailureErrno),
        x(20),
    );
    code.branch(result_stored, false);
    code.bind(succeeded)?;
    immediate(code, x(8), 0);
    store(
        code,
        record,
        schema.offset(DarwinFileRetirementField::FailureKind),
        x(8),
    );
    store(
        code,
        record,
        schema.offset(DarwinFileRetirementField::FailureErrno),
        x(8),
    );
    code.bind(result_stored)?;
    immediate(code, x(8), 0);
    store(
        code,
        record,
        schema.offset(DarwinFileRetirementField::Descriptor),
        x(8),
    );
    Ok(())
}

fn publish_close(
    code: &mut Arm64CodeBuilder,
    record: Arm64Register,
    schema: &DarwinFileRetirementAbiSchema,
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<(), Arm64DarwinFileRetirementError> {
    let inspect = code.create_label();
    let attached = code.create_label();
    let detached = code.create_label();
    let published = code.create_label();
    code.bind(inspect)?;
    load_state(code, x(20), record, schema);
    compare_immediate(
        code,
        x(20),
        DarwinFileRetirementState::ClosingAttached.code(),
    );
    code.branch_conditional(attached, Arm64BranchCondition::Equal);
    compare_immediate(
        code,
        x(20),
        DarwinFileRetirementState::ClosingDetached.code(),
    );
    code.branch_conditional(detached, Arm64BranchCondition::Equal);
    abort(code, imports);

    code.bind(attached)?;
    transition_retry_to(
        code,
        record,
        schema,
        DarwinFileRetirementState::ClosingAttached,
        DarwinFileRetirementEvent::PublishClose,
        DarwinFileRetirementAction::RetainCloseCompletion,
        inspect,
        published,
    )?;

    code.bind(detached)?;
    reset_record_transient(code, record, schema);
    transition_retry_to(
        code,
        record,
        schema,
        DarwinFileRetirementState::ClosingDetached,
        DarwinFileRetirementEvent::PublishClose,
        DarwinFileRetirementAction::ReleaseDetachedClose,
        inspect,
        published,
    )?;

    code.bind(published)?;
    Ok(())
}

fn enqueue_retirement(
    code: &mut Arm64CodeBuilder,
    record: Arm64Register,
    worker: Arm64FunctionId,
    imports: &crate::Arm64DarwinFileServiceImports,
) {
    let record_schema = DarwinFileRetirementAbiSchema::ARM64_DARWIN;
    let service_schema = DarwinFileServiceAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(20),
        record,
        record_schema.offset(DarwinFileRetirementField::Service),
    );
    load(
        code,
        x(0),
        x(20),
        service_schema.offset(DarwinFileServiceField::WorkerGroup),
    );
    call_import(
        code,
        imports.function(DarwinFileServiceFunction::DispatchGroupEnter),
    );
    load(
        code,
        x(0),
        x(20),
        service_schema.offset(DarwinFileServiceField::RetirementQueue),
    );
    move_register(code, x(1), record);
    code.load_function_address(worker, x(2));
    call_import(
        code,
        imports.function(DarwinFileServiceFunction::DispatchAsyncFunction),
    );
}

fn transition_exact(
    code: &mut Arm64CodeBuilder,
    record: Arm64Register,
    schema: &DarwinFileRetirementAbiSchema,
    state: DarwinFileRetirementState,
    event: DarwinFileRetirementEvent,
    expected_action: DarwinFileRetirementAction,
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<(), Arm64DarwinFileRetirementError> {
    address(
        code,
        x(22),
        record,
        schema.offset(DarwinFileRetirementField::LifecycleState),
    );
    let success = code.create_label();
    let mismatch = code.create_label();
    let action = crate::emit_darwin_file_retirement_transition(
        code,
        atomic_registers(x(22), x(23), x(24)),
        state,
        event,
        success,
        mismatch,
    )?;
    require_action(action, expected_action)?;
    code.bind(mismatch)?;
    abort(code, imports);
    code.bind(success)?;
    Ok(())
}

fn transition_retry(
    code: &mut Arm64CodeBuilder,
    record: Arm64Register,
    schema: &DarwinFileRetirementAbiSchema,
    state: DarwinFileRetirementState,
    event: DarwinFileRetirementEvent,
    expected_action: DarwinFileRetirementAction,
    retry: crate::Arm64LabelId,
) -> Result<(), Arm64DarwinFileRetirementError> {
    let success = code.create_label();
    transition_retry_to(
        code,
        record,
        schema,
        state,
        event,
        expected_action,
        retry,
        success,
    )?;
    code.bind(success)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn transition_retry_to(
    code: &mut Arm64CodeBuilder,
    record: Arm64Register,
    schema: &DarwinFileRetirementAbiSchema,
    state: DarwinFileRetirementState,
    event: DarwinFileRetirementEvent,
    expected_action: DarwinFileRetirementAction,
    retry: crate::Arm64LabelId,
    success: crate::Arm64LabelId,
) -> Result<(), Arm64DarwinFileRetirementError> {
    address(
        code,
        x(22),
        record,
        schema.offset(DarwinFileRetirementField::LifecycleState),
    );
    let action = crate::emit_darwin_file_retirement_transition(
        code,
        atomic_registers(x(22), x(23), x(24)),
        state,
        event,
        success,
        retry,
    )?;
    require_action(action, expected_action)
}

fn require_action(
    actual: DarwinFileRetirementAction,
    expected: DarwinFileRetirementAction,
) -> Result<(), Arm64DarwinFileRetirementError> {
    if actual == expected {
        Ok(())
    } else {
        Err(Arm64DarwinFileRetirementError::LifecycleAction)
    }
}

fn reset_record_transient(
    code: &mut Arm64CodeBuilder,
    record: Arm64Register,
    schema: &DarwinFileRetirementAbiSchema,
) {
    immediate(code, x(8), 0);
    for field in [
        DarwinFileRetirementField::AllocationContext,
        DarwinFileRetirementField::Descriptor,
        DarwinFileRetirementField::FailureKind,
        DarwinFileRetirementField::FailureErrno,
        DarwinFileRetirementField::Readiness,
    ] {
        store(code, record, schema.offset(field), x(8));
    }
}

fn load_state(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    record: Arm64Register,
    schema: &DarwinFileRetirementAbiSchema,
) {
    address(
        code,
        x(22),
        record,
        schema.offset(DarwinFileRetirementField::LifecycleState),
    );
    code.append(Arm64Instruction::LoadAcquire {
        size: Arm64DataSize::Bits64,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(x(22)),
    });
}

fn signal_record(code: &mut Arm64CodeBuilder, record: Arm64Register) {
    let record_schema = DarwinFileRetirementAbiSchema::ARM64_DARWIN;
    let service_schema = DarwinFileServiceAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(20),
        record,
        record_schema.offset(DarwinFileRetirementField::Service),
    );
    load(
        code,
        x(0),
        x(20),
        service_schema.offset(DarwinFileServiceField::NotificationWriter),
    );
    immediate(code, x(8), 1);
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Byte,
        x(8),
        NOTIFICATION_BYTE_OFFSET,
    );
    crate::frame_access::form_stack_address(code, x(1), NOTIFICATION_BYTE_OFFSET);
    immediate(code, x(2), 1);
    emit_system_call(code, DarwinSystemCall::Write);
}

fn prologue(code: &mut Arm64CodeBuilder) {
    crate::frame_access::adjust_stack(code, STACK_SIZE, Arm64AddSubtract::Subtract);
    for (register, offset) in [
        (19, 16),
        (20, 24),
        (21, 32),
        (22, 40),
        (23, 48),
        (24, 56),
        (25, 64),
        (26, 72),
    ] {
        crate::frame_access::store_at_stack_offset(
            code,
            Arm64LoadStoreSize::Double,
            x(register),
            offset,
        );
    }
    crate::frame_access::store_at_stack_offset(code, Arm64LoadStoreSize::Double, x(30), 8);
}

fn epilogue(code: &mut Arm64CodeBuilder) {
    for (register, offset) in [
        (19, 16),
        (20, 24),
        (21, 32),
        (22, 40),
        (23, 48),
        (24, 56),
        (25, 64),
        (26, 72),
    ] {
        crate::frame_access::load_at_stack_offset(
            code,
            Arm64LoadStoreSize::Double,
            x(register),
            offset,
        );
    }
    crate::frame_access::load_at_stack_offset(code, Arm64LoadStoreSize::Double, x(30), 8);
    crate::frame_access::adjust_stack(code, STACK_SIZE, Arm64AddSubtract::Add);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
}

fn address(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    base: Arm64Register,
    offset: u64,
) {
    move_register(code, destination, base);
    crate::address_code::add_offset(code, destination, offset);
}

fn load(code: &mut Arm64CodeBuilder, destination: Arm64Register, base: Arm64Register, offset: u64) {
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        destination,
        base,
        offset,
    );
}

fn store(code: &mut Arm64CodeBuilder, base: Arm64Register, offset: u64, source: Arm64Register) {
    crate::address_code::store_native(code, Arm64LoadStoreSize::Double, source, base, offset);
}

fn move_register(code: &mut Arm64CodeBuilder, destination: Arm64Register, source: Arm64Register) {
    crate::address_code::move_register(code, source, destination);
}

fn immediate(code: &mut Arm64CodeBuilder, destination: Arm64Register, value: u64) {
    crate::frame_access::load_immediate(code, destination, value, Arm64DataSize::Bits64);
}

fn subtract_immediate(code: &mut Arm64CodeBuilder, register: Arm64Register, value: u16) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(register),
        source: Arm64BaseRegister::General(register),
        immediate: value,
        shift_12: false,
    });
}

fn compare_immediate(code: &mut Arm64CodeBuilder, value: Arm64Register, expected: u64) {
    let expected_register = if value == x(8) { x(9) } else { x(8) };
    immediate(code, expected_register, expected);
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(value),
        right: Arm64DataRegister::General(expected_register),
    });
}

fn atomic_registers(
    address: Arm64Register,
    observed: Arm64Register,
    status: Arm64Register,
) -> Arm64AtomicUpdateRegisters {
    Arm64AtomicUpdateRegisters::new(address, observed, status)
        .expect("closed retirement registers do not alias")
}

fn call_import(code: &mut Arm64CodeBuilder, target: crate::Arm64FunctionImportId) {
    code.load_function_import(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
}

fn abort(code: &mut Arm64CodeBuilder, imports: &crate::Arm64DarwinFileServiceImports) {
    call_import(code, imports.function(DarwinFileServiceFunction::Abort));
}

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).expect("closed ARM64 register is valid")
}

#[derive(Debug)]
pub enum Arm64DarwinFileRetirementError {
    ContractLayout,
    LifecycleAction,
    Lifecycle(crate::Arm64DarwinFileLifecycleError),
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinFileRetirementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin file retirement failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinFileRetirementError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lifecycle(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ContractLayout | Self::LifecycleAction => None,
        }
    }
}

impl From<crate::Arm64DarwinFileLifecycleError> for Arm64DarwinFileRetirementError {
    fn from(error: crate::Arm64DarwinFileLifecycleError) -> Self {
        Self::Lifecycle(error)
    }
}

impl From<Arm64CodeError> for Arm64DarwinFileRetirementError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinFileRetirementError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}

#[cfg(test)]
mod tests {
    use super::Arm64DarwinFileRetirementTargets;
    use crate::{Arm64DarwinFileServiceImports, Arm64ProgramBuilder};

    #[test]
    fn retirement_targets_are_declared_and_defined_as_one_family() {
        let mut program = Arm64ProgramBuilder::new();
        let imports = Arm64DarwinFileServiceImports::declare(&mut program).unwrap();
        let targets = Arm64DarwinFileRetirementTargets::declare(&mut program, &imports).unwrap();
        program.set_entry(targets.reserve()).unwrap();
        let program = program.finish().unwrap();
        for target in [
            targets.reserve(),
            targets.abandon_reservation(),
            targets.publish_owner(),
            targets.drop_owner(),
            targets.begin_close(),
            targets.resume_close(),
            targets.cancel_close(),
            targets.consume_close(),
            targets.close_worker(),
        ] {
            assert!(program.function(target).unwrap().size() > 0);
        }
    }
}
