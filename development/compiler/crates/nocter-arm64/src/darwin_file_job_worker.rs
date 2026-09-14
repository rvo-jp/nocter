use nocter_runtime_contract::{
    DarwinCanonicalPathAbi, DarwinFileAccess, DarwinFileFailureKind, DarwinFileJobAbiSchema,
    DarwinFileJobField, DarwinFileMetadataKind, DarwinFileOperation, DarwinFileRetirementAbiSchema,
    DarwinFileRetirementField, DarwinFileSeekOrigin,
};

use crate::darwin_file_job_code::{
    abort, add_immediate, add_register, address, compare_immediate, compare_register, immediate,
    load, move_register, store, x,
};
use crate::darwin_kernel_abi::{DarwinErrorAbi, DarwinFileAbi, DarwinSystemCall, emit_system_call};
use crate::{
    Arm64AddSubtract, Arm64BranchCondition, Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize,
    Arm64Instruction, Arm64Logical,
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
        DarwinFileOperation::ReadDirectory => execute_directory_read(code, job),
        DarwinFileOperation::Write => execute_write(code, job, false, imports),
        DarwinFileOperation::Flush => execute_flush(code, job, imports),
        DarwinFileOperation::Seek => execute_seek(code, job, imports),
        DarwinFileOperation::Truncate => execute_truncate(code, job, imports),
        DarwinFileOperation::ReadAt => execute_read(code, job, true, imports),
        DarwinFileOperation::WriteAt => execute_write(code, job, true, imports),
        DarwinFileOperation::RemoveFile
        | DarwinFileOperation::Rename
        | DarwinFileOperation::CreateSymlink
        | DarwinFileOperation::CreateDirectory
        | DarwinFileOperation::RemoveDirectory => execute_path_mutation(code, job, operation),
        DarwinFileOperation::Metadata | DarwinFileOperation::SymlinkMetadata => {
            execute_metadata(code, job, operation)
        }
        DarwinFileOperation::ReadLink => execute_read_link(code, job),
        DarwinFileOperation::Canonicalize => execute_canonicalize(code, job),
        DarwinFileOperation::Identity => execute_identity(code, job),
    }
}

fn execute_identity(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let retry = code.create_label();
    code.bind(retry)?;
    load_descriptor(code, x(0), job);
    address(code, x(1), job, schema.owned_bytes_offset());
    emit_system_call(code, DarwinSystemCall::Fstat64);
    let success = code.create_label();
    code.branch_conditional(success, Arm64BranchCondition::CarryClear);
    retry_interrupted_or_store_target(code, job, retry);
    let finished = code.create_label();
    code.branch(finished, false);
    code.bind(success)?;
    address(code, x(20), job, schema.owned_bytes_offset());
    crate::address_code::load_native(
        code,
        crate::Arm64LoadStoreSize::Word,
        None,
        x(21),
        x(20),
        DarwinFileAbi::STAT_DEVICE_OFFSET,
    );
    crate::address_code::load_native(
        code,
        crate::Arm64LoadStoreSize::Double,
        None,
        x(22),
        x(20),
        DarwinFileAbi::STAT_INODE_OFFSET,
    );
    store(
        code,
        job,
        schema.offset(DarwinFileJobField::IdentityDevice),
        x(21),
    );
    store(
        code,
        job,
        schema.offset(DarwinFileJobField::IdentityInode),
        x(22),
    );
    code.bind(finished)?;
    Ok(())
}

fn execute_canonicalize(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(21),
        job,
        schema.offset(DarwinFileJobField::OwnedByteLength),
    );
    load(
        code,
        x(22),
        job,
        schema.offset(DarwinFileJobField::SecondaryBytesOffset),
    );
    subtract_register(code, x(21), x(21), x(22));
    let capacity_valid = code.create_label();
    compare_immediate(code, x(21), DarwinCanonicalPathAbi::OUTPUT_SIZE as u64);
    code.branch_conditional(capacity_valid, Arm64BranchCondition::CarrySet);
    store_failure(code, job, DarwinFileFailureKind::InvalidProgress, 0);
    let finished = code.create_label();
    code.branch(finished, false);
    code.bind(capacity_valid)?;

    let retry_open = code.create_label();
    code.bind(retry_open)?;
    address(code, x(0), job, schema.owned_bytes_offset());
    immediate(code, x(1), DarwinFileAbi::EVENT_ONLY_CLOSE_ON_EXEC);
    immediate(code, x(2), 0);
    emit_system_call(code, DarwinSystemCall::Open);
    let opened = code.create_label();
    code.branch_conditional(opened, Arm64BranchCondition::CarryClear);
    retry_interrupted_or_store_target(code, job, retry_open);
    code.branch(finished, false);
    code.bind(opened)?;
    move_register(code, x(23), x(0));

    address(code, x(24), job, schema.owned_bytes_offset());
    add_register(code, x(24), x(24), x(22), false);
    let retry_get_path = code.create_label();
    code.bind(retry_get_path)?;
    move_register(code, x(0), x(23));
    immediate(code, x(1), DarwinFileAbi::GET_PATH);
    move_register(code, x(2), x(24));
    emit_system_call(code, DarwinSystemCall::Fcntl);
    let path_ready = code.create_label();
    code.branch_conditional(path_ready, Arm64BranchCondition::CarryClear);
    retry_interrupted_or_store_target(code, job, retry_get_path);
    let close_preserving_failure = code.create_label();
    code.branch(close_preserving_failure, false);

    code.bind(path_ready)?;
    immediate(code, x(25), 0);
    let scan = code.create_label();
    let path_complete = code.create_label();
    let invalid_path = code.create_label();
    code.bind(scan)?;
    compare_immediate(code, x(25), DarwinCanonicalPathAbi::OUTPUT_SIZE as u64);
    code.branch_conditional(invalid_path, Arm64BranchCondition::Equal);
    add_register(code, x(9), x(24), x(25), false);
    crate::address_code::load_native(code, crate::Arm64LoadStoreSize::Byte, None, x(8), x(9), 0);
    compare_immediate(code, x(8), 0);
    code.branch_conditional(path_complete, Arm64BranchCondition::Equal);
    add_immediate(code, x(25), x(25), 1);
    code.branch(scan, false);

    code.bind(invalid_path)?;
    store_failure(code, job, DarwinFileFailureKind::InvalidProgress, 0);
    code.branch(close_preserving_failure, false);

    code.bind(path_complete)?;
    store(
        code,
        job,
        schema.offset(DarwinFileJobField::TransferredByteCount),
        x(25),
    );
    move_register(code, x(0), x(23));
    emit_system_call(code, DarwinSystemCall::Close);
    let close_succeeded = code.create_label();
    code.branch_conditional(close_succeeded, Arm64BranchCondition::CarryClear);
    store_target_failure(code, job);
    code.bind(close_succeeded)?;
    code.branch(finished, false);

    code.bind(close_preserving_failure)?;
    move_register(code, x(0), x(23));
    emit_system_call(code, DarwinSystemCall::Close);
    code.bind(finished)?;
    Ok(())
}

fn execute_read_link(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(21),
        job,
        schema.offset(DarwinFileJobField::OwnedByteLength),
    );
    load(
        code,
        x(22),
        job,
        schema.offset(DarwinFileJobField::SecondaryBytesOffset),
    );
    subtract_register(code, x(21), x(21), x(22));
    let valid = code.create_label();
    immediate(code, x(8), DarwinFileAbi::MAXIMUM_TRANSFER);
    compare_register(code, x(21), x(8));
    code.branch_conditional(valid, Arm64BranchCondition::UnsignedLowerOrSame);
    store_failure(code, job, DarwinFileFailureKind::InvalidProgress, 0);
    let finished = code.create_label();
    code.branch(finished, false);
    code.bind(valid)?;
    let retry = code.create_label();
    code.bind(retry)?;
    address(code, x(0), job, schema.owned_bytes_offset());
    address(code, x(1), job, schema.owned_bytes_offset());
    add_register(code, x(1), x(1), x(22), false);
    move_register(code, x(2), x(21));
    emit_system_call(code, DarwinSystemCall::ReadLink);
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

fn execute_metadata(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    operation: DarwinFileOperation,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let retry = code.create_label();
    code.bind(retry)?;
    address(code, x(0), job, schema.owned_bytes_offset());
    load(
        code,
        x(8),
        job,
        schema.offset(DarwinFileJobField::OwnedByteLength),
    );
    immediate(code, x(9), DarwinFileAbi::STAT_BUFFER_SIZE);
    subtract_register(code, x(8), x(8), x(9));
    address(code, x(1), job, schema.owned_bytes_offset());
    add_register(code, x(1), x(1), x(8), false);
    move_register(code, x(20), x(1));
    let syscall = match operation {
        DarwinFileOperation::Metadata => DarwinSystemCall::Stat64,
        DarwinFileOperation::SymlinkMetadata => DarwinSystemCall::Lstat64,
        _ => unreachable!("metadata worker only receives metadata operations"),
    };
    emit_system_call(code, syscall);
    let success = code.create_label();
    code.branch_conditional(success, Arm64BranchCondition::CarryClear);
    retry_interrupted_or_store_target(code, job, retry);
    let finished = code.create_label();
    code.branch(finished, false);
    code.bind(success)?;

    publish_metadata(code, job, x(20))?;
    code.bind(finished)?;
    Ok(())
}

fn publish_metadata(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    target_record: crate::Arm64Register,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    crate::address_code::load_native(
        code,
        crate::Arm64LoadStoreSize::Half,
        None,
        x(21),
        target_record,
        DarwinFileAbi::STAT_MODE_OFFSET,
    );
    let regular = code.create_label();
    let directory = code.create_label();
    let symbolic_link = code.create_label();
    let publish = code.create_label();
    immediate(code, x(8), DarwinFileAbi::STAT_MODE_KIND_MASK);
    code.append(Arm64Instruction::LogicalRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64Logical::And,
        destination: Arm64DataRegister::General(x(21)),
        left: Arm64DataRegister::General(x(21)),
        right: Arm64DataRegister::General(x(8)),
    });
    immediate(code, x(8), DarwinFileAbi::STAT_MODE_REGULAR);
    compare_register(code, x(21), x(8));
    code.branch_conditional(regular, Arm64BranchCondition::Equal);
    immediate(code, x(8), DarwinFileAbi::STAT_MODE_DIRECTORY);
    compare_register(code, x(21), x(8));
    code.branch_conditional(directory, Arm64BranchCondition::Equal);
    immediate(code, x(8), DarwinFileAbi::STAT_MODE_SYMBOLIC_LINK);
    compare_register(code, x(21), x(8));
    code.branch_conditional(symbolic_link, Arm64BranchCondition::Equal);
    immediate(code, x(22), DarwinFileMetadataKind::Other.code());
    code.branch(publish, false);
    code.bind(regular)?;
    immediate(code, x(22), DarwinFileMetadataKind::Regular.code());
    code.branch(publish, false);
    code.bind(directory)?;
    immediate(code, x(22), DarwinFileMetadataKind::Directory.code());
    code.branch(publish, false);
    code.bind(symbolic_link)?;
    immediate(code, x(22), DarwinFileMetadataKind::SymbolicLink.code());
    code.bind(publish)?;
    store(
        code,
        job,
        schema.offset(DarwinFileJobField::MetadataKind),
        x(22),
    );
    for (offset, field) in [
        (
            DarwinFileAbi::STAT_SIZE_OFFSET,
            DarwinFileJobField::MetadataLength,
        ),
        (
            DarwinFileAbi::STAT_MODIFIED_SECONDS_OFFSET,
            DarwinFileJobField::MetadataModifiedSeconds,
        ),
        (
            DarwinFileAbi::STAT_MODIFIED_NANOSECONDS_OFFSET,
            DarwinFileJobField::MetadataModifiedNanoseconds,
        ),
    ] {
        crate::address_code::load_native(
            code,
            crate::Arm64LoadStoreSize::Double,
            None,
            x(8),
            target_record,
            offset,
        );
        store(code, job, schema.offset(field), x(8));
    }
    Ok(())
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
    let directory = code.create_label();
    let copy_destination = code.create_label();
    let invoke = code.create_label();
    load(code, x(20), job, schema.offset(DarwinFileJobField::Access));
    compare_immediate(code, x(20), u64::from(DarwinFileAccess::Read.code()));
    code.branch_conditional(read, Arm64BranchCondition::Equal);
    compare_immediate(code, x(20), u64::from(DarwinFileAccess::Create.code()));
    code.branch_conditional(create, Arm64BranchCondition::Equal);
    compare_immediate(code, x(20), u64::from(DarwinFileAccess::Append.code()));
    code.branch_conditional(append, Arm64BranchCondition::Equal);
    compare_immediate(code, x(20), u64::from(DarwinFileAccess::Directory.code()));
    code.branch_conditional(directory, Arm64BranchCondition::Equal);
    compare_immediate(
        code,
        x(20),
        u64::from(DarwinFileAccess::CopyDestination.code()),
    );
    code.branch_conditional(copy_destination, Arm64BranchCondition::Equal);
    abort(code, imports);
    code.bind(read)?;
    immediate(code, x(21), DarwinFileAbi::READ_ONLY);
    code.branch(invoke, false);
    code.bind(create)?;
    immediate(code, x(21), DarwinFileAbi::CREATE_TRUNCATE_WRITE_ONLY);
    code.branch(invoke, false);
    code.bind(append)?;
    immediate(code, x(21), DarwinFileAbi::CREATE_APPEND_WRITE_ONLY);
    code.branch(invoke, false);
    code.bind(directory)?;
    immediate(code, x(21), DarwinFileAbi::DIRECTORY_ONLY);
    code.branch(invoke, false);
    code.bind(copy_destination)?;
    immediate(code, x(21), DarwinFileAbi::COPY_DESTINATION);
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

fn execute_directory_read(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
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
    let retry = code.create_label();
    code.bind(retry)?;
    load_descriptor(code, x(0), job);
    address(code, x(1), job, schema.owned_bytes_offset());
    move_register(code, x(2), x(21));
    address(
        code,
        x(3),
        job,
        schema.offset(DarwinFileJobField::DirectoryBasePosition),
    );
    emit_system_call(code, DarwinSystemCall::GetDirectoryEntries64);
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

fn execute_path_mutation(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    operation: DarwinFileOperation,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    address(code, x(0), job, schema.owned_bytes_offset());
    match operation {
        DarwinFileOperation::RemoveFile => emit_system_call(code, DarwinSystemCall::Unlink),
        DarwinFileOperation::Rename => {
            load(
                code,
                x(8),
                job,
                schema.offset(DarwinFileJobField::SecondaryBytesOffset),
            );
            address(code, x(1), job, schema.owned_bytes_offset());
            add_register(code, x(1), x(1), x(8), false);
            emit_system_call(code, DarwinSystemCall::Rename);
        }
        DarwinFileOperation::CreateSymlink => {
            load(
                code,
                x(8),
                job,
                schema.offset(DarwinFileJobField::SecondaryBytesOffset),
            );
            address(code, x(1), job, schema.owned_bytes_offset());
            add_register(code, x(1), x(1), x(8), false);
            emit_system_call(code, DarwinSystemCall::Symlink);
        }
        DarwinFileOperation::CreateDirectory => {
            immediate(code, x(1), DarwinFileAbi::CREATE_DIRECTORY_MODE);
            emit_system_call(code, DarwinSystemCall::MakeDirectory);
        }
        DarwinFileOperation::RemoveDirectory => {
            emit_system_call(code, DarwinSystemCall::RemoveDirectory);
        }
        _ => unreachable!("path worker accepts only path mutations"),
    }
    let success = code.create_label();
    code.branch_conditional(success, Arm64BranchCondition::CarryClear);
    store_target_failure(code, job);
    code.bind(success)?;
    Ok(())
}

fn retry_interrupted_or_store_target(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    retry: crate::Arm64LabelId,
) {
    compare_immediate(code, x(0), DarwinErrorAbi::INTERRUPTED);
    code.branch_conditional(retry, Arm64BranchCondition::Equal);
    store_target_failure(code, job);
}

fn store_target_failure(code: &mut Arm64CodeBuilder, job: crate::Arm64Register) {
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
