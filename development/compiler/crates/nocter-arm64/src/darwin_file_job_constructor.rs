use nocter_runtime_contract::{
    DarwinFileJobAbiSchema, DarwinFileJobField, DarwinFileJobState, DarwinFileOperation,
    DarwinFileServiceAbiSchema, DarwinFileServiceField, DarwinFileServiceFunction,
};

use crate::darwin_file_job_code::{
    abort, add_immediate, add_register, address, call_import, compare_immediate, epilogue,
    immediate, load, move_register, prologue, store, x,
};
use crate::darwin_kernel_abi::DarwinFileAbi;
use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64Logical, Arm64NocterAbi,
};

pub(crate) fn build(
    operation: DarwinFileOperation,
    targets: crate::Arm64DarwinFileJobTargets,
    imports: &crate::Arm64DarwinFileServiceImports,
    root: crate::Arm64DarwinFileServiceRootTargets,
) -> Result<Arm64CodeBuilder, crate::Arm64DarwinFileJobError> {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let service = DarwinFileServiceAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code);
    stage_inputs(&mut code, operation, imports)?;
    move_register(
        &mut code,
        x(26),
        Arm64NocterAbi::allocation_context_register(),
    );

    move_register(&mut code, x(0), Arm64NocterAbi::process_context_register());
    code.call(root.ensure());
    move_register(&mut code, x(23), x(0));

    compare_register_sum(&mut code, x(25), x(20), schema.fixed_size(), imports);
    move_register(&mut code, x(1), x(25));
    crate::darwin_memory_code::emit_map(&mut code)?;
    move_register(&mut code, x(24), x(0));

    zero_fixed_record(&mut code, x(24), &schema)?;
    for (target, field) in [
        (targets.resume(), DarwinFileJobField::ResumeFunction),
        (targets.cancel(), DarwinFileJobField::CancelFunction),
        (targets.consume(), DarwinFileJobField::ConsumeFunction),
    ] {
        code.load_function_address(target, x(8));
        store(&mut code, x(24), schema.offset(field), x(8));
    }
    store_immediate(
        &mut code,
        x(24),
        &schema,
        DarwinFileJobField::LifecycleState,
        DarwinFileJobState::Prepared.code(),
    );
    store(
        &mut code,
        x(24),
        schema.offset(DarwinFileJobField::AllocationContext),
        x(26),
    );
    store(
        &mut code,
        x(24),
        schema.offset(DarwinFileJobField::Service),
        x(23),
    );
    store_immediate(
        &mut code,
        x(24),
        &schema,
        DarwinFileJobField::Operation,
        u64::from(operation.code()),
    );
    store(
        &mut code,
        x(24),
        schema.offset(DarwinFileJobField::RetirementRecord),
        x(19),
    );
    store(
        &mut code,
        x(24),
        schema.offset(DarwinFileJobField::AllocationSize),
        x(25),
    );
    store(
        &mut code,
        x(24),
        schema.offset(DarwinFileJobField::OwnedByteLength),
        x(20),
    );
    initialize_operands(&mut code, x(24), operation, &schema);
    initialize_interest(&mut code, x(24), x(23), &schema, &service);
    initialize_owned_bytes(&mut code, x(24), operation, imports, &schema);
    move_register(&mut code, x(0), x(24));
    epilogue(&mut code);
    Ok(code)
}

/// Stages every constructor into one operation-independent register convention.
///
/// x19 retirement owner, x20 total trailing-byte length, x21 primary byte pointer or consumer
/// pointer, x22 secondary byte pointer or scalar operand, and x27/x28 terminated path lengths.
fn stage_inputs(
    code: &mut Arm64CodeBuilder,
    operation: DarwinFileOperation,
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    match operation {
        DarwinFileOperation::Open => {
            immediate(code, x(19), 0);
            move_register(code, x(21), x(0));
            move_register(code, x(20), x(1));
            compare_immediate(code, x(20), u64::MAX);
            let valid = code.create_label();
            code.branch_conditional(valid, Arm64BranchCondition::NotEqual);
            abort(code, imports);
            code.bind(valid)?;
            add_immediate(code, x(20), x(20), 1);
            move_register(code, x(22), x(2));
        }
        DarwinFileOperation::Read
        | DarwinFileOperation::ReadDirectory
        | DarwinFileOperation::Write => {
            move_register(code, x(19), x(0));
            move_register(code, x(21), x(1));
            move_register(code, x(20), x(2));
            immediate(code, x(22), 0);
        }
        DarwinFileOperation::Flush => {
            move_register(code, x(19), x(0));
            immediate(code, x(20), 0);
            immediate(code, x(21), 0);
            immediate(code, x(22), 0);
        }
        DarwinFileOperation::Seek => {
            move_register(code, x(19), x(0));
            immediate(code, x(20), 0);
            move_register(code, x(21), x(1));
            move_register(code, x(22), x(2));
        }
        DarwinFileOperation::Truncate => {
            move_register(code, x(19), x(0));
            immediate(code, x(20), 0);
            immediate(code, x(21), 0);
            move_register(code, x(22), x(1));
        }
        DarwinFileOperation::ReadAt | DarwinFileOperation::WriteAt => {
            move_register(code, x(19), x(0));
            move_register(code, x(21), x(1));
            move_register(code, x(20), x(2));
            move_register(code, x(22), x(3));
        }
        DarwinFileOperation::RemoveFile
        | DarwinFileOperation::CreateDirectory
        | DarwinFileOperation::RemoveDirectory => {
            immediate(code, x(19), 0);
            move_register(code, x(21), x(0));
            move_register(code, x(27), x(1));
            add_path_terminator(code, x(27), imports)?;
            move_register(code, x(20), x(27));
            immediate(code, x(22), 0);
            immediate(code, x(28), 0);
        }
        DarwinFileOperation::Rename | DarwinFileOperation::CreateSymlink => {
            immediate(code, x(19), 0);
            move_register(code, x(21), x(0));
            move_register(code, x(27), x(1));
            move_register(code, x(22), x(2));
            move_register(code, x(28), x(3));
            add_path_terminator(code, x(27), imports)?;
            add_path_terminator(code, x(28), imports)?;
            add_register(code, x(20), x(27), x(28), true);
            let valid = code.create_label();
            code.branch_conditional(valid, Arm64BranchCondition::CarryClear);
            abort(code, imports);
            code.bind(valid)?;
        }
        DarwinFileOperation::Metadata | DarwinFileOperation::SymlinkMetadata => {
            immediate(code, x(19), 0);
            move_register(code, x(21), x(0));
            move_register(code, x(27), x(1));
            add_path_terminator(code, x(27), imports)?;
            align_up_metadata_path(code, x(20), x(27), imports)?;
            compare_register_sum(code, x(20), x(20), DarwinFileAbi::STAT_BUFFER_SIZE, imports);
            immediate(code, x(22), 0);
            immediate(code, x(28), 0);
        }
    }
    Ok(())
}

fn add_path_terminator(
    code: &mut Arm64CodeBuilder,
    length: crate::Arm64Register,
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    compare_immediate(code, length, u64::MAX);
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::NotEqual);
    abort(code, imports);
    code.bind(valid)?;
    add_immediate(code, length, length, 1);
    Ok(())
}

fn align_up_metadata_path(
    code: &mut Arm64CodeBuilder,
    aligned: crate::Arm64Register,
    length: crate::Arm64Register,
    imports: &crate::Arm64DarwinFileServiceImports,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    immediate(code, x(8), 7);
    add_register(code, x(9), length, x(8), true);
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::CarryClear);
    abort(code, imports);
    code.bind(valid)?;
    immediate(code, x(8), u64::MAX - 7);
    code.append(Arm64Instruction::LogicalRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64Logical::And,
        destination: Arm64DataRegister::General(aligned),
        left: Arm64DataRegister::General(x(9)),
        right: Arm64DataRegister::General(x(8)),
    });
    Ok(())
}

fn compare_register_sum(
    code: &mut Arm64CodeBuilder,
    total: crate::Arm64Register,
    dynamic: crate::Arm64Register,
    fixed: u64,
    imports: &crate::Arm64DarwinFileServiceImports,
) {
    immediate(code, x(8), fixed);
    add_register(code, x(9), dynamic, x(8), true);
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::CarryClear);
    abort(code, imports);
    code.bind(valid).expect("local constructor label is valid");
    move_register(code, total, x(9));
}

fn zero_fixed_record(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    schema: &DarwinFileJobAbiSchema,
) -> Result<(), crate::Arm64DarwinFileJobError> {
    if schema.fixed_size() == 0 || !schema.fixed_size().is_multiple_of(8) {
        return Err(crate::Arm64DarwinFileJobError::ContractLayout);
    }
    move_register(code, x(10), job);
    immediate(code, x(11), schema.fixed_size() / 8);
    immediate(code, x(8), 0);
    let repeat = code.create_label();
    code.bind(repeat)?;
    store(code, x(10), 0, x(8));
    add_immediate(code, x(10), x(10), 8);
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::General(x(11)),
        source: Arm64BaseRegister::General(x(11)),
        immediate: 1,
        shift_12: false,
    });
    code.branch_conditional(repeat, Arm64BranchCondition::NotEqual);
    Ok(())
}

fn initialize_operands(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    operation: DarwinFileOperation,
    schema: &DarwinFileJobAbiSchema,
) {
    match operation {
        DarwinFileOperation::Open => {
            store(code, job, schema.offset(DarwinFileJobField::Access), x(22));
        }
        DarwinFileOperation::Read | DarwinFileOperation::ReadDirectory => {
            store(
                code,
                job,
                schema.offset(DarwinFileJobField::ConsumerBytePointer),
                x(21),
            );
        }
        DarwinFileOperation::Write
        | DarwinFileOperation::Flush
        | DarwinFileOperation::RemoveFile
        | DarwinFileOperation::CreateDirectory
        | DarwinFileOperation::RemoveDirectory
        | DarwinFileOperation::Metadata
        | DarwinFileOperation::SymlinkMetadata => {}
        DarwinFileOperation::Seek => {
            store(
                code,
                job,
                schema.offset(DarwinFileJobField::SeekOrigin),
                x(21),
            );
            store(
                code,
                job,
                schema.offset(DarwinFileJobField::SeekDisplacement),
                x(22),
            );
        }
        DarwinFileOperation::Truncate => {
            store(
                code,
                job,
                schema.offset(DarwinFileJobField::TruncateLength),
                x(22),
            );
        }
        DarwinFileOperation::ReadAt => {
            store(
                code,
                job,
                schema.offset(DarwinFileJobField::ConsumerBytePointer),
                x(21),
            );
            store(
                code,
                job,
                schema.offset(DarwinFileJobField::PositionedOffset),
                x(22),
            );
        }
        DarwinFileOperation::WriteAt => {
            store(
                code,
                job,
                schema.offset(DarwinFileJobField::PositionedOffset),
                x(22),
            );
        }
        DarwinFileOperation::Rename | DarwinFileOperation::CreateSymlink => {
            store(
                code,
                job,
                schema.offset(DarwinFileJobField::SecondaryPathOffset),
                x(27),
            );
        }
    }
}

fn initialize_interest(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    service: crate::Arm64Register,
    schema: &DarwinFileJobAbiSchema,
    service_schema: &DarwinFileServiceAbiSchema,
) {
    let asynchronous = schema.asynchronous();
    store_immediate(
        code,
        job,
        schema,
        DarwinFileJobField::InterestKind,
        asynchronous.descriptor_interest_kind(),
    );
    load(
        code,
        x(8),
        service,
        service_schema.offset(DarwinFileServiceField::NotificationReader),
    );
    store(
        code,
        job,
        schema.offset(DarwinFileJobField::InterestSubject),
        x(8),
    );
    store_immediate(
        code,
        job,
        schema,
        DarwinFileJobField::InterestDetail,
        asynchronous.readable_interest_detail(),
    );
    address(
        code,
        x(8),
        job,
        schema.offset(DarwinFileJobField::Readiness),
    );
    store(
        code,
        job,
        schema.offset(DarwinFileJobField::InterestReadinessPointer),
        x(8),
    );
}

fn initialize_owned_bytes(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    operation: DarwinFileOperation,
    imports: &crate::Arm64DarwinFileServiceImports,
    schema: &DarwinFileJobAbiSchema,
) {
    match operation {
        DarwinFileOperation::Open => {
            copy_path(code, job, x(21), x(20), None, imports, schema);
        }
        DarwinFileOperation::Write | DarwinFileOperation::WriteAt => {
            let skip = code.create_label();
            compare_immediate(code, x(20), 0);
            code.branch_conditional(skip, Arm64BranchCondition::Equal);
            address(code, x(0), job, schema.owned_bytes_offset());
            move_register(code, x(1), x(21));
            move_register(code, x(2), x(20));
            call_import(code, imports, DarwinFileServiceFunction::MemoryCopy);
            code.bind(skip).expect("local constructor label is valid");
        }
        DarwinFileOperation::RemoveFile
        | DarwinFileOperation::CreateDirectory
        | DarwinFileOperation::RemoveDirectory
        | DarwinFileOperation::Metadata
        | DarwinFileOperation::SymlinkMetadata => {
            copy_path(code, job, x(21), x(27), None, imports, schema);
        }
        DarwinFileOperation::Rename | DarwinFileOperation::CreateSymlink => {
            copy_path(code, job, x(21), x(27), None, imports, schema);
            copy_path(code, job, x(22), x(28), Some(x(27)), imports, schema);
        }
        DarwinFileOperation::Read
        | DarwinFileOperation::ReadDirectory
        | DarwinFileOperation::Flush
        | DarwinFileOperation::Seek
        | DarwinFileOperation::Truncate
        | DarwinFileOperation::ReadAt => {}
    }
}

fn copy_path(
    code: &mut Arm64CodeBuilder,
    job: crate::Arm64Register,
    source: crate::Arm64Register,
    terminated_length: crate::Arm64Register,
    offset: Option<crate::Arm64Register>,
    imports: &crate::Arm64DarwinFileServiceImports,
    schema: &DarwinFileJobAbiSchema,
) {
    subtract_one(code, x(10), terminated_length);
    let skip = code.create_label();
    compare_immediate(code, x(10), 0);
    code.branch_conditional(skip, Arm64BranchCondition::Equal);
    path_address(code, x(0), job, offset, schema);
    move_register(code, x(1), source);
    move_register(code, x(2), x(10));
    call_import(code, imports, DarwinFileServiceFunction::MemoryCopy);
    code.bind(skip).expect("local path-copy label is valid");
    path_address(code, x(9), job, offset, schema);
    subtract_one(code, x(10), terminated_length);
    add_register(code, x(9), x(9), x(10), false);
    immediate(code, x(8), 0);
    crate::address_code::store_native(code, Arm64LoadStoreSize::Byte, x(8), x(9), 0);
}

fn path_address(
    code: &mut Arm64CodeBuilder,
    destination: crate::Arm64Register,
    job: crate::Arm64Register,
    offset: Option<crate::Arm64Register>,
    schema: &DarwinFileJobAbiSchema,
) {
    address(code, destination, job, schema.owned_bytes_offset());
    if let Some(offset) = offset {
        add_register(code, destination, destination, offset, false);
    }
}

fn subtract_one(
    code: &mut Arm64CodeBuilder,
    destination: crate::Arm64Register,
    source: crate::Arm64Register,
) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::General(source),
        immediate: 1,
        shift_12: false,
    });
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
