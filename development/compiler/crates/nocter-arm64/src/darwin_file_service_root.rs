use std::fmt;

use nocter_runtime_contract::{
    DarwinFileRetirementField, DarwinFileRetirementState, DarwinFileServiceAbiSchema,
    DarwinFileServiceAction, DarwinFileServiceEvent, DarwinFileServiceField,
    DarwinFileServiceFunction, DarwinFileServiceState,
};

use crate::darwin_kernel_abi::{DarwinDescriptorAbi, DarwinSystemCall, emit_system_call};
use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64AtomicUpdateRegisters, Arm64BaseRegister,
    Arm64BranchCondition, Arm64CodeBuilder, Arm64CodeError, Arm64DarwinFileLifecycleError,
    Arm64DarwinFileServiceImports, Arm64DataRegister, Arm64DataSize, Arm64FunctionId,
    Arm64Instruction, Arm64LoadStoreSize, Arm64Logical, Arm64NocterAbi, Arm64ProgramBuilder,
    Arm64ProgramError, Arm64Register,
};

const STACK_SIZE: u16 = 64;

/// Compiler-owned access to the one generated Darwin file-service root in a process context.
///
/// `ensure` accepts the process-context pointer in `x0` and returns its existing or newly created
/// service in `x0`. `shutdown` accepts the same pointer, closes admission, drains every worker,
/// validates that no retirement owner survived executor teardown, releases all native resources,
/// clears the context slot, and frees the root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinFileServiceRootTargets {
    ensure: Arm64FunctionId,
    shutdown: Arm64FunctionId,
}

impl Arm64DarwinFileServiceRootTargets {
    /// Declares and defines the complete root lifecycle as one inseparable target family.
    ///
    /// # Errors
    ///
    /// Propagates runtime-layout, ARM64 code, lifecycle, and program construction failures.
    pub fn declare(
        program: &mut Arm64ProgramBuilder,
        imports: &Arm64DarwinFileServiceImports,
    ) -> Result<Self, Arm64DarwinFileServiceRootError> {
        let ensure = program.declare_function();
        let shutdown = program.declare_function();
        program.define_function(ensure, build_ensure(imports)?.finish()?)?;
        program.define_function(shutdown, build_shutdown(imports)?.finish()?)?;
        Ok(Self { ensure, shutdown })
    }

    #[must_use]
    pub const fn ensure(self) -> Arm64FunctionId {
        self.ensure
    }

    #[must_use]
    pub const fn shutdown(self) -> Arm64FunctionId {
        self.shutdown
    }
}

fn build_ensure(
    imports: &Arm64DarwinFileServiceImports,
) -> Result<Arm64CodeBuilder, Arm64DarwinFileServiceRootError> {
    let schema = DarwinFileServiceAbiSchema::ARM64_DARWIN;
    let process = Arm64NocterAbi::process_context();
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code);
    move_register(&mut code, x(19), x(0));
    address(
        &mut code,
        x(20),
        x(19),
        process.blocking_service_pointer_offset(),
    )?;
    code.append(Arm64Instruction::LoadAcquire {
        size: Arm64DataSize::Bits64,
        destination: Arm64DataRegister::General(x(0)),
        base: Arm64BaseRegister::General(x(20)),
    });
    let ready = code.create_label();
    compare_immediate(&mut code, x(0), 0);
    code.branch_conditional(ready, Arm64BranchCondition::NotEqual);

    immediate(&mut code, x(0), schema.size());
    call_import(
        &mut code,
        imports.function(DarwinFileServiceFunction::Malloc),
    );
    require_nonzero(&mut code, x(0), imports)?;
    move_register(&mut code, x(21), x(0));
    zero_service(&mut code, x(21), &schema)?;
    create_native_roots(&mut code, x(21), &schema, imports)?;
    initialize_retirement_records(&mut code, x(21), &schema)?;
    code.append(Arm64Instruction::StoreRelease {
        size: Arm64DataSize::Bits64,
        source: Arm64DataRegister::General(x(21)),
        base: Arm64BaseRegister::General(x(20)),
    });
    move_register(&mut code, x(0), x(21));

    code.bind(ready)?;
    epilogue(&mut code);
    Ok(code)
}

fn zero_service(
    code: &mut Arm64CodeBuilder,
    service: Arm64Register,
    schema: &DarwinFileServiceAbiSchema,
) -> Result<(), Arm64DarwinFileServiceRootError> {
    if schema.size() == 0 || !schema.size().is_multiple_of(Arm64NocterAbi::word_size()) {
        return Err(Arm64DarwinFileServiceRootError::ContractLayout);
    }
    move_register(code, x(22), service);
    immediate(code, x(23), schema.size() / Arm64NocterAbi::word_size());
    immediate(code, x(8), 0);
    let loop_ = code.create_label();
    code.bind(loop_)?;
    store(code, Arm64LoadStoreSize::Double, x(22), 0, x(8))?;
    add_immediate(code, x(22), x(22), Arm64NocterAbi::word_size())?;
    subtract_immediate(code, x(23), x(23), 1);
    compare_immediate(code, x(23), 0);
    code.branch_conditional(loop_, Arm64BranchCondition::NotEqual);
    Ok(())
}

fn create_native_roots(
    code: &mut Arm64CodeBuilder,
    service: Arm64Register,
    schema: &DarwinFileServiceAbiSchema,
    imports: &Arm64DarwinFileServiceImports,
) -> Result<(), Arm64DarwinFileServiceRootError> {
    for index in 0..DarwinFileServiceField::OPERATION_QUEUES.len() {
        let offset = schema
            .operation_queue_offset(index)
            .ok_or(Arm64DarwinFileServiceRootError::ContractLayout)?;
        create_queue(code, service, offset, imports)?;
    }
    create_queue(
        code,
        service,
        schema.offset(DarwinFileServiceField::RetirementQueue),
        imports,
    )?;

    call_import(
        code,
        imports.function(DarwinFileServiceFunction::DispatchGroupCreate),
    );
    require_nonzero(code, x(0), imports)?;
    store(
        code,
        Arm64LoadStoreSize::Double,
        service,
        schema.offset(DarwinFileServiceField::WorkerGroup),
        x(0),
    )?;

    emit_system_call(code, DarwinSystemCall::Pipe);
    require_system_call_success(code, imports)?;
    move_register(code, x(22), x(0));
    move_register(code, x(23), x(1));
    configure_notification_descriptor(code, x(22), imports)?;
    configure_notification_descriptor(code, x(23), imports)?;
    store(
        code,
        Arm64LoadStoreSize::Double,
        service,
        schema.offset(DarwinFileServiceField::NotificationReader),
        x(22),
    )?;
    store(
        code,
        Arm64LoadStoreSize::Double,
        service,
        schema.offset(DarwinFileServiceField::NotificationWriter),
        x(23),
    )?;
    Ok(())
}

fn create_queue(
    code: &mut Arm64CodeBuilder,
    service: Arm64Register,
    offset: u64,
    imports: &Arm64DarwinFileServiceImports,
) -> Result<(), Arm64DarwinFileServiceRootError> {
    immediate(code, x(0), 0);
    immediate(code, x(1), 0);
    call_import(
        code,
        imports.function(DarwinFileServiceFunction::DispatchQueueCreate),
    );
    require_nonzero(code, x(0), imports)?;
    store(code, Arm64LoadStoreSize::Double, service, offset, x(0))?;
    Ok(())
}

fn configure_notification_descriptor(
    code: &mut Arm64CodeBuilder,
    descriptor: Arm64Register,
    imports: &Arm64DarwinFileServiceImports,
) -> Result<(), Arm64DarwinFileServiceRootError> {
    move_register(code, x(0), descriptor);
    immediate(code, x(1), DarwinDescriptorAbi::SET_DESCRIPTOR_FLAGS);
    immediate(code, x(2), DarwinDescriptorAbi::CLOSE_ON_EXEC);
    emit_system_call(code, DarwinSystemCall::Fcntl);
    require_system_call_success(code, imports)?;

    move_register(code, x(0), descriptor);
    immediate(code, x(1), DarwinDescriptorAbi::GET_STATUS_FLAGS);
    immediate(code, x(2), 0);
    emit_system_call(code, DarwinSystemCall::Fcntl);
    require_system_call_success(code, imports)?;
    immediate(code, x(8), DarwinDescriptorAbi::NONBLOCKING);
    code.append(Arm64Instruction::LogicalRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64Logical::Or,
        destination: Arm64DataRegister::General(x(2)),
        left: Arm64DataRegister::General(x(0)),
        right: Arm64DataRegister::General(x(8)),
    });
    move_register(code, x(0), descriptor);
    immediate(code, x(1), DarwinDescriptorAbi::SET_STATUS_FLAGS);
    emit_system_call(code, DarwinSystemCall::Fcntl);
    require_system_call_success(code, imports)?;
    Ok(())
}

fn initialize_retirement_records(
    code: &mut Arm64CodeBuilder,
    service: Arm64Register,
    schema: &DarwinFileServiceAbiSchema,
) -> Result<(), Arm64DarwinFileServiceRootError> {
    let record = schema.retirement_record();
    let asynchronous = record.asynchronous();
    let first = schema
        .retirement_record_offset(0)
        .ok_or(Arm64DarwinFileServiceRootError::ContractLayout)?;
    let count = u64::try_from(schema.retirement_record_count())
        .map_err(|_| Arm64DarwinFileServiceRootError::ContractLayout)?;
    if count == 0 {
        return Err(Arm64DarwinFileServiceRootError::ContractLayout);
    }
    address(code, x(22), service, first)?;
    immediate(code, x(23), count);
    load(
        code,
        Arm64LoadStoreSize::Double,
        x(24),
        service,
        schema.offset(DarwinFileServiceField::NotificationReader),
    )?;
    let loop_ = code.create_label();
    code.bind(loop_)?;
    store(
        code,
        Arm64LoadStoreSize::Double,
        x(22),
        record.offset(DarwinFileRetirementField::Service),
        service,
    )?;
    immediate(code, x(8), asynchronous.descriptor_interest_kind());
    store(
        code,
        Arm64LoadStoreSize::Double,
        x(22),
        record.offset(DarwinFileRetirementField::InterestKind),
        x(8),
    )?;
    store(
        code,
        Arm64LoadStoreSize::Double,
        x(22),
        record.offset(DarwinFileRetirementField::InterestSubject),
        x(24),
    )?;
    immediate(code, x(8), asynchronous.readable_interest_detail());
    store(
        code,
        Arm64LoadStoreSize::Double,
        x(22),
        record.offset(DarwinFileRetirementField::InterestDetail),
        x(8),
    )?;
    address(
        code,
        x(8),
        x(22),
        record.offset(DarwinFileRetirementField::Readiness),
    )?;
    store(
        code,
        Arm64LoadStoreSize::Double,
        x(22),
        record.offset(DarwinFileRetirementField::InterestReadinessPointer),
        x(8),
    )?;
    add_immediate(code, x(22), x(22), record.size())?;
    subtract_immediate(code, x(23), x(23), 1);
    compare_immediate(code, x(23), 0);
    code.branch_conditional(loop_, Arm64BranchCondition::NotEqual);
    Ok(())
}

fn build_shutdown(
    imports: &Arm64DarwinFileServiceImports,
) -> Result<Arm64CodeBuilder, Arm64DarwinFileServiceRootError> {
    let schema = DarwinFileServiceAbiSchema::ARM64_DARWIN;
    let process = Arm64NocterAbi::process_context();
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code);
    move_register(&mut code, x(19), x(0));
    address(
        &mut code,
        x(20),
        x(19),
        process.blocking_service_pointer_offset(),
    )?;
    code.append(Arm64Instruction::LoadAcquire {
        size: Arm64DataSize::Bits64,
        destination: Arm64DataRegister::General(x(21)),
        base: Arm64BaseRegister::General(x(20)),
    });
    let complete = code.create_label();
    compare_immediate(&mut code, x(21), 0);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);

    transition_service(
        &mut code,
        x(21),
        &schema,
        DarwinFileServiceState::Accepting,
        DarwinFileServiceEvent::BeginDrain,
        DarwinFileServiceAction::DrainWorkers,
        imports,
    )?;
    load(
        &mut code,
        Arm64LoadStoreSize::Double,
        x(0),
        x(21),
        schema.offset(DarwinFileServiceField::WorkerGroup),
    )?;
    immediate(&mut code, x(1), u64::MAX);
    call_import(
        &mut code,
        imports.function(DarwinFileServiceFunction::DispatchGroupWait),
    );
    compare_immediate(&mut code, x(0), 0);
    let drained = code.create_label();
    code.branch_conditional(drained, Arm64BranchCondition::Equal);
    abort(&mut code, imports);
    code.bind(drained)?;
    validate_drained(&mut code, x(21), &schema, imports)?;
    transition_service(
        &mut code,
        x(21),
        &schema,
        DarwinFileServiceState::Draining,
        DarwinFileServiceEvent::FinishDrain,
        DarwinFileServiceAction::ReleaseResources,
        imports,
    )?;
    release_dispatch_roots(&mut code, x(21), &schema, imports)?;
    close_notification_descriptors(&mut code, x(21), &schema)?;
    immediate(&mut code, x(8), 0);
    code.append(Arm64Instruction::StoreRelease {
        size: Arm64DataSize::Bits64,
        source: Arm64DataRegister::General(x(8)),
        base: Arm64BaseRegister::General(x(20)),
    });
    move_register(&mut code, x(0), x(21));
    call_import(&mut code, imports.function(DarwinFileServiceFunction::Free));
    code.bind(complete)?;
    epilogue(&mut code);
    Ok(code)
}

fn transition_service(
    code: &mut Arm64CodeBuilder,
    service: Arm64Register,
    schema: &DarwinFileServiceAbiSchema,
    state: DarwinFileServiceState,
    event: DarwinFileServiceEvent,
    expected_action: DarwinFileServiceAction,
    imports: &Arm64DarwinFileServiceImports,
) -> Result<(), Arm64DarwinFileServiceRootError> {
    address(
        code,
        x(22),
        service,
        schema.offset(DarwinFileServiceField::AdmissionState),
    )?;
    let success = code.create_label();
    let mismatch = code.create_label();
    let action = crate::emit_darwin_file_service_transition(
        code,
        Arm64AtomicUpdateRegisters::new(x(22), x(23), x(24))
            .expect("closed root registers do not alias"),
        state,
        event,
        success,
        mismatch,
    )?;
    if action != expected_action {
        return Err(Arm64DarwinFileServiceRootError::LifecycleAction);
    }
    code.bind(mismatch)?;
    abort(code, imports);
    code.bind(success)?;
    Ok(())
}

fn validate_drained(
    code: &mut Arm64CodeBuilder,
    service: Arm64Register,
    schema: &DarwinFileServiceAbiSchema,
    imports: &Arm64DarwinFileServiceImports,
) -> Result<(), Arm64DarwinFileServiceRootError> {
    load(
        code,
        Arm64LoadStoreSize::Double,
        x(22),
        service,
        schema.offset(DarwinFileServiceField::ActiveOperationCount),
    )?;
    compare_immediate(code, x(22), 0);
    let operations_drained = code.create_label();
    code.branch_conditional(operations_drained, Arm64BranchCondition::Equal);
    abort(code, imports);
    code.bind(operations_drained)?;

    let record = schema.retirement_record();
    let first = schema
        .retirement_record_offset(0)
        .ok_or(Arm64DarwinFileServiceRootError::ContractLayout)?;
    let count = u64::try_from(schema.retirement_record_count())
        .map_err(|_| Arm64DarwinFileServiceRootError::ContractLayout)?;
    if count == 0 {
        return Err(Arm64DarwinFileServiceRootError::ContractLayout);
    }
    address(code, x(22), service, first)?;
    immediate(code, x(23), count);
    let loop_ = code.create_label();
    let valid = code.create_label();
    code.bind(loop_)?;
    address(
        code,
        x(24),
        x(22),
        record.offset(DarwinFileRetirementField::LifecycleState),
    )?;
    code.append(Arm64Instruction::LoadAcquire {
        size: Arm64DataSize::Bits64,
        destination: Arm64DataRegister::General(x(8)),
        base: Arm64BaseRegister::General(x(24)),
    });
    compare_immediate(code, x(8), DarwinFileRetirementState::Available.code());
    code.branch_conditional(valid, Arm64BranchCondition::Equal);
    abort(code, imports);
    code.bind(valid)?;
    add_immediate(code, x(22), x(22), record.size())?;
    subtract_immediate(code, x(23), x(23), 1);
    compare_immediate(code, x(23), 0);
    code.branch_conditional(loop_, Arm64BranchCondition::NotEqual);
    Ok(())
}

fn release_dispatch_roots(
    code: &mut Arm64CodeBuilder,
    service: Arm64Register,
    schema: &DarwinFileServiceAbiSchema,
    imports: &Arm64DarwinFileServiceImports,
) -> Result<(), Arm64DarwinFileServiceRootError> {
    for field in DarwinFileServiceField::OPERATION_QUEUES
        .iter()
        .copied()
        .chain([DarwinFileServiceField::RetirementQueue])
    {
        load(
            code,
            Arm64LoadStoreSize::Double,
            x(0),
            service,
            schema.offset(field),
        )?;
        call_import(
            code,
            imports.function(DarwinFileServiceFunction::DispatchRelease),
        );
    }
    load(
        code,
        Arm64LoadStoreSize::Double,
        x(0),
        service,
        schema.offset(DarwinFileServiceField::WorkerGroup),
    )?;
    call_import(
        code,
        imports.function(DarwinFileServiceFunction::DispatchRelease),
    );
    Ok(())
}

fn close_notification_descriptors(
    code: &mut Arm64CodeBuilder,
    service: Arm64Register,
    schema: &DarwinFileServiceAbiSchema,
) -> Result<(), Arm64DarwinFileServiceRootError> {
    for field in [
        DarwinFileServiceField::NotificationReader,
        DarwinFileServiceField::NotificationWriter,
    ] {
        load(
            code,
            Arm64LoadStoreSize::Double,
            x(0),
            service,
            schema.offset(field),
        )?;
        emit_system_call(code, DarwinSystemCall::Close);
    }
    Ok(())
}

fn require_nonzero(
    code: &mut Arm64CodeBuilder,
    value: Arm64Register,
    imports: &Arm64DarwinFileServiceImports,
) -> Result<(), Arm64CodeError> {
    let valid = code.create_label();
    compare_immediate(code, value, 0);
    code.branch_conditional(valid, Arm64BranchCondition::NotEqual);
    abort(code, imports);
    code.bind(valid)
}

fn require_system_call_success(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinFileServiceImports,
) -> Result<(), Arm64CodeError> {
    let success = code.create_label();
    code.branch_conditional(success, Arm64BranchCondition::CarryClear);
    abort(code, imports);
    code.bind(success)
}

fn prologue(code: &mut Arm64CodeBuilder) {
    adjust_stack(code, Arm64AddSubtract::Subtract);
    for (register, offset) in [(19, 16), (20, 24), (21, 32), (22, 40), (23, 48), (24, 56)] {
        store_stack(
            code,
            Arm64LoadStoreSize::Double,
            offset,
            Arm64DataRegister::General(x(register)),
        );
    }
    store_stack(
        code,
        Arm64LoadStoreSize::Double,
        8,
        Arm64DataRegister::General(x(30)),
    );
}

fn epilogue(code: &mut Arm64CodeBuilder) {
    for (register, offset) in [(19, 16), (20, 24), (21, 32), (22, 40), (23, 48), (24, 56)] {
        load_stack(code, Arm64LoadStoreSize::Double, x(register), offset);
    }
    load_stack(code, Arm64LoadStoreSize::Double, x(30), 8);
    adjust_stack(code, Arm64AddSubtract::Add);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
}

fn adjust_stack(code: &mut Arm64CodeBuilder, operation: Arm64AddSubtract) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation,
        set_flags: false,
        destination: Arm64AddSubtractDestination::StackPointer,
        source: Arm64BaseRegister::StackPointer,
        immediate: STACK_SIZE,
        shift_12: false,
    });
}

fn address(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    base: Arm64Register,
    offset: u64,
) -> Result<(), Arm64DarwinFileServiceRootError> {
    add_immediate(code, destination, base, offset)
}

fn add_immediate(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    source: Arm64Register,
    value: u64,
) -> Result<(), Arm64DarwinFileServiceRootError> {
    let value =
        u16::try_from(value).map_err(|_| Arm64DarwinFileServiceRootError::ContractLayout)?;
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::General(source),
        immediate: value,
        shift_12: false,
    });
    Ok(())
}

fn subtract_immediate(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    source: Arm64Register,
    value: u16,
) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::General(source),
        immediate: value,
        shift_12: false,
    });
}

fn move_register(code: &mut Arm64CodeBuilder, destination: Arm64Register, source: Arm64Register) {
    if destination != source {
        code.append(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Add,
            set_flags: false,
            destination: Arm64AddSubtractDestination::General(destination),
            source: Arm64BaseRegister::General(source),
            immediate: 0,
            shift_12: false,
        });
    }
}

fn immediate(code: &mut Arm64CodeBuilder, destination: Arm64Register, value: u64) {
    crate::frame_access::load_immediate(code, destination, value, Arm64DataSize::Bits64);
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

fn store(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    base: Arm64Register,
    offset: u64,
    source: Arm64Register,
) -> Result<(), Arm64DarwinFileServiceRootError> {
    code.append(Arm64Instruction::StoreUnsigned {
        size,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::General(base),
        offset: u32::try_from(offset)
            .map_err(|_| Arm64DarwinFileServiceRootError::ContractLayout)?,
    });
    Ok(())
}

fn load(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    destination: Arm64Register,
    base: Arm64Register,
    offset: u64,
) -> Result<(), Arm64DarwinFileServiceRootError> {
    code.append(Arm64Instruction::LoadUnsigned {
        size,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(base),
        offset: u32::try_from(offset)
            .map_err(|_| Arm64DarwinFileServiceRootError::ContractLayout)?,
    });
    Ok(())
}

fn store_stack(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    offset: u32,
    source: Arm64DataRegister,
) {
    code.append(Arm64Instruction::StoreUnsigned {
        size,
        source,
        base: Arm64BaseRegister::StackPointer,
        offset,
    });
}

fn load_stack(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    destination: Arm64Register,
    offset: u32,
) {
    code.append(Arm64Instruction::LoadUnsigned {
        size,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::StackPointer,
        offset,
    });
}

fn call_import(code: &mut Arm64CodeBuilder, target: crate::Arm64FunctionImportId) {
    code.load_function_import(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
}

fn abort(code: &mut Arm64CodeBuilder, imports: &Arm64DarwinFileServiceImports) {
    call_import(code, imports.function(DarwinFileServiceFunction::Abort));
}

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).expect("closed ARM64 register is valid")
}

#[derive(Debug)]
pub enum Arm64DarwinFileServiceRootError {
    ContractLayout,
    LifecycleAction,
    Lifecycle(Arm64DarwinFileLifecycleError),
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinFileServiceRootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin file-service root failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinFileServiceRootError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lifecycle(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ContractLayout | Self::LifecycleAction => None,
        }
    }
}

impl From<Arm64DarwinFileLifecycleError> for Arm64DarwinFileServiceRootError {
    fn from(error: Arm64DarwinFileLifecycleError) -> Self {
        Self::Lifecycle(error)
    }
}

impl From<Arm64CodeError> for Arm64DarwinFileServiceRootError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinFileServiceRootError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}

#[cfg(test)]
mod tests {
    use nocter_runtime_contract::DarwinFileServiceFunction;

    use super::Arm64DarwinFileServiceRootTargets;
    use crate::{Arm64DarwinFileServiceImports, Arm64ProgramBuilder};

    #[test]
    fn root_targets_are_defined_together_from_the_typed_import_catalog() {
        let mut program = Arm64ProgramBuilder::new();
        let imports = Arm64DarwinFileServiceImports::declare(&mut program).unwrap();
        let targets = Arm64DarwinFileServiceRootTargets::declare(&mut program, &imports).unwrap();
        assert_ne!(targets.ensure(), targets.shutdown());
        program.set_entry(targets.ensure()).unwrap();
        let program = program.finish().unwrap();
        assert!(program.function(targets.ensure()).unwrap().size() > 0);
        assert!(program.function(targets.shutdown()).unwrap().size() > 0);
        assert_eq!(
            program.runtime_imports().len(),
            DarwinFileServiceFunction::ALL.len()
        );
    }
}
