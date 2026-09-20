use std::fmt;

use nocter_runtime_contract::{DarwinEventAbiSchema, DarwinTerminationFunction, PrimitiveRole};

use crate::darwin_kernel_abi::{DarwinSystemCall, emit_system_call};
use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DataRegister, Arm64DataSize, Arm64FunctionId,
    Arm64FunctionImportId, Arm64Instruction, Arm64LabelId, Arm64LoadStoreSize, Arm64NocterAbi,
    Arm64ProgramBuilder, Arm64ProgramError, Arm64Register,
};

const INTERRUPT_SIGNAL: u64 = 2;
const TERMINATE_SIGNAL: u64 = 15;
const IGNORE_HANDLER: u64 = 1;
const SIGNAL_ERROR: u64 = u64::MAX;
const INVALID_ARGUMENT: u64 = 22;
const WOULD_BLOCK: u64 = 35;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Arm64DarwinTerminationImports {
    signal: Arm64FunctionImportId,
}

impl Arm64DarwinTerminationImports {
    fn declare(program: &mut Arm64ProgramBuilder) -> Result<Self, Arm64ProgramError> {
        Ok(Self {
            signal: program.add_function_import(DarwinTerminationFunction::Signal.import())?,
        })
    }
}

/// Compiler-owned process-lifetime termination observation entry points.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinTerminationTargets {
    descriptor: Arm64FunctionId,
    observe: Arm64FunctionId,
    finalize: Arm64FunctionId,
}

impl Arm64DarwinTerminationTargets {
    pub(crate) fn declare(
        roles: &std::collections::BTreeSet<PrimitiveRole>,
        program: &mut Arm64ProgramBuilder,
    ) -> Result<Option<Self>, Arm64DarwinTerminationError> {
        if !roles.contains(&PrimitiveRole::ProcessTerminationDescriptor)
            && !roles.contains(&PrimitiveRole::ProcessTerminationObserve)
        {
            return Ok(None);
        }
        let imports = Arm64DarwinTerminationImports::declare(program)?;
        let descriptor = program.declare_function();
        let observe = program.declare_function();
        let finalize = program.declare_function();
        program.define_function(descriptor, descriptor_entry(&imports)?.finish()?)?;
        program.define_function(observe, observe_entry(descriptor)?.finish()?)?;
        program.define_function(finalize, finalize_entry(&imports)?.finish()?)?;
        Ok(Some(Self {
            descriptor,
            observe,
            finalize,
        }))
    }

    pub(crate) const fn descriptor(self) -> Arm64FunctionId {
        self.descriptor
    }

    pub(crate) const fn observe(self) -> Arm64FunctionId {
        self.observe
    }

    pub(crate) const fn finalize(self) -> Arm64FunctionId {
        self.finalize
    }
}

fn descriptor_entry(
    imports: &Arm64DarwinTerminationImports,
) -> Result<Arm64CodeBuilder, Arm64DarwinTerminationError> {
    const FRAME_SIZE: u16 = 96;
    let context = Arm64NocterAbi::process_context();
    let mut code = Arm64CodeBuilder::new();
    let saved = [
        (x(19), 48),
        (x(20), 56),
        (x(21), 64),
        (x(22), 72),
        (x(10), 80),
    ];
    prologue(&mut code, FRAME_SIZE, &saved);

    load_context(&mut code, x(19), context.termination_descriptor_offset());
    compare_immediate(&mut code, x(19), SIGNAL_ERROR);
    let initialize = code.create_label();
    let success = code.create_label();
    code.branch_conditional(initialize, Arm64BranchCondition::Equal);
    code.branch(success, false);
    code.bind(initialize)?;

    call_signal(&mut code, imports.signal, INTERRUPT_SIGNAL, IGNORE_HANDLER);
    move_register(&mut code, x(20), x(0));
    compare_immediate(&mut code, x(20), SIGNAL_ERROR);
    let interrupt_ready = code.create_label();
    code.branch_conditional(interrupt_ready, Arm64BranchCondition::NotEqual);
    fail(&mut code, INVALID_ARGUMENT, FRAME_SIZE, &saved);
    code.bind(interrupt_ready)?;

    call_signal(&mut code, imports.signal, TERMINATE_SIGNAL, IGNORE_HANDLER);
    move_register(&mut code, x(21), x(0));
    compare_immediate(&mut code, x(21), SIGNAL_ERROR);
    let dispositions_ready = code.create_label();
    code.branch_conditional(dispositions_ready, Arm64BranchCondition::NotEqual);
    call_signal(&mut code, imports.signal, INTERRUPT_SIGNAL, x_value(20));
    fail(&mut code, INVALID_ARGUMENT, FRAME_SIZE, &saved);
    code.bind(dispositions_ready)?;

    emit_system_call(&mut code, DarwinSystemCall::Kqueue);
    let queue_ready = code.create_label();
    code.branch_conditional(queue_ready, Arm64BranchCondition::CarryClear);
    move_register(&mut code, x(19), x(0));
    restore_dispositions(&mut code, imports, x(20), x(21));
    fail_from_register(&mut code, x(19), FRAME_SIZE, &saved);
    code.bind(queue_ready)?;
    move_register(&mut code, x(19), x(0));
    move_register(&mut code, x(0), x(19));
    immediate(
        &mut code,
        x(1),
        crate::darwin_kernel_abi::DarwinDescriptorAbi::SET_DESCRIPTOR_FLAGS,
    );
    immediate(
        &mut code,
        x(2),
        crate::darwin_kernel_abi::DarwinDescriptorAbi::CLOSE_ON_EXEC,
    );
    emit_system_call(&mut code, DarwinSystemCall::Fcntl);
    let descriptor_configured = code.create_label();
    code.branch_conditional(descriptor_configured, Arm64BranchCondition::CarryClear);
    move_register(&mut code, x(22), x(0));
    move_register(&mut code, x(0), x(19));
    emit_system_call(&mut code, DarwinSystemCall::Close);
    restore_dispositions(&mut code, imports, x(20), x(21));
    fail_from_register(&mut code, x(22), FRAME_SIZE, &saved);
    code.bind(descriptor_configured)?;

    let registration_failed = code.create_label();
    register_signal(&mut code, x(19), INTERRUPT_SIGNAL, registration_failed);
    register_signal(&mut code, x(19), TERMINATE_SIGNAL, registration_failed);
    load_stack(&mut code, Arm64LoadStoreSize::Double, x(10), 80);
    store_context(&mut code, context.termination_descriptor_offset(), x(19));
    store_context(
        &mut code,
        context.termination_interrupt_handler_offset(),
        x(20),
    );
    store_context(
        &mut code,
        context.termination_terminate_handler_offset(),
        x(21),
    );
    code.branch(success, false);

    code.bind(registration_failed)?;
    move_register(&mut code, x(22), x(0));
    move_register(&mut code, x(0), x(19));
    emit_system_call(&mut code, DarwinSystemCall::Close);
    restore_dispositions(&mut code, imports, x(20), x(21));
    fail_from_register(&mut code, x(22), FRAME_SIZE, &saved);

    code.bind(success)?;
    move_register(&mut code, x(0), x(19));
    immediate(&mut code, x(1), 0);
    epilogue(&mut code, FRAME_SIZE, &saved);
    Ok(code)
}

fn observe_entry(
    descriptor: Arm64FunctionId,
) -> Result<Arm64CodeBuilder, Arm64DarwinTerminationError> {
    const FRAME_SIZE: u16 = 96;
    let context = Arm64NocterAbi::process_context();
    let saved = [(x(19), 64)];
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code, FRAME_SIZE, &saved);
    load_context(
        &mut code,
        x(0),
        context.termination_observed_signal_offset(),
    );
    compare_immediate(&mut code, x(0), 0);
    let initialize = code.create_label();
    code.branch_conditional(initialize, Arm64BranchCondition::Equal);
    immediate(&mut code, x(1), 0);
    epilogue(&mut code, FRAME_SIZE, &saved);
    code.bind(initialize)?;

    code.call(descriptor);
    compare_immediate(&mut code, x(1), 0);
    let descriptor_ready = code.create_label();
    code.branch_conditional(descriptor_ready, Arm64BranchCondition::Equal);
    epilogue(&mut code, FRAME_SIZE, &saved);
    code.bind(descriptor_ready)?;
    move_register(&mut code, x(19), x(0));

    immediate(&mut code, x(0), 0);
    store_stack(&mut code, Arm64LoadStoreSize::Double, x(0), 48);
    store_stack(&mut code, Arm64LoadStoreSize::Double, x(0), 56);
    move_register(&mut code, x(0), x(19));
    immediate(&mut code, x(1), 0);
    immediate(&mut code, x(2), 0);
    stack_address(&mut code, x(3), 0);
    immediate(&mut code, x(4), 1);
    immediate(&mut code, x(5), 0);
    stack_address(&mut code, x(6), 48);
    emit_system_call(&mut code, DarwinSystemCall::Kevent64);
    let observed = code.create_label();
    code.branch_conditional(observed, Arm64BranchCondition::CarryClear);
    move_register(&mut code, x(1), x(0));
    immediate(&mut code, x(0), 0);
    epilogue(&mut code, FRAME_SIZE, &saved);
    code.bind(observed)?;
    compare_immediate(&mut code, x(0), 0);
    let one_event = code.create_label();
    code.branch_conditional(one_event, Arm64BranchCondition::NotEqual);
    immediate(&mut code, x(0), 0);
    immediate(&mut code, x(1), WOULD_BLOCK);
    epilogue(&mut code, FRAME_SIZE, &saved);
    code.bind(one_event)?;
    compare_immediate(&mut code, x(0), 1);
    let count_valid = code.create_label();
    code.branch_conditional(count_valid, Arm64BranchCondition::Equal);
    fail(&mut code, INVALID_ARGUMENT, FRAME_SIZE, &saved);
    code.bind(count_valid)?;
    load_stack(&mut code, Arm64LoadStoreSize::Double, x(0), 0);
    compare_immediate(&mut code, x(0), INTERRUPT_SIGNAL);
    let signal_valid = code.create_label();
    code.branch_conditional(signal_valid, Arm64BranchCondition::Equal);
    compare_immediate(&mut code, x(0), TERMINATE_SIGNAL);
    code.branch_conditional(signal_valid, Arm64BranchCondition::Equal);
    fail(&mut code, INVALID_ARGUMENT, FRAME_SIZE, &saved);
    code.bind(signal_valid)?;
    store_context(
        &mut code,
        context.termination_observed_signal_offset(),
        x(0),
    );
    immediate(&mut code, x(1), 0);
    epilogue(&mut code, FRAME_SIZE, &saved);
    Ok(code)
}

fn finalize_entry(
    imports: &Arm64DarwinTerminationImports,
) -> Result<Arm64CodeBuilder, Arm64DarwinTerminationError> {
    const FRAME_SIZE: u16 = 48;
    let context = Arm64NocterAbi::process_context();
    let saved = [(x(19), 0), (x(20), 8), (x(21), 16)];
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code, FRAME_SIZE, &saved);
    move_register(&mut code, x(19), x(0));
    load_at(
        &mut code,
        x(20),
        x(19),
        context.termination_descriptor_offset(),
    );
    compare_immediate(&mut code, x(20), SIGNAL_ERROR);
    let complete = code.create_label();
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    move_register(&mut code, x(0), x(20));
    emit_system_call(&mut code, DarwinSystemCall::Close);
    load_at(
        &mut code,
        x(0),
        x(19),
        context.termination_interrupt_handler_offset(),
    );
    move_register(&mut code, x(21), x(0));
    call_signal(&mut code, imports.signal, INTERRUPT_SIGNAL, x_value(21));
    load_at(
        &mut code,
        x(21),
        x(19),
        context.termination_terminate_handler_offset(),
    );
    call_signal(&mut code, imports.signal, TERMINATE_SIGNAL, x_value(21));
    immediate(&mut code, x(0), SIGNAL_ERROR);
    store_at(
        &mut code,
        x(19),
        context.termination_descriptor_offset(),
        x(0),
    );
    code.bind(complete)?;
    epilogue(&mut code, FRAME_SIZE, &saved);
    Ok(code)
}

fn register_signal(
    code: &mut Arm64CodeBuilder,
    descriptor: Arm64Register,
    signal: u64,
    failure: Arm64LabelId,
) {
    let event = DarwinEventAbiSchema::ARM64_DARWIN;
    immediate(code, x(0), signal);
    store_stack(code, Arm64LoadStoreSize::Double, x(0), event.ident_offset());
    immediate(
        code,
        x(0),
        u64::from_ne_bytes(i64::from(event.signal_filter()).to_ne_bytes()),
    );
    store_stack(code, Arm64LoadStoreSize::Half, x(0), event.filter_offset());
    immediate(code, x(0), u64::from(event.add_flag()));
    store_stack(code, Arm64LoadStoreSize::Half, x(0), event.flags_offset());
    immediate(code, x(0), 0);
    store_stack(
        code,
        Arm64LoadStoreSize::Word,
        x(0),
        event.filter_flags_offset(),
    );
    store_stack(code, Arm64LoadStoreSize::Double, x(0), event.data_offset());
    store_stack(
        code,
        Arm64LoadStoreSize::Double,
        x(0),
        event.user_data_offset(),
    );
    store_stack(
        code,
        Arm64LoadStoreSize::Double,
        x(0),
        event.extension_zero_offset(),
    );
    store_stack(
        code,
        Arm64LoadStoreSize::Double,
        x(0),
        event.extension_one_offset(),
    );
    move_register(code, x(0), descriptor);
    stack_address(code, x(1), 0);
    immediate(code, x(2), 1);
    for register in 3..=6 {
        immediate(code, x(register), 0);
    }
    emit_system_call(code, DarwinSystemCall::Kevent64);
    code.branch_conditional(failure, Arm64BranchCondition::CarrySet);
    compare_immediate(code, x(0), 0);
    code.branch_conditional(failure, Arm64BranchCondition::NotEqual);
}

enum SignalHandler {
    Immediate(u64),
    Register(Arm64Register),
}

fn x_value(register: u8) -> SignalHandler {
    SignalHandler::Register(x(register))
}

impl From<u64> for SignalHandler {
    fn from(value: u64) -> Self {
        Self::Immediate(value)
    }
}

fn call_signal(
    code: &mut Arm64CodeBuilder,
    import: Arm64FunctionImportId,
    signal: u64,
    handler: impl Into<SignalHandler>,
) {
    immediate(code, x(0), signal);
    match handler.into() {
        SignalHandler::Immediate(value) => immediate(code, x(1), value),
        SignalHandler::Register(register) => move_register(code, x(1), register),
    }
    code.load_function_import(import, x(8));
    code.append(Arm64Instruction::BranchRegister {
        target: x(8),
        link: true,
    });
}

fn restore_dispositions(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinTerminationImports,
    interrupt: Arm64Register,
    terminate: Arm64Register,
) {
    call_signal(
        code,
        imports.signal,
        INTERRUPT_SIGNAL,
        SignalHandler::Register(interrupt),
    );
    call_signal(
        code,
        imports.signal,
        TERMINATE_SIGNAL,
        SignalHandler::Register(terminate),
    );
}

fn fail(code: &mut Arm64CodeBuilder, errno: u64, frame: u16, saved: &[(Arm64Register, u64)]) {
    immediate(code, x(0), 0);
    immediate(code, x(1), errno);
    epilogue(code, frame, saved);
}

fn fail_from_register(
    code: &mut Arm64CodeBuilder,
    errno: Arm64Register,
    frame: u16,
    saved: &[(Arm64Register, u64)],
) {
    move_register(code, x(1), errno);
    immediate(code, x(0), 0);
    epilogue(code, frame, saved);
}

fn prologue(code: &mut Arm64CodeBuilder, frame: u16, saved: &[(Arm64Register, u64)]) {
    crate::frame_access::adjust_stack(code, u64::from(frame), Arm64AddSubtract::Subtract);
    for (register, offset) in saved {
        store_stack(code, Arm64LoadStoreSize::Double, *register, *offset);
    }
    store_stack(
        code,
        Arm64LoadStoreSize::Double,
        x(30),
        u64::from(frame) - 8,
    );
}

fn epilogue(code: &mut Arm64CodeBuilder, frame: u16, saved: &[(Arm64Register, u64)]) {
    for (register, offset) in saved {
        load_stack(code, Arm64LoadStoreSize::Double, *register, *offset);
    }
    load_stack(
        code,
        Arm64LoadStoreSize::Double,
        x(30),
        u64::from(frame) - 8,
    );
    crate::frame_access::adjust_stack(code, u64::from(frame), Arm64AddSubtract::Add);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
}

fn load_context(code: &mut Arm64CodeBuilder, destination: Arm64Register, offset: u64) {
    load_at(
        code,
        destination,
        Arm64NocterAbi::process_context_register(),
        offset,
    );
}

fn store_context(code: &mut Arm64CodeBuilder, offset: u64, source: Arm64Register) {
    store_at(
        code,
        Arm64NocterAbi::process_context_register(),
        offset,
        source,
    );
}

fn load_at(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    base: Arm64Register,
    offset: u64,
) {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(base),
        offset: u32::try_from(offset).expect("termination context offset fits load"),
    });
}

fn store_at(code: &mut Arm64CodeBuilder, base: Arm64Register, offset: u64, source: Arm64Register) {
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::General(base),
        offset: u32::try_from(offset).expect("termination context offset fits store"),
    });
}

fn store_stack(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    source: Arm64Register,
    offset: u64,
) {
    crate::frame_access::store_at_stack_offset(code, size, source, offset);
}

fn load_stack(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    destination: Arm64Register,
    offset: u64,
) {
    crate::frame_access::load_at_stack_offset(code, size, destination, offset);
}

fn stack_address(code: &mut Arm64CodeBuilder, destination: Arm64Register, offset: u64) {
    crate::frame_access::form_stack_address(code, destination, offset);
}

fn immediate(code: &mut Arm64CodeBuilder, destination: Arm64Register, value: u64) {
    crate::frame_access::load_immediate(code, destination, value, Arm64DataSize::Bits64);
}

fn move_register(code: &mut Arm64CodeBuilder, destination: Arm64Register, source: Arm64Register) {
    if destination == source {
        return;
    }
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

fn compare_immediate(code: &mut Arm64CodeBuilder, value: Arm64Register, immediate_value: u64) {
    immediate(code, x(8), immediate_value);
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(value),
        right: Arm64DataRegister::General(x(8)),
    });
}

fn x(index: u8) -> Arm64Register {
    Arm64Register::new(index).expect("termination service uses only general registers")
}

#[derive(Debug)]
pub enum Arm64DarwinTerminationError {
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinTerminationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Darwin termination service generation failed: {self:?}"
        )
    }
}

impl std::error::Error for Arm64DarwinTerminationError {}

impl From<Arm64CodeError> for Arm64DarwinTerminationError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinTerminationError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}
