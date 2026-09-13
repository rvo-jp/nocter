use nocter_runtime_contract::{
    DarwinFileAccess, DarwinFileCompletionAbiSchema, DarwinFileCompletionField,
    DarwinFileFailureKind, DarwinFileJobAbiSchema, DarwinFileJobAction, DarwinFileJobEvent,
    DarwinFileJobField, DarwinFileJobState, DarwinFileOperation, DarwinFileRetirementAbiSchema,
    DarwinFileRetirementField, DarwinFileRetirementState, DarwinFileSeekOrigin,
    DarwinFileServiceAbiSchema, DarwinFileServiceAdmission, DarwinFileServiceConfiguration,
    DarwinFileServiceField, DarwinFileServiceFunction, DarwinFileServiceState,
};

use crate::darwin_file_job_code::{
    abort, add_immediate, add_register, address, atomic_registers, call_import, compare_immediate,
    compare_register, completed, epilogue, immediate, load, move_register, pending, prologue,
    signal, signal_descriptor, store, x,
};
use crate::darwin_kernel_abi::{DarwinErrorAbi, DarwinFileAbi, DarwinSystemCall, emit_system_call};
use crate::{
    Arm64AddSubtract, Arm64BaseRegister, Arm64BranchCondition, Arm64CodeBuilder, Arm64DataRegister,
    Arm64DataSize, Arm64Instruction,
};

pub(crate) fn build_resume(
    targets: crate::Arm64DarwinFileJobTargets,
    imports: &crate::Arm64DarwinFileServiceImports,
    retirement: crate::Arm64DarwinFileRetirementTargets,
) -> Result<Arm64CodeBuilder, crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code);
    move_register(&mut code, x(19), x(0));
    let inspect = code.create_label();
    let prepared = code.create_label();
    let running = code.create_label();
    let done = code.create_label();
    code.bind(inspect)?;
    load_job_state(&mut code, x(20), x(19), &schema);
    compare_immediate(&mut code, x(20), DarwinFileJobState::Prepared.code());
    code.branch_conditional(prepared, Arm64BranchCondition::Equal);
    compare_immediate(&mut code, x(20), DarwinFileJobState::RunningAttached.code());
    code.branch_conditional(running, Arm64BranchCondition::Equal);
    compare_immediate(&mut code, x(20), DarwinFileJobState::Completed.code());
    code.branch_conditional(done, Arm64BranchCondition::Equal);
    abort(&mut code, imports);

    code.bind(prepared)?;
    ensure_open_reservation(&mut code, x(19), imports, retirement)?;
    let rejected = code.create_label();
    check_service_accepting(&mut code, x(19), rejected);
    let admitted = code.create_label();
    let saturated = code.create_label();
    acquire_capacity(&mut code, x(19), admitted, saturated)?;
    code.bind(saturated)?;
    pending(&mut code, x(19));

    code.bind(rejected)?;
    store_failure(&mut code, x(19), DarwinFileFailureKind::ServiceClosed, 0);
    cleanup_failed_open_reservation(&mut code, x(19), retirement)?;
    transition_exact(
        &mut code,
        x(19),
        DarwinFileJobState::Prepared,
        DarwinFileJobEvent::Reject,
        DarwinFileJobAction::RetainRejection,
        imports,
    )?;
    completed(&mut code);

    code.bind(admitted)?;
    store_immediate(
        &mut code,
        x(19),
        &schema,
        DarwinFileJobField::CapacityHeld,
        1,
    );
    transition_exact(
        &mut code,
        x(19),
        DarwinFileJobState::Prepared,
        DarwinFileJobEvent::Admit,
        DarwinFileJobAction::Dispatch,
        imports,
    )?;
    dispatch_job(&mut code, x(19), targets.worker(), imports)?;
    code.bind(running)?;
    pending(&mut code, x(19));
    code.bind(done)?;
    completed(&mut code);
    Ok(code)
}

pub(crate) fn build_cancel(
    imports: &crate::Arm64DarwinFileServiceImports,
    retirement: crate::Arm64DarwinFileRetirementTargets,
) -> Result<Arm64CodeBuilder, crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code);
    move_register(&mut code, x(19), x(0));
    let inspect = code.create_label();
    let prepared = code.create_label();
    let running = code.create_label();
    let completed_state = code.create_label();
    code.bind(inspect)?;
    load_job_state(&mut code, x(20), x(19), &schema);
    compare_immediate(&mut code, x(20), DarwinFileJobState::Prepared.code());
    code.branch_conditional(prepared, Arm64BranchCondition::Equal);
    compare_immediate(&mut code, x(20), DarwinFileJobState::RunningAttached.code());
    code.branch_conditional(running, Arm64BranchCondition::Equal);
    compare_immediate(&mut code, x(20), DarwinFileJobState::Completed.code());
    code.branch_conditional(completed_state, Arm64BranchCondition::Equal);
    abort(&mut code, imports);

    code.bind(prepared)?;
    let prepared_released = code.create_label();
    transition_retry_to(
        &mut code,
        x(19),
        DarwinFileJobState::Prepared,
        DarwinFileJobEvent::Cancel,
        DarwinFileJobAction::ReleasePrepared,
        inspect,
        prepared_released,
    )?;
    code.bind(prepared_released)?;
    cleanup_retirement(&mut code, x(19), imports, retirement)?;
    release_job(&mut code, x(19))?;

    code.bind(running)?;
    immediate(&mut code, x(8), 0);
    store(
        &mut code,
        x(19),
        schema.offset(DarwinFileJobField::ConsumerBytePointer),
        x(8),
    );
    let detached = code.create_label();
    transition_retry_to(
        &mut code,
        x(19),
        DarwinFileJobState::RunningAttached,
        DarwinFileJobEvent::Cancel,
        DarwinFileJobAction::Detach,
        inspect,
        detached,
    )?;
    code.bind(detached)?;
    epilogue(&mut code);

    code.bind(completed_state)?;
    let completion_released = code.create_label();
    transition_retry_to(
        &mut code,
        x(19),
        DarwinFileJobState::Completed,
        DarwinFileJobEvent::Cancel,
        DarwinFileJobAction::ReleaseCompletion,
        inspect,
        completion_released,
    )?;
    code.bind(completion_released)?;
    cleanup_retirement(&mut code, x(19), imports, retirement)?;
    release_capacity_if_held(&mut code, x(19), imports)?;
    release_job(&mut code, x(19))?;
    Ok(code)
}

pub(crate) fn build_consume(
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<Arm64CodeBuilder, crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let completion = DarwinFileCompletionAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code);
    move_register(&mut code, x(19), x(0));
    move_register(&mut code, x(26), x(1));
    transition_exact(
        &mut code,
        x(19),
        DarwinFileJobState::Completed,
        DarwinFileJobEvent::Consume,
        DarwinFileJobAction::ConsumeCompletion,
        imports,
    )?;
    copy_read_output(&mut code, x(19), imports, &schema);
    for (source, destination) in [
        (
            DarwinFileJobField::RetirementRecord,
            DarwinFileCompletionField::RetirementRecord,
        ),
        (
            DarwinFileJobField::TransferredByteCount,
            DarwinFileCompletionField::TransferredByteCount,
        ),
        (
            DarwinFileJobField::ResultPosition,
            DarwinFileCompletionField::ResultPosition,
        ),
        (
            DarwinFileJobField::FailureKind,
            DarwinFileCompletionField::FailureKind,
        ),
        (
            DarwinFileJobField::FailureErrno,
            DarwinFileCompletionField::FailureErrno,
        ),
    ] {
        load(&mut code, x(8), x(19), schema.offset(source));
        store(&mut code, x(26), completion.offset(destination), x(8));
    }
    release_capacity_if_held(&mut code, x(19), imports)?;
    release_job(&mut code, x(19))?;
    Ok(code)
}

pub(crate) fn build_worker(
    imports: &crate::Arm64DarwinFileServiceImports,
    retirement: crate::Arm64DarwinFileRetirementTargets,
) -> Result<Arm64CodeBuilder, crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code);
    move_register(&mut code, x(19), x(0));
    load(
        &mut code,
        x(20),
        x(19),
        schema.offset(DarwinFileJobField::Operation),
    );
    let publish = code.create_label();
    let branches = DarwinFileOperation::ALL
        .iter()
        .copied()
        .map(|operation| (operation, code.create_label()))
        .collect::<Vec<_>>();
    for (operation, label) in branches.iter().copied() {
        compare_immediate(&mut code, x(20), u64::from(operation.code()));
        code.branch_conditional(label, Arm64BranchCondition::Equal);
    }
    abort(&mut code, imports);
    for (operation, label) in branches {
        code.bind(label)?;
        execute_operation(&mut code, x(19), operation, imports, retirement)?;
        code.branch(publish, false);
    }
    code.bind(publish)?;
    retain_worker_resources(&mut code, x(19));
    publish_worker_result(&mut code, x(19), x(27), x(28), imports, retirement)?;
    epilogue(&mut code);
    Ok(code)
}

fn ensure_open_reservation(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    imports: &crate::Arm64DarwinFileServiceImports,
    retirement: crate::Arm64DarwinFileRetirementTargets,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(20),
        job,
        schema.offset(DarwinFileJobField::Operation),
    );
    let ready = code.create_label();
    compare_immediate(code, x(20), u64::from(DarwinFileOperation::Open.code()));
    code.branch_conditional(ready, Arm64BranchCondition::NotEqual);
    load(
        code,
        x(20),
        job,
        schema.offset(DarwinFileJobField::RetirementRecord),
    );
    compare_immediate(code, x(20), 0);
    code.branch_conditional(ready, Arm64BranchCondition::NotEqual);
    load(code, x(0), job, schema.offset(DarwinFileJobField::Service));
    load(
        code,
        x(1),
        job,
        schema.offset(DarwinFileJobField::AllocationContext),
    );
    code.call(retirement.reserve());
    compare_immediate(code, x(1), DarwinFileServiceAdmission::Ready.code());
    let reserved = code.create_label();
    code.branch_conditional(reserved, Arm64BranchCondition::Equal);
    compare_immediate(code, x(1), DarwinFileServiceAdmission::Saturated.code());
    let closed = code.create_label();
    code.branch_conditional(closed, Arm64BranchCondition::NotEqual);
    pending(code, job);
    code.bind(closed)?;
    store_failure(code, job, DarwinFileFailureKind::ServiceClosed, 0);
    transition_exact(
        code,
        job,
        DarwinFileJobState::Prepared,
        DarwinFileJobEvent::Reject,
        DarwinFileJobAction::RetainRejection,
        imports,
    )?;
    completed(code);
    code.bind(reserved)?;
    store(
        code,
        job,
        schema.offset(DarwinFileJobField::RetirementRecord),
        x(0),
    );
    code.bind(ready)?;
    Ok(())
}

fn check_service_accepting(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    rejected: crate::Arm64LabelId,
) {
    let job_schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let service_schema = DarwinFileServiceAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(20),
        job,
        job_schema.offset(DarwinFileJobField::Service),
    );
    address(
        code,
        x(21),
        x(20),
        service_schema.offset(DarwinFileServiceField::AdmissionState),
    );
    code.append(Arm64Instruction::LoadAcquire {
        size: Arm64DataSize::Bits64,
        destination: Arm64DataRegister::General(x(22)),
        base: Arm64BaseRegister::General(x(21)),
    });
    compare_immediate(code, x(22), DarwinFileServiceState::Accepting.code());
    code.branch_conditional(rejected, Arm64BranchCondition::NotEqual);
}

fn acquire_capacity(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    admitted: crate::Arm64LabelId,
    saturated: crate::Arm64LabelId,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let job_schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let service_schema = DarwinFileServiceAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(20),
        job,
        job_schema.offset(DarwinFileJobField::Service),
    );
    address(
        code,
        x(22),
        x(20),
        service_schema.offset(DarwinFileServiceField::ActiveOperationCount),
    );
    code.append_atomic_bounded_increment(
        atomic_registers(x(22), x(23), x(24)),
        u16::try_from(DarwinFileServiceConfiguration::ARM64_DARWIN.maximum_operations())
            .map_err(|_| crate::Arm64DarwinFileJobError::ContractLayout)?,
        admitted,
        saturated,
    )?;
    Ok(())
}

fn dispatch_job(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    worker: crate::Arm64FunctionId,
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let job_schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let service_schema = DarwinFileServiceAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(20),
        job,
        job_schema.offset(DarwinFileJobField::Service),
    );
    load(
        code,
        x(21),
        x(20),
        service_schema.offset(DarwinFileServiceField::NextOperationQueue),
    );
    let queue_labels = (0..DarwinFileServiceField::OPERATION_QUEUES.len())
        .map(|_| code.create_label())
        .collect::<Vec<_>>();
    let selected = code.create_label();
    for (index, label) in queue_labels.iter().copied().enumerate() {
        compare_immediate(code, x(21), index as u64);
        code.branch_conditional(label, Arm64BranchCondition::Equal);
    }
    abort(code, imports);
    for (index, label) in queue_labels.into_iter().enumerate() {
        code.bind(label)?;
        load(
            code,
            x(25),
            x(20),
            service_schema
                .operation_queue_offset(index)
                .ok_or(crate::Arm64DarwinFileJobError::ContractLayout)?,
        );
        code.branch(selected, false);
    }
    code.bind(selected)?;
    add_immediate(code, x(21), x(21), 1);
    compare_immediate(
        code,
        x(21),
        DarwinFileServiceField::OPERATION_QUEUES.len() as u64,
    );
    let store_index = code.create_label();
    code.branch_conditional(store_index, Arm64BranchCondition::NotEqual);
    immediate(code, x(21), 0);
    code.bind(store_index)?;
    store(
        code,
        x(20),
        service_schema.offset(DarwinFileServiceField::NextOperationQueue),
        x(21),
    );
    load(
        code,
        x(0),
        x(20),
        service_schema.offset(DarwinFileServiceField::WorkerGroup),
    );
    call_import(code, imports, DarwinFileServiceFunction::DispatchGroupEnter);
    move_register(code, x(0), x(25));
    move_register(code, x(1), job);
    code.load_function_address(worker, x(2));
    call_import(
        code,
        imports,
        DarwinFileServiceFunction::DispatchAsyncFunction,
    );
    Ok(())
}

fn execute_operation(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    operation: DarwinFileOperation,
    imports: &crate::Arm64DarwinFileServiceImports,
    retirement: crate::Arm64DarwinFileRetirementTargets,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    match operation {
        DarwinFileOperation::Open => execute_open(code, job, imports, retirement),
        DarwinFileOperation::Read => execute_read(code, job, false, imports),
        DarwinFileOperation::Write => execute_write(code, job, false, imports),
        DarwinFileOperation::Flush => execute_flush(code, job, imports),
        DarwinFileOperation::Seek => execute_seek(code, job, imports),
        DarwinFileOperation::Truncate => execute_truncate(code, job, imports),
        DarwinFileOperation::ReadAt => execute_read(code, job, true, imports),
        DarwinFileOperation::WriteAt => execute_write(code, job, true, imports),
    }
}

fn execute_open(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    imports: &crate::Arm64DarwinFileServiceImports,
    retirement: crate::Arm64DarwinFileRetirementTargets,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let retry = code.create_label();
    let read = code.create_label();
    let create = code.create_label();
    let append = code.create_label();
    let invoke = code.create_label();
    load(code, x(20), job, schema.offset(DarwinFileJobField::Access));
    compare_immediate(code, x(20), u64::from(DarwinFileAccess::Read.code()));
    code.branch_conditional(read, Arm64BranchCondition::Equal);
    compare_immediate(code, x(20), u64::from(DarwinFileAccess::Create.code()));
    code.branch_conditional(create, Arm64BranchCondition::Equal);
    compare_immediate(code, x(20), u64::from(DarwinFileAccess::Append.code()));
    code.branch_conditional(append, Arm64BranchCondition::Equal);
    abort(code, imports);
    code.bind(read)?;
    immediate(code, x(21), DarwinFileAbi::READ_ONLY);
    code.branch(invoke, false);
    code.bind(create)?;
    immediate(code, x(21), DarwinFileAbi::CREATE_TRUNCATE_WRITE_ONLY);
    code.branch(invoke, false);
    code.bind(append)?;
    immediate(code, x(21), DarwinFileAbi::CREATE_APPEND_WRITE_ONLY);
    code.bind(invoke)?;
    code.bind(retry)?;
    address(code, x(0), job, schema.owned_bytes_offset());
    move_register(code, x(1), x(21));
    immediate(code, x(2), DarwinFileAbi::CREATE_MODE);
    emit_system_call(code, DarwinSystemCall::Open);
    let success = code.create_label();
    code.branch_conditional(success, Arm64BranchCondition::CarryClear);
    retry_interrupted_or_store_target(code, job, retry);
    let finished = code.create_label();
    code.branch(finished, false);
    code.bind(success)?;
    move_register(code, x(21), x(0));
    load(
        code,
        x(0),
        job,
        schema.offset(DarwinFileJobField::RetirementRecord),
    );
    move_register(code, x(1), x(21));
    code.call(retirement.publish_owner());
    code.bind(finished)?;
    Ok(())
}

fn execute_read(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    positioned: bool,
    _imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(21),
        job,
        schema.offset(DarwinFileJobField::OwnedByteLength),
    );
    let valid = code.create_label();
    immediate(code, x(8), DarwinFileAbi::MAXIMUM_TRANSFER);
    compare_register(code, x(21), x(8));
    code.branch_conditional(valid, Arm64BranchCondition::UnsignedLowerOrSame);
    store_failure(code, job, DarwinFileFailureKind::InvalidProgress, 0);
    let finished = code.create_label();
    code.branch(finished, false);
    code.bind(valid)?;
    if positioned {
        load(
            code,
            x(22),
            job,
            schema.offset(DarwinFileJobField::PositionedOffset),
        );
        immediate(code, x(8), DarwinFileAbi::MAXIMUM_OFFSET);
        compare_register(code, x(22), x(8));
        let offset_valid = code.create_label();
        code.branch_conditional(offset_valid, Arm64BranchCondition::UnsignedLowerOrSame);
        store_failure(code, job, DarwinFileFailureKind::OffsetOverflow, 0);
        code.branch(finished, false);
        code.bind(offset_valid)?;
    }
    let retry = code.create_label();
    code.bind(retry)?;
    load_descriptor(code, x(0), job);
    address(code, x(1), job, schema.owned_bytes_offset());
    move_register(code, x(2), x(21));
    if positioned {
        move_register(code, x(3), x(22));
        emit_system_call(code, DarwinSystemCall::PositionedRead);
    } else {
        emit_system_call(code, DarwinSystemCall::Read);
    }
    let success = code.create_label();
    code.branch_conditional(success, Arm64BranchCondition::CarryClear);
    retry_interrupted_or_store_target(code, job, retry);
    code.branch(finished, false);
    code.bind(success)?;
    compare_register(code, x(0), x(21));
    let progress_valid = code.create_label();
    code.branch_conditional(progress_valid, Arm64BranchCondition::UnsignedLowerOrSame);
    store_failure(code, job, DarwinFileFailureKind::InvalidProgress, 0);
    code.branch(finished, false);
    code.bind(progress_valid)?;
    store(
        code,
        job,
        schema.offset(DarwinFileJobField::TransferredByteCount),
        x(0),
    );
    code.bind(finished)?;
    Ok(())
}

fn execute_write(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    positioned: bool,
    _imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(22),
        job,
        schema.offset(DarwinFileJobField::OwnedByteLength),
    );
    immediate(code, x(21), 0);
    let finished = code.create_label();
    immediate(code, x(8), DarwinFileAbi::MAXIMUM_TRANSFER);
    compare_register(code, x(22), x(8));
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::UnsignedLowerOrSame);
    store_failure(code, job, DarwinFileFailureKind::InvalidProgress, 0);
    code.branch(finished, false);
    code.bind(valid)?;
    if positioned {
        load(
            code,
            x(23),
            job,
            schema.offset(DarwinFileJobField::PositionedOffset),
        );
        immediate(code, x(8), DarwinFileAbi::MAXIMUM_OFFSET);
        compare_register(code, x(23), x(8));
        let offset_valid = code.create_label();
        code.branch_conditional(offset_valid, Arm64BranchCondition::UnsignedLowerOrSame);
        store_failure(code, job, DarwinFileFailureKind::OffsetOverflow, 0);
        code.branch(finished, false);
        code.bind(offset_valid)?;
    }
    let loop_ = code.create_label();
    let retry = code.create_label();
    code.bind(loop_)?;
    compare_register(code, x(21), x(22));
    code.branch_conditional(finished, Arm64BranchCondition::Equal);
    code.bind(retry)?;
    load_descriptor(code, x(0), job);
    address(code, x(1), job, schema.owned_bytes_offset());
    add_register(code, x(1), x(1), x(21), false);
    subtract_register(code, x(2), x(22), x(21));
    if positioned {
        add_register(code, x(3), x(23), x(21), true);
        let offset_valid = code.create_label();
        code.branch_conditional(offset_valid, Arm64BranchCondition::CarryClear);
        store_failure(code, job, DarwinFileFailureKind::OffsetOverflow, 0);
        code.branch(finished, false);
        code.bind(offset_valid)?;
        immediate(code, x(8), DarwinFileAbi::MAXIMUM_OFFSET);
        compare_register(code, x(3), x(8));
        let offset_bounded = code.create_label();
        code.branch_conditional(offset_bounded, Arm64BranchCondition::UnsignedLowerOrSame);
        store_failure(code, job, DarwinFileFailureKind::OffsetOverflow, 0);
        code.branch(finished, false);
        code.bind(offset_bounded)?;
        emit_system_call(code, DarwinSystemCall::PositionedWrite);
    } else {
        emit_system_call(code, DarwinSystemCall::Write);
    }
    let progress = code.create_label();
    code.branch_conditional(progress, Arm64BranchCondition::CarryClear);
    retry_interrupted_or_store_target(code, job, retry);
    code.branch(finished, false);
    code.bind(progress)?;
    compare_immediate(code, x(0), 0);
    let nonzero = code.create_label();
    code.branch_conditional(nonzero, Arm64BranchCondition::NotEqual);
    store_failure(code, job, DarwinFileFailureKind::ZeroProgress, 0);
    code.branch(finished, false);
    code.bind(nonzero)?;
    subtract_register(code, x(8), x(22), x(21));
    compare_register(code, x(0), x(8));
    let valid_progress = code.create_label();
    code.branch_conditional(valid_progress, Arm64BranchCondition::UnsignedLowerOrSame);
    store_failure(code, job, DarwinFileFailureKind::InvalidProgress, 0);
    code.branch(finished, false);
    code.bind(valid_progress)?;
    add_register(code, x(21), x(21), x(0), false);
    store(
        code,
        job,
        schema.offset(DarwinFileJobField::TransferredByteCount),
        x(21),
    );
    code.branch(loop_, false);
    code.bind(finished)?;
    Ok(())
}

fn execute_flush(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    _imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let retry = code.create_label();
    code.bind(retry)?;
    load_descriptor(code, x(0), job);
    emit_system_call(code, DarwinSystemCall::SynchronizeFile);
    let success = code.create_label();
    code.branch_conditional(success, Arm64BranchCondition::CarryClear);
    retry_interrupted_or_store_target(code, job, retry);
    code.bind(success)?;
    Ok(())
}

fn execute_seek(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(20),
        job,
        schema.offset(DarwinFileJobField::SeekOrigin),
    );
    let start = code.create_label();
    let end = code.create_label();
    let current = code.create_label();
    let invoke = code.create_label();
    compare_immediate(code, x(20), u64::from(DarwinFileSeekOrigin::Start.code()));
    code.branch_conditional(start, Arm64BranchCondition::Equal);
    compare_immediate(code, x(20), u64::from(DarwinFileSeekOrigin::End.code()));
    code.branch_conditional(end, Arm64BranchCondition::Equal);
    compare_immediate(code, x(20), u64::from(DarwinFileSeekOrigin::Current.code()));
    code.branch_conditional(current, Arm64BranchCondition::Equal);
    abort(code, imports);
    code.bind(start)?;
    immediate(code, x(22), DarwinFileAbi::SEEK_FROM_START);
    code.branch(invoke, false);
    code.bind(end)?;
    immediate(code, x(22), DarwinFileAbi::SEEK_FROM_END);
    code.branch(invoke, false);
    code.bind(current)?;
    immediate(code, x(22), DarwinFileAbi::SEEK_FROM_CURRENT);
    code.bind(invoke)?;
    let retry = code.create_label();
    code.bind(retry)?;
    load_descriptor(code, x(0), job);
    load(
        code,
        x(1),
        job,
        schema.offset(DarwinFileJobField::SeekDisplacement),
    );
    move_register(code, x(2), x(22));
    emit_system_call(code, DarwinSystemCall::Seek);
    let success = code.create_label();
    code.branch_conditional(success, Arm64BranchCondition::CarryClear);
    retry_interrupted_or_store_target(code, job, retry);
    let finished = code.create_label();
    code.branch(finished, false);
    code.bind(success)?;
    store(
        code,
        job,
        schema.offset(DarwinFileJobField::ResultPosition),
        x(0),
    );
    code.bind(finished)?;
    Ok(())
}

fn execute_truncate(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    _imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(21),
        job,
        schema.offset(DarwinFileJobField::TruncateLength),
    );
    immediate(code, x(8), DarwinFileAbi::MAXIMUM_OFFSET);
    compare_register(code, x(21), x(8));
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::UnsignedLowerOrSame);
    store_failure(code, job, DarwinFileFailureKind::OffsetOverflow, 0);
    let finished = code.create_label();
    code.branch(finished, false);
    code.bind(valid)?;
    let retry = code.create_label();
    code.bind(retry)?;
    load_descriptor(code, x(0), job);
    move_register(code, x(1), x(21));
    emit_system_call(code, DarwinSystemCall::TruncateFile);
    let success = code.create_label();
    code.branch_conditional(success, Arm64BranchCondition::CarryClear);
    retry_interrupted_or_store_target(code, job, retry);
    code.bind(success)?;
    code.bind(finished)?;
    Ok(())
}

fn retry_interrupted_or_store_target(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    retry: crate::Arm64LabelId,
) {
    compare_immediate(code, x(0), DarwinErrorAbi::INTERRUPTED);
    code.branch_conditional(retry, Arm64BranchCondition::Equal);
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    immediate(code, x(8), DarwinFileFailureKind::Target.code());
    store(
        code,
        job,
        schema.offset(DarwinFileJobField::FailureKind),
        x(8),
    );
    store(
        code,
        job,
        schema.offset(DarwinFileJobField::FailureErrno),
        x(0),
    );
}

fn publish_worker_result(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    notification_writer: crate::Arm64Register,
    worker_group: crate::Arm64Register,
    imports: &crate::Arm64DarwinFileServiceImports,
    retirement: crate::Arm64DarwinFileRetirementTargets,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    cleanup_failed_open_reservation(code, job, retirement)?;
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let inspect = code.create_label();
    let attached = code.create_label();
    let detached = code.create_label();
    let published = code.create_label();
    code.bind(inspect)?;
    load_job_state(code, x(20), job, &schema);
    compare_immediate(code, x(20), DarwinFileJobState::RunningAttached.code());
    code.branch_conditional(attached, Arm64BranchCondition::Equal);
    compare_immediate(code, x(20), DarwinFileJobState::RunningDetached.code());
    code.branch_conditional(detached, Arm64BranchCondition::Equal);
    abort(code, imports);
    code.bind(attached)?;
    transition_retry_to(
        code,
        job,
        DarwinFileJobState::RunningAttached,
        DarwinFileJobEvent::Publish,
        DarwinFileJobAction::RetainCompletion,
        inspect,
        published,
    )?;
    code.bind(detached)?;
    let released = code.create_label();
    transition_retry_to(
        code,
        job,
        DarwinFileJobState::RunningDetached,
        DarwinFileJobEvent::Publish,
        DarwinFileJobAction::ReleaseDetachedCompletion,
        inspect,
        released,
    )?;
    code.bind(released)?;
    release_detached_worker(code, job, worker_group, imports, retirement)?;
    code.bind(published)?;
    signal_descriptor(code, notification_writer);
    leave_worker_group(code, worker_group, imports);
    Ok(())
}

fn cleanup_failed_open_reservation(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    retirement: crate::Arm64DarwinFileRetirementTargets,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let done = code.create_label();
    load(
        code,
        x(20),
        job,
        schema.offset(DarwinFileJobField::Operation),
    );
    compare_immediate(code, x(20), u64::from(DarwinFileOperation::Open.code()));
    code.branch_conditional(done, Arm64BranchCondition::NotEqual);
    load(
        code,
        x(20),
        job,
        schema.offset(DarwinFileJobField::FailureKind),
    );
    compare_immediate(code, x(20), 0);
    code.branch_conditional(done, Arm64BranchCondition::Equal);
    load(
        code,
        x(0),
        job,
        schema.offset(DarwinFileJobField::RetirementRecord),
    );
    code.call(retirement.abandon_reservation());
    immediate(code, x(8), 0);
    store(
        code,
        job,
        schema.offset(DarwinFileJobField::RetirementRecord),
        x(8),
    );
    code.bind(done)?;
    Ok(())
}

fn cleanup_retirement(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    imports: &crate::Arm64DarwinFileServiceImports,
    retirement: crate::Arm64DarwinFileRetirementTargets,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let job_schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let record_schema = DarwinFileRetirementAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(21),
        job,
        job_schema.offset(DarwinFileJobField::RetirementRecord),
    );
    let done = code.create_label();
    compare_immediate(code, x(21), 0);
    code.branch_conditional(done, Arm64BranchCondition::Equal);
    address(
        code,
        x(22),
        x(21),
        record_schema.offset(DarwinFileRetirementField::LifecycleState),
    );
    code.append(Arm64Instruction::LoadAcquire {
        size: Arm64DataSize::Bits64,
        destination: Arm64DataRegister::General(x(23)),
        base: Arm64BaseRegister::General(x(22)),
    });
    let reserved = code.create_label();
    let live = code.create_label();
    compare_immediate(code, x(23), DarwinFileRetirementState::Reserved.code());
    code.branch_conditional(reserved, Arm64BranchCondition::Equal);
    compare_immediate(code, x(23), DarwinFileRetirementState::Live.code());
    code.branch_conditional(live, Arm64BranchCondition::Equal);
    abort(code, imports);
    code.bind(reserved)?;
    move_register(code, x(0), x(21));
    code.call(retirement.abandon_reservation());
    code.branch(done, false);
    code.bind(live)?;
    move_register(code, x(0), x(21));
    code.call(retirement.drop_owner());
    code.bind(done)?;
    Ok(())
}

fn copy_read_output(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    imports: &crate::Arm64DarwinFileServiceImports,
    schema: &DarwinFileJobAbiSchema,
) {
    load(
        code,
        x(20),
        job,
        schema.offset(DarwinFileJobField::Operation),
    );
    let copy = code.create_label();
    let done = code.create_label();
    compare_immediate(code, x(20), u64::from(DarwinFileOperation::Read.code()));
    code.branch_conditional(copy, Arm64BranchCondition::Equal);
    compare_immediate(code, x(20), u64::from(DarwinFileOperation::ReadAt.code()));
    code.branch_conditional(done, Arm64BranchCondition::NotEqual);
    code.bind(copy).expect("local consume label is valid");
    load(
        code,
        x(22),
        job,
        schema.offset(DarwinFileJobField::TransferredByteCount),
    );
    compare_immediate(code, x(22), 0);
    code.branch_conditional(done, Arm64BranchCondition::Equal);
    load(
        code,
        x(0),
        job,
        schema.offset(DarwinFileJobField::ConsumerBytePointer),
    );
    address(code, x(1), job, schema.owned_bytes_offset());
    move_register(code, x(2), x(22));
    call_import(code, imports, DarwinFileServiceFunction::MemoryCopy);
    code.bind(done).expect("local consume label is valid");
}

fn release_capacity_if_held(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let job_schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let service_schema = DarwinFileServiceAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(20),
        job,
        job_schema.offset(DarwinFileJobField::CapacityHeld),
    );
    let done = code.create_label();
    compare_immediate(code, x(20), 0);
    code.branch_conditional(done, Arm64BranchCondition::Equal);
    load(
        code,
        x(20),
        job,
        job_schema.offset(DarwinFileJobField::Service),
    );
    address(
        code,
        x(22),
        x(20),
        service_schema.offset(DarwinFileServiceField::ActiveOperationCount),
    );
    let released = code.create_label();
    let underflow = code.create_label();
    code.append_atomic_nonzero_decrement(
        atomic_registers(x(22), x(23), x(24)),
        released,
        underflow,
    )?;
    code.bind(underflow)?;
    abort(code, imports);
    code.bind(released)?;
    immediate(code, x(8), 0);
    store(
        code,
        job,
        job_schema.offset(DarwinFileJobField::CapacityHeld),
        x(8),
    );
    signal(code, job);
    code.bind(done)?;
    Ok(())
}

fn release_job(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(21),
        job,
        schema.offset(DarwinFileJobField::AllocationSize),
    );
    move_register(code, x(0), job);
    move_register(code, x(1), x(21));
    crate::darwin_memory_code::emit_unmap(
        code,
        crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameReleaseFailure,
    )?;
    epilogue(code);
    Ok(())
}

fn release_detached_worker(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    worker_group: crate::Arm64Register,
    imports: &crate::Arm64DarwinFileServiceImports,
    retirement: crate::Arm64DarwinFileRetirementTargets,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let job_schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    cleanup_retirement(code, job, imports, retirement)?;
    release_capacity_if_held(code, job, imports)?;
    load(
        code,
        x(21),
        job,
        job_schema.offset(DarwinFileJobField::AllocationSize),
    );
    move_register(code, x(0), job);
    move_register(code, x(1), x(21));
    crate::darwin_memory_code::emit_unmap(
        code,
        crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameReleaseFailure,
    )?;
    leave_worker_group(code, worker_group, imports);
    epilogue(code);
    Ok(())
}

fn retain_worker_resources(code: &mut Arm64CodeBuilder, job: crate::Arm64Register) {
    let job_schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let service_schema = DarwinFileServiceAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(26),
        job,
        job_schema.offset(DarwinFileJobField::Service),
    );
    load(
        code,
        x(27),
        x(26),
        service_schema.offset(DarwinFileServiceField::NotificationWriter),
    );
    load(
        code,
        x(28),
        x(26),
        service_schema.offset(DarwinFileServiceField::WorkerGroup),
    );
}

fn leave_worker_group(
    code: &mut Arm64CodeBuilder,
    worker_group: crate::Arm64Register,
    imports: &crate::Arm64DarwinFileServiceImports,
) {
    move_register(code, x(0), worker_group);
    call_import(code, imports, DarwinFileServiceFunction::DispatchGroupLeave);
}

fn load_descriptor(
    code: &mut Arm64CodeBuilder,
    destination: crate::Arm64Register,
    job: crate::Arm64Register,
) {
    let job_schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let record_schema = DarwinFileRetirementAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(20),
        job,
        job_schema.offset(DarwinFileJobField::RetirementRecord),
    );
    load(
        code,
        destination,
        x(20),
        record_schema.offset(DarwinFileRetirementField::Descriptor),
    );
}

fn store_failure(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    failure: DarwinFileFailureKind,
    errno: u64,
) {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    immediate(code, x(8), failure.code());
    store(
        code,
        job,
        schema.offset(DarwinFileJobField::FailureKind),
        x(8),
    );
    immediate(code, x(8), errno);
    store(
        code,
        job,
        schema.offset(DarwinFileJobField::FailureErrno),
        x(8),
    );
}

fn load_job_state(
    code: &mut Arm64CodeBuilder,
    destination: crate::Arm64Register,
    job: crate::Arm64Register,
    schema: &DarwinFileJobAbiSchema,
) {
    address(
        code,
        x(22),
        job,
        schema.offset(DarwinFileJobField::LifecycleState),
    );
    code.append(Arm64Instruction::LoadAcquire {
        size: Arm64DataSize::Bits64,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(x(22)),
    });
}

fn transition_exact(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    state: DarwinFileJobState,
    event: DarwinFileJobEvent,
    expected: DarwinFileJobAction,
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    address(
        code,
        x(22),
        job,
        schema.offset(DarwinFileJobField::LifecycleState),
    );
    let success = code.create_label();
    let mismatch = code.create_label();
    let action = crate::emit_darwin_file_job_transition(
        code,
        atomic_registers(x(22), x(23), x(24)),
        state,
        event,
        success,
        mismatch,
    )?;
    require_action(action, expected)?;
    code.bind(mismatch)?;
    abort(code, imports);
    code.bind(success)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn transition_retry_to(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    state: DarwinFileJobState,
    event: DarwinFileJobEvent,
    expected: DarwinFileJobAction,
    retry: crate::Arm64LabelId,
    success: crate::Arm64LabelId,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    address(
        code,
        x(22),
        job,
        schema.offset(DarwinFileJobField::LifecycleState),
    );
    let action = crate::emit_darwin_file_job_transition(
        code,
        atomic_registers(x(22), x(23), x(24)),
        state,
        event,
        success,
        retry,
    )?;
    require_action(action, expected)
}

fn require_action(
    actual: DarwinFileJobAction,
    expected: DarwinFileJobAction,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    if actual == expected {
        Ok(())
    } else {
        Err(crate::Arm64DarwinFileJobError::LifecycleAction)
    }
}

fn store_immediate(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    schema: &DarwinFileJobAbiSchema,
    field: DarwinFileJobField,
    value: u64,
) {
    immediate(code, x(8), value);
    store(code, job, schema.offset(field), x(8));
}

fn subtract_register(
    code: &mut Arm64CodeBuilder,
    destination: crate::Arm64Register,
    left: crate::Arm64Register,
    right: crate::Arm64Register,
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
