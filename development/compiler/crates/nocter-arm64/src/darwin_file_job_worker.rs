use nocter_runtime_contract::{
    DarwinFileAccess, DarwinFileFailureKind, DarwinFileJobAbiSchema, DarwinFileJobField,
    DarwinFileOperation, DarwinFileRetirementAbiSchema, DarwinFileRetirementField,
    DarwinFileSeekOrigin,
};

use crate::darwin_file_job_code::{
    abort, add_register, address, compare_immediate, compare_register, immediate, load,
    move_register, store, x,
};
use crate::darwin_kernel_abi::{DarwinErrorAbi, DarwinFileAbi, DarwinSystemCall, emit_system_call};
use crate::{
    Arm64AddSubtract, Arm64BranchCondition, Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize,
    Arm64Instruction,
};

pub(crate) fn execute_operation(
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
