use std::fmt;

use nocter_runtime_contract::{
    DarwinNetworkAdapterFunction, DarwinNetworkAdapterOperation, DarwinNetworkConnectionState,
    DarwinNetworkEventKind, DarwinNetworkListenerEventObservationAbiSchema,
    DarwinNetworkListenerState, DarwinNetworkOwnerAbiSchema, DarwinNetworkOwnerCreateStatus,
    DarwinNetworkOwnerField, DarwinNetworkOwnerKind, DarwinNetworkOwnerState,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DarwinNetworkAdapterImports,
    Arm64DarwinNetworkConnectionEventTargets, Arm64DarwinNetworkListenerEventTargets,
    Arm64DarwinNetworkOwnerError, Arm64DarwinNetworkOwnerLifecycleTargets, Arm64DataRegister,
    Arm64DataSize, Arm64FunctionId, Arm64Instruction, Arm64LoadStoreSize, Arm64ProgramBuilder,
    Arm64ProgramError, Arm64Register, emit_darwin_network_owner_guard,
};

/// One nonblocking ownership-consuming entry and its private blocking cleanup worker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinNetworkOwnerDisposalTarget {
    dispose: Arm64FunctionId,
    worker: Arm64FunctionId,
}

impl Arm64DarwinNetworkOwnerDisposalTarget {
    #[must_use]
    pub const fn dispose(self) -> Arm64FunctionId {
        self.dispose
    }

    #[must_use]
    pub const fn worker(self) -> Arm64FunctionId {
        self.worker
    }
}

/// Adds a connection-owner disposal entry.
///
/// The public target copies the consumed inline owner into one runtime allocation and submits its
/// private worker to a global concurrent queue. The worker may wait for the terminal callback and
/// the owner's serial-queue barrier; invocation itself never waits for either source of progress.
pub(crate) fn add_darwin_network_connection_disposal_target(
    program: &mut Arm64ProgramBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    lifecycle: Arm64DarwinNetworkOwnerLifecycleTargets,
    events: Arm64DarwinNetworkConnectionEventTargets,
) -> Result<Arm64DarwinNetworkOwnerDisposalTarget, Arm64DarwinNetworkOwnerDisposalError> {
    let dispose = program.declare_function();
    let worker = program.declare_function();
    program.define_function(
        worker,
        connection_worker(imports, lifecycle, events)?.finish()?,
    )?;
    program.define_function(
        dispose,
        disposal_entry(imports, DarwinNetworkOwnerKind::Connection, worker)?.finish()?,
    )?;
    Ok(Arm64DarwinNetworkOwnerDisposalTarget { dispose, worker })
}

/// Adds a listener-owner disposal entry.
///
/// Accepted connections already queued before listener cancellation are transferred to the same
/// connection disposal boundary. No retained provider object is discarded by the worker.
pub(crate) fn add_darwin_network_listener_disposal_target(
    program: &mut Arm64ProgramBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    lifecycle: Arm64DarwinNetworkOwnerLifecycleTargets,
    events: Arm64DarwinNetworkListenerEventTargets,
    connection_dispose: Arm64FunctionId,
) -> Result<Arm64DarwinNetworkOwnerDisposalTarget, Arm64DarwinNetworkOwnerDisposalError> {
    let dispose = program.declare_function();
    let worker = program.declare_function();
    program.define_function(
        worker,
        listener_worker(imports, lifecycle, events, connection_dispose)?.finish()?,
    )?;
    program.define_function(
        dispose,
        disposal_entry(imports, DarwinNetworkOwnerKind::Listener, worker)?.finish()?,
    )?;
    Ok(Arm64DarwinNetworkOwnerDisposalTarget { dispose, worker })
}

fn disposal_entry(
    imports: &Arm64DarwinNetworkAdapterImports,
    kind: DarwinNetworkOwnerKind,
    worker: Arm64FunctionId,
) -> Result<Arm64CodeBuilder, Arm64DarwinNetworkOwnerDisposalError> {
    const FRAME_SIZE: u16 = 32;
    let schema = DarwinNetworkOwnerAbiSchema::ARM64_DARWIN;
    let owner_size = u16::try_from(schema.size())
        .map_err(|_| Arm64DarwinNetworkOwnerDisposalError::ContractLayout)?;
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, FRAME_SIZE);
    store_stack(&mut code, x(19), 0);
    store_stack(&mut code, x(20), 8);
    store_stack(&mut code, x(30), 24);
    move_register(&mut code, x(19), x(0));
    emit_darwin_network_owner_guard(
        &mut code,
        x(19),
        kind,
        DarwinNetworkAdapterOperation::Dispose,
        imports,
    )?;

    immediate(&mut code, x(0), owner_size);
    call_import(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::Malloc),
    );
    let allocated = code.create_label();
    compare_immediate(&mut code, x(0), 0);
    code.branch_conditional(allocated, Arm64BranchCondition::NotEqual);
    abort(&mut code, imports);
    code.bind(allocated)?;
    move_register(&mut code, x(20), x(0));
    for offset in (0..schema.size()).step_by(8) {
        load_at(&mut code, x(8), x(19), offset)?;
        store_at(&mut code, x(20), offset, x(8))?;
    }

    immediate(&mut code, x(0), 0);
    immediate(&mut code, x(1), 0);
    call_import(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::DispatchGetGlobalQueue),
    );
    let queue_available = code.create_label();
    compare_immediate(&mut code, x(0), 0);
    code.branch_conditional(queue_available, Arm64BranchCondition::NotEqual);
    abort(&mut code, imports);
    code.bind(queue_available)?;
    move_register(&mut code, x(1), x(20));
    code.load_function_address(worker, x(2));
    call_import(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::DispatchAsyncFunction),
    );

    load_stack(&mut code, x(19), 0);
    load_stack(&mut code, x(20), 8);
    load_stack(&mut code, x(30), 24);
    adjust_stack(&mut code, Arm64AddSubtract::Add, FRAME_SIZE);
    return_from_function(&mut code);
    Ok(code)
}

fn connection_worker(
    imports: &Arm64DarwinNetworkAdapterImports,
    lifecycle: Arm64DarwinNetworkOwnerLifecycleTargets,
    events: Arm64DarwinNetworkConnectionEventTargets,
) -> Result<Arm64CodeBuilder, Arm64DarwinNetworkOwnerDisposalError> {
    const FRAME_SIZE: u16 = 96;
    const OBSERVATION_OFFSET: u16 = 0;
    let mut code = Arm64CodeBuilder::new();
    worker_prologue(&mut code, FRAME_SIZE, 48, 56, 88);
    start_if_initialized(&mut code, lifecycle)?;
    call_owner(&mut code, lifecycle.request_cancel());
    let receive = code.create_label();
    code.bind(receive)?;
    move_register(&mut code, x(0), x(19));
    immediate(&mut code, x(1), 1);
    immediate(&mut code, x(2), 0);
    stack_address(&mut code, x(8), OBSERVATION_OFFSET);
    call_function(&mut code, events.receive());
    load_stack(&mut code, x(20), OBSERVATION_OFFSET.into());
    compare_immediate(
        &mut code,
        x(20),
        DarwinNetworkEventKind::ConnectionState.code(),
    );
    code.branch_conditional(receive, Arm64BranchCondition::NotEqual);
    load_stack(&mut code, x(20), u32::from(OBSERVATION_OFFSET) + 8);
    compare_immediate(
        &mut code,
        x(20),
        DarwinNetworkConnectionState::Cancelled.code(),
    );
    code.branch_conditional(receive, Arm64BranchCondition::NotEqual);
    finish_worker(&mut code, imports, lifecycle, FRAME_SIZE, 48, 56, 88);
    Ok(code)
}

fn listener_worker(
    imports: &Arm64DarwinNetworkAdapterImports,
    lifecycle: Arm64DarwinNetworkOwnerLifecycleTargets,
    events: Arm64DarwinNetworkListenerEventTargets,
    connection_dispose: Arm64FunctionId,
) -> Result<Arm64CodeBuilder, Arm64DarwinNetworkOwnerDisposalError> {
    const FRAME_SIZE: u16 = 128;
    const OWNER_SAVE: u32 = 96;
    const VALUE_SAVE: u32 = 104;
    const LINK_SAVE: u32 = 120;
    let observation = DarwinNetworkListenerEventObservationAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    worker_prologue(&mut code, FRAME_SIZE, OWNER_SAVE, VALUE_SAVE, LINK_SAVE);
    start_if_initialized(&mut code, lifecycle)?;
    call_owner(&mut code, lifecycle.request_cancel());

    let receive = code.create_label();
    let accepted = code.create_label();
    let cancelled = code.create_label();
    code.bind(receive)?;
    move_register(&mut code, x(0), x(19));
    stack_address(&mut code, x(8), 0);
    call_function(&mut code, events.receive());
    load_stack(
        &mut code,
        x(20),
        u32::try_from(observation.kind_offset())
            .map_err(|_| Arm64DarwinNetworkOwnerDisposalError::ContractLayout)?,
    );
    compare_immediate(
        &mut code,
        x(20),
        DarwinNetworkEventKind::AcceptedConnection.code(),
    );
    code.branch_conditional(accepted, Arm64BranchCondition::Equal);
    compare_immediate(
        &mut code,
        x(20),
        DarwinNetworkEventKind::ListenerState.code(),
    );
    code.branch_conditional(receive, Arm64BranchCondition::NotEqual);
    load_stack(
        &mut code,
        x(20),
        u32::try_from(
            observation
                .value_offset(0)
                .ok_or(Arm64DarwinNetworkOwnerDisposalError::ContractLayout)?,
        )
        .map_err(|_| Arm64DarwinNetworkOwnerDisposalError::ContractLayout)?,
    );
    compare_immediate(
        &mut code,
        x(20),
        DarwinNetworkListenerState::Cancelled.code(),
    );
    code.branch_conditional(cancelled, Arm64BranchCondition::Equal);
    code.branch(receive, false);

    code.bind(accepted)?;
    load_stack(
        &mut code,
        x(20),
        u32::try_from(
            observation
                .value_offset(0)
                .ok_or(Arm64DarwinNetworkOwnerDisposalError::ContractLayout)?,
        )
        .map_err(|_| Arm64DarwinNetworkOwnerDisposalError::ContractLayout)?,
    );
    compare_immediate(
        &mut code,
        x(20),
        DarwinNetworkOwnerCreateStatus::Created.code(),
    );
    code.branch_conditional(receive, Arm64BranchCondition::NotEqual);
    stack_address(
        &mut code,
        x(0),
        u16::try_from(observation.accepted_owner_offset())
            .map_err(|_| Arm64DarwinNetworkOwnerDisposalError::ContractLayout)?,
    );
    call_function(&mut code, connection_dispose);
    code.branch(receive, false);

    code.bind(cancelled)?;
    finish_worker(
        &mut code, imports, lifecycle, FRAME_SIZE, OWNER_SAVE, VALUE_SAVE, LINK_SAVE,
    );
    Ok(code)
}

fn start_if_initialized(
    code: &mut Arm64CodeBuilder,
    lifecycle: Arm64DarwinNetworkOwnerLifecycleTargets,
) -> Result<(), Arm64DarwinNetworkOwnerDisposalError> {
    load_at(
        code,
        x(20),
        x(19),
        DarwinNetworkOwnerAbiSchema::ARM64_DARWIN.offset(DarwinNetworkOwnerField::Lifecycle),
    )?;
    compare_immediate(code, x(20), DarwinNetworkOwnerState::Initialized.code());
    let initialized = code.create_label();
    code.branch_conditional(initialized, Arm64BranchCondition::NotEqual);
    call_owner(code, lifecycle.start());
    code.bind(initialized)?;
    Ok(())
}

fn finish_worker(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    lifecycle: Arm64DarwinNetworkOwnerLifecycleTargets,
    frame_size: u16,
    owner_save: u32,
    value_save: u32,
    link_save: u32,
) {
    call_owner(code, lifecycle.complete_release_barrier());
    call_owner(code, lifecycle.release());
    move_register(code, x(0), x(19));
    call_import(code, imports.function(DarwinNetworkAdapterFunction::Free));
    load_stack(code, x(19), owner_save);
    load_stack(code, x(20), value_save);
    load_stack(code, x(30), link_save);
    adjust_stack(code, Arm64AddSubtract::Add, frame_size);
    return_from_function(code);
}

fn worker_prologue(
    code: &mut Arm64CodeBuilder,
    frame_size: u16,
    owner_save: u32,
    value_save: u32,
    link_save: u32,
) {
    adjust_stack(code, Arm64AddSubtract::Subtract, frame_size);
    store_stack(code, x(19), owner_save);
    store_stack(code, x(20), value_save);
    store_stack(code, x(30), link_save);
    move_register(code, x(19), x(0));
}

fn call_owner(code: &mut Arm64CodeBuilder, target: Arm64FunctionId) {
    move_register(code, x(0), x(19));
    call_function(code, target);
}

fn adjust_stack(code: &mut Arm64CodeBuilder, operation: Arm64AddSubtract, amount: u16) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation,
        set_flags: false,
        destination: Arm64AddSubtractDestination::StackPointer,
        source: Arm64BaseRegister::StackPointer,
        immediate: amount,
        shift_12: false,
    });
}

fn stack_address(code: &mut Arm64CodeBuilder, destination: Arm64Register, offset: u16) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::StackPointer,
        immediate: offset,
        shift_12: false,
    });
}

fn move_register(code: &mut Arm64CodeBuilder, destination: Arm64Register, source: Arm64Register) {
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

fn immediate(code: &mut Arm64CodeBuilder, destination: Arm64Register, value: u16) {
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: crate::Arm64MoveWide::Zero,
        destination,
        immediate: value,
        shift: 0,
    });
}

fn compare_immediate(code: &mut Arm64CodeBuilder, value: Arm64Register, expected: u64) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: u16::try_from(expected).expect("closed network code fits an ARM64 immediate"),
        shift_12: false,
    });
}

fn load_at(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    base: Arm64Register,
    offset: u64,
) -> Result<(), Arm64DarwinNetworkOwnerDisposalError> {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(base),
        offset: u32::try_from(offset)
            .map_err(|_| Arm64DarwinNetworkOwnerDisposalError::ContractLayout)?,
    });
    Ok(())
}

fn store_at(
    code: &mut Arm64CodeBuilder,
    base: Arm64Register,
    offset: u64,
    source: Arm64Register,
) -> Result<(), Arm64DarwinNetworkOwnerDisposalError> {
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::General(base),
        offset: u32::try_from(offset)
            .map_err(|_| Arm64DarwinNetworkOwnerDisposalError::ContractLayout)?,
    });
    Ok(())
}

fn store_stack(code: &mut Arm64CodeBuilder, source: Arm64Register, offset: u32) {
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::StackPointer,
        offset,
    });
}

fn load_stack(code: &mut Arm64CodeBuilder, destination: Arm64Register, offset: u32) {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::StackPointer,
        offset,
    });
}

fn call_function(code: &mut Arm64CodeBuilder, target: Arm64FunctionId) {
    code.load_function_address(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
}

fn call_import(code: &mut Arm64CodeBuilder, target: crate::Arm64FunctionImportId) {
    code.load_function_import(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
}

fn abort(code: &mut Arm64CodeBuilder, imports: &Arm64DarwinNetworkAdapterImports) {
    call_import(code, imports.function(DarwinNetworkAdapterFunction::Abort));
}

fn return_from_function(code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
}

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).expect("closed ARM64 register is valid")
}

#[derive(Debug)]
pub enum Arm64DarwinNetworkOwnerDisposalError {
    ContractLayout,
    Owner(Arm64DarwinNetworkOwnerError),
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinNetworkOwnerDisposalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ARM64 Darwin network owner disposal failed: {self:?}"
        )
    }
}

impl std::error::Error for Arm64DarwinNetworkOwnerDisposalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Owner(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ContractLayout => None,
        }
    }
}

impl From<Arm64DarwinNetworkOwnerError> for Arm64DarwinNetworkOwnerDisposalError {
    fn from(error: Arm64DarwinNetworkOwnerError) -> Self {
        Self::Owner(error)
    }
}

impl From<Arm64CodeError> for Arm64DarwinNetworkOwnerDisposalError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinNetworkOwnerDisposalError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}
