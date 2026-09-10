use std::fmt;

use nocter_runtime_contract::{
    DarwinNetworkAdapterFunction, DarwinNetworkAdapterOperation,
    DarwinNetworkCallbackEventAbiSchema, DarwinNetworkEventKind,
    DarwinNetworkListenerEventObservationAbiSchema, DarwinNetworkListenerEventPollAbiSchema,
    DarwinNetworkListenerState, DarwinNetworkOwnerAbiSchema, DarwinNetworkOwnerCreateStatus,
    DarwinNetworkOwnerField, DarwinNetworkOwnerKind,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DarwinAcceptedConnectionAdoptionTarget,
    Arm64DarwinNetworkAdapterImports, Arm64DarwinNetworkChannelError,
    Arm64DarwinNetworkErrorConsumptionError, Arm64DarwinNetworkOwnerError, Arm64DataRegister,
    Arm64DataSize, Arm64FunctionId, Arm64Instruction, Arm64LoadStoreSize, Arm64ProgramBuilder,
    Arm64ProgramError, Arm64Register, emit_darwin_network_consume_error,
    emit_darwin_network_event_receive_to_pointer, emit_darwin_network_event_try_receive_to_pointer,
    emit_darwin_network_owner_guard, emit_darwin_network_owner_transition,
};

/// The sole native target that consumes listener events into source-owned values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinNetworkListenerEventTargets {
    receive: Arm64FunctionId,
    try_receive: Arm64FunctionId,
}

impl Arm64DarwinNetworkListenerEventTargets {
    #[must_use]
    pub const fn receive(self) -> Arm64FunctionId {
        self.receive
    }

    #[must_use]
    pub const fn try_receive(self) -> Arm64FunctionId {
        self.try_receive
    }
}

/// Adds an atomic listener event consumer.
///
/// The target accepts a listener owner in `x0`, consumes exactly one callback record, and writes
/// [`DarwinNetworkListenerEventObservationAbiSchema::ARM64_DARWIN`] through result register `x8`.
/// An accepted native connection is consumed by the typed adoption target before it can cross the
/// result boundary.
///
/// # Errors
///
/// Propagates malformed runtime contracts and ARM64 program/code construction failures.
pub(crate) fn add_darwin_network_listener_event_target(
    program: &mut Arm64ProgramBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    adoption: Arm64DarwinAcceptedConnectionAdoptionTarget,
) -> Result<Arm64DarwinNetworkListenerEventTargets, Arm64DarwinNetworkListenerEventError> {
    let receive = program.declare_function();
    program.define_function(
        receive,
        receive_code(
            imports,
            adoption,
            DarwinNetworkListenerEventObservationAbiSchema::ARM64_DARWIN,
            None,
        )?
        .finish()?,
    )?;
    let poll = DarwinNetworkListenerEventPollAbiSchema::ARM64_DARWIN;
    let try_receive = program.declare_function();
    program.define_function(
        try_receive,
        receive_code(
            imports,
            adoption,
            poll.observation(),
            Some(poll.available_offset()),
        )?
        .finish()?,
    )?;
    Ok(Arm64DarwinNetworkListenerEventTargets {
        receive,
        try_receive,
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "event receipt, retained-object consumption, and result publication are one transaction"
)]
fn receive_code(
    imports: &Arm64DarwinNetworkAdapterImports,
    adoption: Arm64DarwinAcceptedConnectionAdoptionTarget,
    observation: DarwinNetworkListenerEventObservationAbiSchema,
    availability_offset: Option<u64>,
) -> Result<Arm64CodeBuilder, Arm64DarwinNetworkListenerEventError> {
    const FRAME_SIZE: u16 = 112;
    let saved = [
        (x(19), 48),
        (x(20), 56),
        (x(21), 64),
        (x(22), 72),
        (x(23), 80),
        (x(28), 88),
        (x(24), 96),
        (x(30), 104),
    ];
    let event = DarwinNetworkCallbackEventAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, FRAME_SIZE);
    for (register, offset) in saved {
        store_stack(&mut code, register, offset);
    }
    move_register(&mut code, x(19), x(0));
    move_register(&mut code, x(28), x(8));
    emit_darwin_network_owner_guard(
        &mut code,
        x(19),
        DarwinNetworkOwnerKind::Listener,
        if availability_offset.is_some() {
            DarwinNetworkAdapterOperation::TryReceiveEvent
        } else {
            DarwinNetworkAdapterOperation::ReceiveEvent
        },
        imports,
    )?;
    clear_observation(&mut code, x(28), observation)?;
    if let Some(offset) = availability_offset {
        store_zero_at(&mut code, x(28), offset)?;
    }
    load_owner_field(
        &mut code,
        x(21),
        x(19),
        DarwinNetworkOwnerField::EventReader,
    )?;
    stack_address(&mut code, x(20), 0);
    let no_event = availability_offset.map(|_| code.create_label());
    if let Some(offset) = availability_offset {
        emit_darwin_network_event_try_receive_to_pointer(
            &mut code,
            imports.channel(),
            x(21),
            x(20),
            x(22),
        )?;
        store_at(&mut code, x(28), offset, x(22))?;
        compare_immediate(&mut code, x(22), 0)?;
        code.branch_conditional(
            no_event.expect("poll mode owns the no-event label"),
            Arm64BranchCondition::Equal,
        );
    } else {
        emit_darwin_network_event_receive_to_pointer(&mut code, imports.channel(), x(21), x(20))?;
    }
    immediate(&mut code, x(21), 1)?;
    store_at(&mut code, x(28), observation.accepted_tag_offset(), x(21))?;
    load_event_field(&mut code, x(21), x(20), event.kind_offset())?;
    store_at(&mut code, x(28), observation.kind_offset(), x(21))?;

    let state = code.create_label();
    let accepted = code.create_label();
    let complete = code.create_label();
    branch_if_kind(
        &mut code,
        x(21),
        DarwinNetworkEventKind::ListenerState,
        state,
    )?;
    branch_if_kind(
        &mut code,
        x(21),
        DarwinNetworkEventKind::AcceptedConnection,
        accepted,
    )?;
    abort(&mut code, imports);

    code.bind(state)?;
    consume_state_event(&mut code, imports, event, observation, complete)?;

    code.bind(accepted)?;
    load_event_field(&mut code, x(22), x(20), event_payload_offset(event, 0)?)?;
    add_immediate(&mut code, x(0), x(28), observation.accepted_owner_offset())?;
    move_register(&mut code, x(1), x(22));
    call_function(&mut code, adoption.function());
    store_at(
        &mut code,
        x(28),
        observation
            .value_offset(0)
            .ok_or(Arm64DarwinNetworkListenerEventError::ContractLayout)?,
        x(0),
    )?;
    compare_immediate(
        &mut code,
        x(0),
        DarwinNetworkOwnerCreateStatus::Created.code(),
    )?;
    code.branch_conditional(complete, Arm64BranchCondition::NotEqual);
    store_zero_at(&mut code, x(28), observation.accepted_tag_offset())?;

    code.bind(complete)?;
    clear_event(&mut code, x(20), event)?;
    if let Some(no_event) = no_event {
        code.bind(no_event)?;
    }
    for (register, offset) in saved {
        load_stack(&mut code, register, offset);
    }
    adjust_stack(&mut code, Arm64AddSubtract::Add, FRAME_SIZE);
    return_from_function(&mut code);
    Ok(code)
}

fn consume_state_event(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    event: DarwinNetworkCallbackEventAbiSchema,
    observation: DarwinNetworkListenerEventObservationAbiSchema,
    complete: crate::Arm64LabelId,
) -> Result<(), Arm64DarwinNetworkListenerEventError> {
    load_event_field(code, x(21), x(20), event_payload_offset(event, 0)?)?;
    validate_listener_state(code, x(21), imports)?;
    load_event_field(code, x(22), x(20), event_payload_offset(event, 1)?)?;
    emit_darwin_network_consume_error(code, imports, x(22), x(23), x(24))?;
    let not_final = code.create_label();
    compare_immediate(code, x(21), DarwinNetworkListenerState::Cancelled.code())?;
    code.branch_conditional(not_final, Arm64BranchCondition::NotEqual);
    emit_darwin_network_owner_transition(
        code,
        x(19),
        DarwinNetworkOwnerKind::Listener,
        DarwinNetworkAdapterOperation::ObserveFinalState,
        imports,
    )?;
    code.bind(not_final)?;
    for (lane, value) in [x(21), x(23), x(24)].into_iter().enumerate() {
        store_at(
            code,
            x(28),
            observation
                .value_offset(lane)
                .ok_or(Arm64DarwinNetworkListenerEventError::ContractLayout)?,
            value,
        )?;
    }
    code.branch(complete, false);
    Ok(())
}

fn validate_listener_state(
    code: &mut Arm64CodeBuilder,
    state: Arm64Register,
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<(), Arm64DarwinNetworkListenerEventError> {
    let valid = code.create_label();
    for candidate in DarwinNetworkListenerState::ALL {
        compare_immediate(code, state, candidate.code())?;
        code.branch_conditional(valid, Arm64BranchCondition::Equal);
    }
    abort(code, imports);
    code.bind(valid)?;
    Ok(())
}

fn clear_observation(
    code: &mut Arm64CodeBuilder,
    result: Arm64Register,
    schema: DarwinNetworkListenerEventObservationAbiSchema,
) -> Result<(), Arm64DarwinNetworkListenerEventError> {
    let words = schema
        .size()
        .checked_div(8)
        .ok_or(Arm64DarwinNetworkListenerEventError::ContractLayout)?;
    for word in 0..words {
        store_zero_at(code, result, word * 8)?;
    }
    Ok(())
}

fn clear_event(
    code: &mut Arm64CodeBuilder,
    event: Arm64Register,
    schema: DarwinNetworkCallbackEventAbiSchema,
) -> Result<(), Arm64DarwinNetworkListenerEventError> {
    store_zero_at(code, event, schema.kind_offset())?;
    for lane in 0..4 {
        store_zero_at(code, event, event_payload_offset(schema, lane)?)?;
    }
    Ok(())
}

fn branch_if_kind(
    code: &mut Arm64CodeBuilder,
    actual: Arm64Register,
    expected: DarwinNetworkEventKind,
    target: crate::Arm64LabelId,
) -> Result<(), Arm64DarwinNetworkListenerEventError> {
    compare_immediate(code, actual, expected.code())?;
    code.branch_conditional(target, Arm64BranchCondition::Equal);
    Ok(())
}

fn load_owner_field(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    owner: Arm64Register,
    field: DarwinNetworkOwnerField,
) -> Result<(), Arm64DarwinNetworkListenerEventError> {
    load_event_field(
        code,
        destination,
        owner,
        DarwinNetworkOwnerAbiSchema::ARM64_DARWIN.offset(field),
    )
}

fn load_event_field(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    base: Arm64Register,
    offset: u64,
) -> Result<(), Arm64DarwinNetworkListenerEventError> {
    let offset =
        u32::try_from(offset).map_err(|_| Arm64DarwinNetworkListenerEventError::ContractLayout)?;
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(base),
        offset,
    });
    Ok(())
}

fn store_at(
    code: &mut Arm64CodeBuilder,
    base: Arm64Register,
    offset: u64,
    source: Arm64Register,
) -> Result<(), Arm64DarwinNetworkListenerEventError> {
    let offset =
        u32::try_from(offset).map_err(|_| Arm64DarwinNetworkListenerEventError::ContractLayout)?;
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::General(base),
        offset,
    });
    Ok(())
}

fn store_zero_at(
    code: &mut Arm64CodeBuilder,
    base: Arm64Register,
    offset: u64,
) -> Result<(), Arm64DarwinNetworkListenerEventError> {
    let offset =
        u32::try_from(offset).map_err(|_| Arm64DarwinNetworkListenerEventError::ContractLayout)?;
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::Zero,
        base: Arm64BaseRegister::General(base),
        offset,
    });
    Ok(())
}

fn event_payload_offset(
    schema: DarwinNetworkCallbackEventAbiSchema,
    lane: usize,
) -> Result<u64, Arm64DarwinNetworkListenerEventError> {
    schema
        .payload_offset(lane)
        .ok_or(Arm64DarwinNetworkListenerEventError::ContractLayout)
}

fn immediate(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    value: u64,
) -> Result<(), Arm64DarwinNetworkListenerEventError> {
    let immediate =
        u16::try_from(value).map_err(|_| Arm64DarwinNetworkListenerEventError::ContractLayout)?;
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: crate::Arm64MoveWide::Zero,
        destination,
        immediate,
        shift: 0,
    });
    Ok(())
}

fn compare_immediate(
    code: &mut Arm64CodeBuilder,
    value: Arm64Register,
    expected: u64,
) -> Result<(), Arm64DarwinNetworkListenerEventError> {
    let immediate = u16::try_from(expected)
        .map_err(|_| Arm64DarwinNetworkListenerEventError::ContractLayout)?;
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate,
        shift_12: false,
    });
    Ok(())
}

fn add_immediate(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    source: Arm64Register,
    value: u64,
) -> Result<(), Arm64DarwinNetworkListenerEventError> {
    let immediate =
        u16::try_from(value).map_err(|_| Arm64DarwinNetworkListenerEventError::ContractLayout)?;
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::General(source),
        immediate,
        shift_12: false,
    });
    Ok(())
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

fn call_import(code: &mut Arm64CodeBuilder, target: crate::Arm64FunctionImportId) {
    code.load_function_import(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
}

fn call_function(code: &mut Arm64CodeBuilder, target: Arm64FunctionId) {
    code.load_function_address(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
}

fn abort(code: &mut Arm64CodeBuilder, imports: &Arm64DarwinNetworkAdapterImports) {
    call_import(code, imports.function(DarwinNetworkAdapterFunction::Abort));
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

fn return_from_function(code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
}

const fn x(number: u8) -> Arm64Register {
    match Arm64Register::new(number) {
        Some(register) => register,
        None => panic!("closed ARM64 register is valid"),
    }
}

#[derive(Debug)]
pub enum Arm64DarwinNetworkListenerEventError {
    ContractLayout,
    Owner(Arm64DarwinNetworkOwnerError),
    Channel(Arm64DarwinNetworkChannelError),
    EventError(Arm64DarwinNetworkErrorConsumptionError),
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinNetworkListenerEventError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin listener event failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinNetworkListenerEventError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Owner(error) => Some(error),
            Self::Channel(error) => Some(error),
            Self::EventError(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ContractLayout => None,
        }
    }
}

macro_rules! convert_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for Arm64DarwinNetworkListenerEventError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}

convert_error!(Arm64DarwinNetworkOwnerError, Owner);
convert_error!(Arm64DarwinNetworkChannelError, Channel);
convert_error!(Arm64DarwinNetworkErrorConsumptionError, EventError);
convert_error!(Arm64CodeError, Code);
convert_error!(Arm64ProgramError, Program);
