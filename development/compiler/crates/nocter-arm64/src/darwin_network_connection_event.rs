use std::fmt;

use nocter_runtime_contract::{
    DarwinNetworkAdapterFunction, DarwinNetworkAdapterOperation,
    DarwinNetworkCallbackEventAbiSchema, DarwinNetworkConnectionEventObservationAbiSchema,
    DarwinNetworkConnectionState, DarwinNetworkEventKind, DarwinNetworkOwnerAbiSchema,
    DarwinNetworkOwnerField, DarwinNetworkOwnerKind,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DarwinNetworkAdapterImports,
    Arm64DarwinNetworkChannelError, Arm64DarwinNetworkOwnerError, Arm64DataRegister, Arm64DataSize,
    Arm64FunctionId, Arm64Instruction, Arm64LoadStoreSize, Arm64ProgramBuilder, Arm64ProgramError,
    Arm64Register, emit_darwin_network_event_receive_to_pointer, emit_darwin_network_owner_guard,
    emit_darwin_network_owner_transition,
};

/// Native callable targets for observing and consuming connection events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinNetworkConnectionEventTargets {
    descriptor: Arm64FunctionId,
    receive: Arm64FunctionId,
}

impl Arm64DarwinNetworkConnectionEventTargets {
    #[must_use]
    pub const fn descriptor(self) -> Arm64FunctionId {
        self.descriptor
    }

    #[must_use]
    pub const fn receive(self) -> Arm64FunctionId {
        self.receive
    }
}

/// Adds the only production targets that expose the callback channel of a connection owner.
///
/// `descriptor` accepts the owner address in `x0` and returns the reactor-readable descriptor in
/// `x0`. `receive` accepts `(owner, destination, capacity)` in `x0..x2`, consumes exactly one
/// ordered callback event, and writes a native-object-free five-word observation to the caller
/// storage addressed by the Nocter ABI result register `x8`.
///
/// The receive target owns event-kind, state, and boolean validation; dispatch-data copying;
/// retained-object release; record clearing; and the final-state owner transition. No caller can
/// observe or forget an ownership-bearing native event record.
///
/// # Errors
///
/// Propagates malformed runtime contracts and ARM64 program/code construction failures.
pub(crate) fn add_darwin_network_connection_event_targets(
    program: &mut Arm64ProgramBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<Arm64DarwinNetworkConnectionEventTargets, Arm64DarwinNetworkConnectionEventError> {
    let descriptor = program.declare_function();
    let receive = program.declare_function();
    program.define_function(descriptor, descriptor_code(imports)?)?;
    program.define_function(receive, receive_code(imports)?)?;
    Ok(Arm64DarwinNetworkConnectionEventTargets {
        descriptor,
        receive,
    })
}

fn descriptor_code(
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<crate::Arm64Code, Arm64DarwinNetworkConnectionEventError> {
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, 16);
    store_stack(&mut code, x(19), 0);
    store_stack(&mut code, x(30), 8);
    move_register(&mut code, x(19), x(0));
    emit_darwin_network_owner_guard(
        &mut code,
        x(19),
        DarwinNetworkOwnerKind::Connection,
        DarwinNetworkAdapterOperation::EventDescriptor,
        imports,
    )?;
    load_owner_field(&mut code, x(0), x(19), DarwinNetworkOwnerField::EventReader)?;
    load_stack(&mut code, x(19), 0);
    load_stack(&mut code, x(30), 8);
    adjust_stack(&mut code, Arm64AddSubtract::Add, 16);
    return_from_function(&mut code);
    code.finish().map_err(Into::into)
}

#[allow(
    clippy::too_many_lines,
    reason = "one ordered event read and its complete ownership dispatch must remain atomic"
)]
fn receive_code(
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<crate::Arm64Code, Arm64DarwinNetworkConnectionEventError> {
    const FRAME_SIZE: u16 = 160;
    const SAVED: [(Arm64Register, u32); 11] = [
        (x(19), 56),
        (x(20), 64),
        (x(21), 72),
        (x(22), 80),
        (x(23), 88),
        (x(24), 96),
        (x(25), 104),
        (x(26), 112),
        (x(27), 120),
        (x(28), 128),
        (x(30), 152),
    ];
    let schema = DarwinNetworkCallbackEventAbiSchema::ARM64_DARWIN;
    let observation = DarwinNetworkConnectionEventObservationAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, FRAME_SIZE);
    for (register, offset) in SAVED {
        store_stack(&mut code, register, offset);
    }
    move_register(&mut code, x(19), x(0));
    move_register(&mut code, x(26), x(1));
    move_register(&mut code, x(27), x(2));
    move_register(&mut code, x(28), x(8));
    emit_darwin_network_owner_guard(
        &mut code,
        x(19),
        DarwinNetworkOwnerKind::Connection,
        DarwinNetworkAdapterOperation::ReceiveEvent,
        imports,
    )?;
    load_owner_field(
        &mut code,
        x(21),
        x(19),
        DarwinNetworkOwnerField::EventReader,
    )?;
    stack_address(&mut code, x(20), 0);
    emit_darwin_network_event_receive_to_pointer(&mut code, imports.channel(), x(21), x(20))?;

    clear_observation(&mut code, x(28), observation)?;
    load_event_field(&mut code, x(21), x(20), schema.kind_offset())?;
    store_at(&mut code, x(28), observation.kind_offset(), x(21))?;

    let state = code.create_label();
    let receive = code.create_label();
    let send = code.create_label();
    let complete = code.create_label();
    branch_if_kind(
        &mut code,
        x(21),
        DarwinNetworkEventKind::ConnectionState,
        state,
    )?;
    branch_if_kind(
        &mut code,
        x(21),
        DarwinNetworkEventKind::ReceiveCompletion,
        receive,
    )?;
    branch_if_kind(
        &mut code,
        x(21),
        DarwinNetworkEventKind::SendCompletion,
        send,
    )?;
    abort(&mut code, imports);

    code.bind(state)?;
    consume_state_event(&mut code, imports, schema, observation, complete)?;

    code.bind(receive)?;
    consume_receive_event(&mut code, imports, schema, observation, complete)?;

    code.bind(send)?;
    consume_send_event(&mut code, imports, schema, observation)?;
    code.bind(complete)?;
    clear_event(&mut code, x(20), schema)?;
    for (register, offset) in SAVED {
        load_stack(&mut code, register, offset);
    }
    adjust_stack(&mut code, Arm64AddSubtract::Add, FRAME_SIZE);
    return_from_function(&mut code);
    code.finish().map_err(Into::into)
}

fn consume_state_event(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    schema: DarwinNetworkCallbackEventAbiSchema,
    observation: DarwinNetworkConnectionEventObservationAbiSchema,
    complete: crate::Arm64LabelId,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    load_event_field(code, x(21), x(20), event_payload_offset(schema, 0)?)?;
    validate_connection_state(code, x(21), imports)?;
    load_event_field(code, x(22), x(20), event_payload_offset(schema, 1)?)?;
    consume_error(code, imports, x(22), x(23), x(24))?;
    let not_final = code.create_label();
    compare_immediate(code, x(21), DarwinNetworkConnectionState::Cancelled.code())?;
    code.branch_conditional(not_final, Arm64BranchCondition::NotEqual);
    emit_darwin_network_owner_transition(
        code,
        x(19),
        DarwinNetworkOwnerKind::Connection,
        DarwinNetworkAdapterOperation::ObserveFinalState,
        imports,
    )?;
    code.bind(not_final)?;
    store_observation_values(
        code,
        observation,
        [Some(x(21)), Some(x(23)), Some(x(24)), None],
    )?;
    code.branch(complete, false);
    Ok(())
}

fn consume_receive_event(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    schema: DarwinNetworkCallbackEventAbiSchema,
    observation: DarwinNetworkConnectionEventObservationAbiSchema,
    complete: crate::Arm64LabelId,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    load_event_field(code, x(22), x(20), event_payload_offset(schema, 0)?)?;
    load_event_field(code, x(23), x(20), event_payload_offset(schema, 1)?)?;
    load_event_field(code, x(24), x(20), event_payload_offset(schema, 2)?)?;
    load_event_field(code, x(25), x(20), event_payload_offset(schema, 3)?)?;
    validate_boolean(code, x(24), imports)?;
    store_stack(code, x(24), 136);
    release_optional_network_object(code, imports, x(25))?;
    consume_error(code, imports, x(23), x(21), x(25))?;
    copy_dispatch_data(code, imports, x(22), x(26), x(27), x(23))?;
    load_stack(code, x(19), 136);
    store_observation_values(
        code,
        observation,
        [Some(x(23)), Some(x(19)), Some(x(21)), Some(x(25))],
    )?;
    code.branch(complete, false);
    Ok(())
}

fn consume_send_event(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    schema: DarwinNetworkCallbackEventAbiSchema,
    observation: DarwinNetworkConnectionEventObservationAbiSchema,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    load_event_field(code, x(22), x(20), event_payload_offset(schema, 0)?)?;
    consume_error(code, imports, x(22), x(23), x(24))?;
    store_observation_values(code, observation, [Some(x(23)), Some(x(24)), None, None])?;
    Ok(())
}

fn branch_if_kind(
    code: &mut Arm64CodeBuilder,
    actual: Arm64Register,
    expected: DarwinNetworkEventKind,
    target: crate::Arm64LabelId,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    compare_immediate(code, actual, expected.code())?;
    code.branch_conditional(target, Arm64BranchCondition::Equal);
    Ok(())
}

fn consume_error(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    error: Arm64Register,
    domain: Arm64Register,
    error_code: Arm64Register,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    immediate(code, domain, 0)?;
    immediate(code, error_code, 0)?;
    let consumed = code.create_label();
    compare_immediate(code, error, 0)?;
    code.branch_conditional(consumed, Arm64BranchCondition::Equal);
    move_register(code, x(0), error);
    call_import(
        code,
        imports.function(DarwinNetworkAdapterFunction::NetworkErrorGetDomain),
    );
    move_register(code, domain, x(0));
    move_register(code, x(0), error);
    call_import(
        code,
        imports.function(DarwinNetworkAdapterFunction::NetworkErrorGetCode),
    );
    move_register(code, error_code, x(0));
    move_register(code, x(0), error);
    call_import(
        code,
        imports.function(DarwinNetworkAdapterFunction::NetworkRelease),
    );
    code.bind(consumed)?;
    Ok(())
}

fn release_optional_network_object(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    object: Arm64Register,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    let released = code.create_label();
    compare_immediate(code, object, 0)?;
    code.branch_conditional(released, Arm64BranchCondition::Equal);
    move_register(code, x(0), object);
    call_import(
        code,
        imports.function(DarwinNetworkAdapterFunction::NetworkRelease),
    );
    code.bind(released)?;
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "the emitter makes dispatch-data ownership and destination bounds explicit"
)]
fn copy_dispatch_data(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    content: Arm64Register,
    destination: Arm64Register,
    capacity: Arm64Register,
    copied: Arm64Register,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    immediate(code, copied, 0)?;
    let complete = code.create_label();
    compare_immediate(code, content, 0)?;
    code.branch_conditional(complete, Arm64BranchCondition::Equal);

    stack_address(code, x(1), 40);
    stack_address(code, x(2), 48);
    move_register(code, x(0), content);
    call_import(
        code,
        imports.function(DarwinNetworkAdapterFunction::DispatchDataCreateMap),
    );
    move_register(code, x(24), x(0));
    let mapped = code.create_label();
    compare_immediate(code, x(24), 0)?;
    code.branch_conditional(mapped, Arm64BranchCondition::NotEqual);
    release_dispatch_data(code, imports, content);
    abort(code, imports);
    code.bind(mapped)?;

    load_stack(code, x(19), 40);
    load_stack(code, copied, 48);
    let release = code.create_label();
    compare_immediate(code, capacity, 0)?;
    code.branch_conditional(release, Arm64BranchCondition::Equal);
    compare_register(code, copied, capacity);
    let fits = code.create_label();
    code.branch_conditional(fits, Arm64BranchCondition::UnsignedLowerOrSame);
    release_dispatch_data(code, imports, x(24));
    release_dispatch_data(code, imports, content);
    abort(code, imports);
    code.bind(fits)?;
    copy_bytes(code, x(19), destination, copied)?;
    code.bind(release)?;
    release_dispatch_data(code, imports, x(24));
    release_dispatch_data(code, imports, content);
    code.bind(complete)?;
    Ok(())
}

fn copy_bytes(
    code: &mut Arm64CodeBuilder,
    source: Arm64Register,
    destination: Arm64Register,
    length: Arm64Register,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    immediate(code, x(8), 0)?;
    let next = code.create_label();
    let complete = code.create_label();
    code.bind(next)?;
    compare_register(code, x(8), length);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    add_register(code, x(9), source, x(8));
    load_byte(code, x(10), x(9));
    add_register(code, x(9), destination, x(8));
    store_byte(code, x(9), x(10));
    add_small_immediate(code, x(8), x(8), 1);
    code.branch(next, false);
    code.bind(complete)?;
    Ok(())
}

fn release_dispatch_data(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    object: Arm64Register,
) {
    move_register(code, x(0), object);
    call_import(
        code,
        imports.function(DarwinNetworkAdapterFunction::DispatchRelease),
    );
}

fn abort(code: &mut Arm64CodeBuilder, imports: &Arm64DarwinNetworkAdapterImports) {
    call_import(code, imports.function(DarwinNetworkAdapterFunction::Abort));
}

fn clear_observation(
    code: &mut Arm64CodeBuilder,
    result: Arm64Register,
    schema: DarwinNetworkConnectionEventObservationAbiSchema,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    store_zero_at(code, result, schema.kind_offset())?;
    for lane in 0..4 {
        store_zero_at(
            code,
            result,
            schema
                .value_offset(lane)
                .ok_or(Arm64DarwinNetworkConnectionEventError::ContractLayout)?,
        )?;
    }
    Ok(())
}

fn store_observation_values(
    code: &mut Arm64CodeBuilder,
    schema: DarwinNetworkConnectionEventObservationAbiSchema,
    values: [Option<Arm64Register>; 4],
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    for (lane, value) in values.into_iter().enumerate() {
        if let Some(value) = value {
            store_at(
                code,
                x(28),
                schema
                    .value_offset(lane)
                    .ok_or(Arm64DarwinNetworkConnectionEventError::ContractLayout)?,
                value,
            )?;
        }
    }
    Ok(())
}

fn validate_boolean(
    code: &mut Arm64CodeBuilder,
    value: Arm64Register,
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    let valid = code.create_label();
    compare_immediate(code, value, 0)?;
    code.branch_conditional(valid, Arm64BranchCondition::Equal);
    compare_immediate(code, value, 1)?;
    code.branch_conditional(valid, Arm64BranchCondition::Equal);
    abort(code, imports);
    code.bind(valid)?;
    Ok(())
}

fn validate_connection_state(
    code: &mut Arm64CodeBuilder,
    state: Arm64Register,
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    let valid = code.create_label();
    for candidate in DarwinNetworkConnectionState::ALL {
        compare_immediate(code, state, candidate.code())?;
        code.branch_conditional(valid, Arm64BranchCondition::Equal);
    }
    call_import(code, imports.function(DarwinNetworkAdapterFunction::Abort));
    code.bind(valid)?;
    Ok(())
}

fn clear_event(
    code: &mut Arm64CodeBuilder,
    event: Arm64Register,
    schema: DarwinNetworkCallbackEventAbiSchema,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    store_zero_at(code, event, schema.kind_offset())?;
    for lane in 0..4 {
        store_zero_at(code, event, event_payload_offset(schema, lane)?)?;
    }
    Ok(())
}

fn event_payload_offset(
    schema: DarwinNetworkCallbackEventAbiSchema,
    lane: usize,
) -> Result<u64, Arm64DarwinNetworkConnectionEventError> {
    schema
        .payload_offset(lane)
        .ok_or(Arm64DarwinNetworkConnectionEventError::ContractLayout)
}

fn load_owner_field(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    owner: Arm64Register,
    field: DarwinNetworkOwnerField,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    let offset = u32::try_from(DarwinNetworkOwnerAbiSchema::ARM64_DARWIN.offset(field))
        .map_err(|_| Arm64DarwinNetworkConnectionEventError::ContractLayout)?;
    load_at(code, destination, owner, offset);
    Ok(())
}

fn load_event_field(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    event: Arm64Register,
    offset: u64,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    let offset = u32::try_from(offset)
        .map_err(|_| Arm64DarwinNetworkConnectionEventError::ContractLayout)?;
    load_at(code, destination, event, offset);
    Ok(())
}

fn load_at(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    base: Arm64Register,
    offset: u32,
) {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(base),
        offset,
    });
}

fn store_zero_at(
    code: &mut Arm64CodeBuilder,
    base: Arm64Register,
    offset: u64,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    let offset = u32::try_from(offset)
        .map_err(|_| Arm64DarwinNetworkConnectionEventError::ContractLayout)?;
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::Zero,
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
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    let offset = u32::try_from(offset)
        .map_err(|_| Arm64DarwinNetworkConnectionEventError::ContractLayout)?;
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::General(base),
        offset,
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

fn add_small_immediate(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    source: Arm64Register,
    immediate: u16,
) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::General(source),
        immediate,
        shift_12: false,
    });
}

fn add_register(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    left: Arm64Register,
    right: Arm64Register,
) {
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64DataRegister::General(destination),
        left: Arm64DataRegister::General(left),
        right: Arm64DataRegister::General(right),
    });
}

fn compare_register(code: &mut Arm64CodeBuilder, left: Arm64Register, right: Arm64Register) {
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(left),
        right: Arm64DataRegister::General(right),
    });
}

fn load_byte(code: &mut Arm64CodeBuilder, destination: Arm64Register, base: Arm64Register) {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Byte,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(base),
        offset: 0,
    });
}

fn store_byte(code: &mut Arm64CodeBuilder, base: Arm64Register, source: Arm64Register) {
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Byte,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::General(base),
        offset: 0,
    });
}

fn immediate(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    value: u64,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    let immediate =
        u16::try_from(value).map_err(|_| Arm64DarwinNetworkConnectionEventError::ContractLayout)?;
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
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    let immediate = u16::try_from(expected)
        .map_err(|_| Arm64DarwinNetworkConnectionEventError::ContractLayout)?;
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

fn call_import(code: &mut Arm64CodeBuilder, target: crate::Arm64FunctionImportId) {
    code.load_function_import(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
}

fn return_from_function(code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
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

const fn x(number: u8) -> Arm64Register {
    match Arm64Register::new(number) {
        Some(register) => register,
        None => panic!("closed ARM64 register is valid"),
    }
}

#[derive(Debug)]
pub enum Arm64DarwinNetworkConnectionEventError {
    ContractLayout,
    Owner(Arm64DarwinNetworkOwnerError),
    Channel(Arm64DarwinNetworkChannelError),
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinNetworkConnectionEventError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin connection event failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinNetworkConnectionEventError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Owner(error) => Some(error),
            Self::Channel(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ContractLayout => None,
        }
    }
}

macro_rules! convert_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for Arm64DarwinNetworkConnectionEventError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}

convert_error!(Arm64DarwinNetworkOwnerError, Owner);
convert_error!(Arm64DarwinNetworkChannelError, Channel);
convert_error!(Arm64CodeError, Code);
convert_error!(Arm64ProgramError, Program);

#[cfg(test)]
mod tests {
    use super::add_darwin_network_connection_event_targets;
    use crate::{Arm64DarwinNetworkAdapterImports, Arm64ProgramBuilder};

    #[test]
    fn connection_event_targets_are_distinct() {
        let mut program = Arm64ProgramBuilder::new();
        let imports = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
        let targets = add_darwin_network_connection_event_targets(&mut program, &imports).unwrap();
        assert_ne!(targets.descriptor(), targets.receive());
    }
}
