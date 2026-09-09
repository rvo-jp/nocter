use std::fmt;

use nocter_runtime_contract::{
    DarwinNetworkAdapterFunction, DarwinNetworkCallbackEventAbiSchema, DarwinNetworkCallbackRole,
    DarwinNetworkEventKind,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DarwinNetworkAdapterImports,
    Arm64DarwinNetworkChannelError, Arm64DataRegister, Arm64DataSize, Arm64FunctionId,
    Arm64Instruction, Arm64LoadStoreSize, Arm64ProgramBuilder, Arm64ProgramError, Arm64Register,
    emit_darwin_network_event_send,
};

/// Declares and defines a fixed state-event callback for a connection or listener owner.
///
/// The generated callback retains an optional provider error, transfers one complete event through
/// the captured channel writer, and returns without publishing any other native value. The channel
/// emitter owns its fatal-transfer policy.
///
/// # Errors
///
/// Rejects a non-state callback role or propagates code, channel, and program construction errors.
pub fn add_darwin_network_state_callback(
    program: &mut Arm64ProgramBuilder,
    role: DarwinNetworkCallbackRole,
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<Arm64FunctionId, Arm64DarwinNetworkCallbackError> {
    let event = match role {
        DarwinNetworkCallbackRole::ConnectionState | DarwinNetworkCallbackRole::ListenerState => {
            role.event_kind()
                .ok_or(Arm64DarwinNetworkCallbackError::UnsupportedRole(role))?
        }
        DarwinNetworkCallbackRole::ConfigureProtocol
        | DarwinNetworkCallbackRole::ConnectionReceive
        | DarwinNetworkCallbackRole::ConnectionSend
        | DarwinNetworkCallbackRole::ListenerAccept => {
            return Err(Arm64DarwinNetworkCallbackError::UnsupportedRole(role));
        }
    };
    let target = program.declare_function();
    let schema = DarwinNetworkCallbackEventAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, 96);
    for (register, offset) in [(x(19), 64), (x(20), 72), (x(21), 80), (x(30), 88)] {
        store(&mut code, register, offset);
    }
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(x(19)),
        base: Arm64BaseRegister::General(x(0)),
        offset: 32,
    });
    code.append(Arm64Instruction::BitfieldExtend {
        size: Arm64DataSize::Bits64,
        signed: false,
        source_bits: 32,
        destination: x(20),
        source: x(1),
    });
    move_register(&mut code, x(21), x(2));

    let retained = code.create_label();
    compare_zero(&mut code, x(21));
    code.branch_conditional(retained, Arm64BranchCondition::Equal);
    move_register(&mut code, x(0), x(21));
    call(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::NetworkRetain),
    );
    code.bind(retained)?;

    immediate(&mut code, x(8), event.code())?;
    store(
        &mut code,
        x(8),
        u32::try_from(schema.kind_offset())
            .map_err(|_| Arm64DarwinNetworkCallbackError::ContractLayout)?,
    );
    store(&mut code, x(20), payload_offset(schema, 0)?);
    store(&mut code, x(21), payload_offset(schema, 1)?);
    store_zero(&mut code, payload_offset(schema, 2)?);
    store_zero(&mut code, payload_offset(schema, 3)?);
    emit_darwin_network_event_send(&mut code, imports.channel(), x(19), 0)?;

    for (register, offset) in [(x(19), 64), (x(20), 72), (x(21), 80), (x(30), 88)] {
        load(&mut code, register, offset);
    }
    adjust_stack(&mut code, Arm64AddSubtract::Add, 96);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
    program.define_function(target, code.finish()?)?;
    Ok(target)
}

/// Declares and defines a fixed receive, send, or accepted-connection callback.
///
/// Every object placed in the event record is retained with its own runtime family before channel
/// transfer. The receiver consequently owns each non-null payload after one complete event read.
///
/// # Errors
///
/// Rejects a configuration or state role and propagates code, channel, and program construction
/// errors.
pub fn add_darwin_network_completion_callback(
    program: &mut Arm64ProgramBuilder,
    role: DarwinNetworkCallbackRole,
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<Arm64FunctionId, Arm64DarwinNetworkCallbackError> {
    let target = program.declare_function();
    let code = match role {
        DarwinNetworkCallbackRole::ConnectionReceive => receive_callback(imports)?,
        DarwinNetworkCallbackRole::ConnectionSend => send_callback(imports)?,
        DarwinNetworkCallbackRole::ListenerAccept => accepted_connection_callback(imports)?,
        DarwinNetworkCallbackRole::ConfigureProtocol
        | DarwinNetworkCallbackRole::ConnectionState
        | DarwinNetworkCallbackRole::ListenerState => {
            return Err(Arm64DarwinNetworkCallbackError::UnsupportedRole(role));
        }
    };
    program.define_function(target, code.finish()?)?;
    Ok(target)
}

fn receive_callback(
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<Arm64CodeBuilder, Arm64DarwinNetworkCallbackError> {
    let schema = DarwinNetworkCallbackEventAbiSchema::ARM64_DARWIN;
    let saved = [
        (x(19), 48),
        (x(20), 56),
        (x(21), 64),
        (x(22), 72),
        (x(23), 80),
        (x(30), 88),
    ];
    let mut code = Arm64CodeBuilder::new();
    begin_callback(&mut code, &saved);
    move_register(&mut code, x(20), x(1));
    move_register(&mut code, x(21), x(2));
    code.append(Arm64Instruction::BitfieldExtend {
        size: Arm64DataSize::Bits64,
        signed: false,
        source_bits: 8,
        destination: x(22),
        source: x(3),
    });
    move_register(&mut code, x(23), x(4));
    retain_optional(
        &mut code,
        x(20),
        imports.function(DarwinNetworkAdapterFunction::DispatchRetain),
    )?;
    retain_optional(
        &mut code,
        x(21),
        imports.function(DarwinNetworkAdapterFunction::NetworkRetain),
    )?;
    retain_optional(
        &mut code,
        x(23),
        imports.function(DarwinNetworkAdapterFunction::NetworkRetain),
    )?;
    write_event(
        &mut code,
        schema,
        DarwinNetworkEventKind::ReceiveCompletion,
        [Some(x(20)), Some(x(23)), Some(x(22)), Some(x(21))],
        imports,
    )?;
    finish_callback(&mut code, &saved);
    Ok(code)
}

fn send_callback(
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<Arm64CodeBuilder, Arm64DarwinNetworkCallbackError> {
    let schema = DarwinNetworkCallbackEventAbiSchema::ARM64_DARWIN;
    let saved = [(x(19), 64), (x(20), 72), (x(30), 88)];
    let mut code = Arm64CodeBuilder::new();
    begin_callback(&mut code, &saved);
    move_register(&mut code, x(20), x(1));
    retain_optional(
        &mut code,
        x(20),
        imports.function(DarwinNetworkAdapterFunction::NetworkRetain),
    )?;
    write_event(
        &mut code,
        schema,
        DarwinNetworkEventKind::SendCompletion,
        [Some(x(20)), None, None, None],
        imports,
    )?;
    finish_callback(&mut code, &saved);
    Ok(code)
}

fn accepted_connection_callback(
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<Arm64CodeBuilder, Arm64DarwinNetworkCallbackError> {
    let schema = DarwinNetworkCallbackEventAbiSchema::ARM64_DARWIN;
    let saved = [(x(19), 64), (x(20), 72), (x(30), 88)];
    let mut code = Arm64CodeBuilder::new();
    begin_callback(&mut code, &saved);
    move_register(&mut code, x(20), x(1));
    retain_required(
        &mut code,
        x(20),
        imports.function(DarwinNetworkAdapterFunction::NetworkRetain),
    );
    write_event(
        &mut code,
        schema,
        DarwinNetworkEventKind::AcceptedConnection,
        [Some(x(20)), None, None, None],
        imports,
    )?;
    finish_callback(&mut code, &saved);
    Ok(code)
}

fn begin_callback(code: &mut Arm64CodeBuilder, saved: &[(Arm64Register, u32)]) {
    adjust_stack(code, Arm64AddSubtract::Subtract, 96);
    for (register, offset) in saved {
        store(code, *register, *offset);
    }
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(x(19)),
        base: Arm64BaseRegister::General(x(0)),
        offset: 32,
    });
}

fn finish_callback(code: &mut Arm64CodeBuilder, saved: &[(Arm64Register, u32)]) {
    for (register, offset) in saved {
        load(code, *register, *offset);
    }
    adjust_stack(code, Arm64AddSubtract::Add, 96);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
}

fn retain_optional(
    code: &mut Arm64CodeBuilder,
    object: Arm64Register,
    retain: crate::Arm64FunctionImportId,
) -> Result<(), Arm64DarwinNetworkCallbackError> {
    let complete = code.create_label();
    compare_zero(code, object);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    retain_required(code, object, retain);
    code.bind(complete)?;
    Ok(())
}

fn retain_required(
    code: &mut Arm64CodeBuilder,
    object: Arm64Register,
    retain: crate::Arm64FunctionImportId,
) {
    move_register(code, x(0), object);
    call(code, retain);
}

fn write_event(
    code: &mut Arm64CodeBuilder,
    schema: DarwinNetworkCallbackEventAbiSchema,
    event: DarwinNetworkEventKind,
    payloads: [Option<Arm64Register>; 4],
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<(), Arm64DarwinNetworkCallbackError> {
    immediate(code, x(8), event.code())?;
    store(
        code,
        x(8),
        u32::try_from(schema.kind_offset())
            .map_err(|_| Arm64DarwinNetworkCallbackError::ContractLayout)?,
    );
    for (lane, payload) in payloads.iter().copied().enumerate() {
        if let Some(payload) = payload {
            store(code, payload, payload_offset(schema, lane)?);
        } else {
            store_zero(code, payload_offset(schema, lane)?);
        }
    }
    emit_darwin_network_event_send(code, imports.channel(), x(19), 0)?;
    Ok(())
}

fn payload_offset(
    schema: DarwinNetworkCallbackEventAbiSchema,
    lane: usize,
) -> Result<u32, Arm64DarwinNetworkCallbackError> {
    schema
        .payload_offset(lane)
        .and_then(|offset| u32::try_from(offset).ok())
        .ok_or(Arm64DarwinNetworkCallbackError::ContractLayout)
}

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).expect("closed ARM64 register is valid")
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

fn immediate(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    value: u64,
) -> Result<(), Arm64DarwinNetworkCallbackError> {
    let immediate =
        u16::try_from(value).map_err(|_| Arm64DarwinNetworkCallbackError::ContractLayout)?;
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: crate::Arm64MoveWide::Zero,
        destination,
        immediate,
        shift: 0,
    });
    Ok(())
}

fn compare_zero(code: &mut Arm64CodeBuilder, value: Arm64Register) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: 0,
        shift_12: false,
    });
}

fn store(code: &mut Arm64CodeBuilder, source: Arm64Register, offset: u32) {
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::StackPointer,
        offset,
    });
}

fn store_zero(code: &mut Arm64CodeBuilder, offset: u32) {
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::Zero,
        base: Arm64BaseRegister::StackPointer,
        offset,
    });
}

fn load(code: &mut Arm64CodeBuilder, destination: Arm64Register, offset: u32) {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::StackPointer,
        offset,
    });
}

fn call(code: &mut Arm64CodeBuilder, target: crate::Arm64FunctionImportId) {
    code.load_function_import(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64DarwinNetworkCallbackError {
    UnsupportedRole(DarwinNetworkCallbackRole),
    ContractLayout,
    Channel(Arm64DarwinNetworkChannelError),
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinNetworkCallbackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin network callback failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinNetworkCallbackError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Channel(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::UnsupportedRole(_) | Self::ContractLayout => None,
        }
    }
}

impl From<Arm64DarwinNetworkChannelError> for Arm64DarwinNetworkCallbackError {
    fn from(error: Arm64DarwinNetworkChannelError) -> Self {
        Self::Channel(error)
    }
}

impl From<Arm64CodeError> for Arm64DarwinNetworkCallbackError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinNetworkCallbackError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}

#[cfg(test)]
mod tests {
    use nocter_runtime_contract::DarwinNetworkCallbackRole;

    use super::{
        Arm64DarwinNetworkCallbackError, add_darwin_network_completion_callback,
        add_darwin_network_state_callback,
    };
    use crate::{Arm64DarwinNetworkAdapterImports, Arm64ProgramBuilder};

    #[test]
    fn state_callback_rejects_a_completion_signature() {
        let mut program = Arm64ProgramBuilder::new();
        let imports = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
        assert_eq!(
            add_darwin_network_state_callback(
                &mut program,
                DarwinNetworkCallbackRole::ConnectionReceive,
                &imports,
            ),
            Err(Arm64DarwinNetworkCallbackError::UnsupportedRole(
                DarwinNetworkCallbackRole::ConnectionReceive
            ))
        );
    }

    #[test]
    fn completion_callbacks_cover_each_ownership_shape() {
        let mut program = Arm64ProgramBuilder::new();
        let imports = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
        let callbacks = [
            DarwinNetworkCallbackRole::ConnectionReceive,
            DarwinNetworkCallbackRole::ConnectionSend,
            DarwinNetworkCallbackRole::ListenerAccept,
        ]
        .map(|role| add_darwin_network_completion_callback(&mut program, role, &imports).unwrap());
        assert_ne!(callbacks[0], callbacks[1]);
        assert_ne!(callbacks[1], callbacks[2]);
        assert_eq!(
            add_darwin_network_completion_callback(
                &mut program,
                DarwinNetworkCallbackRole::ListenerState,
                &imports,
            ),
            Err(Arm64DarwinNetworkCallbackError::UnsupportedRole(
                DarwinNetworkCallbackRole::ListenerState
            ))
        );
    }
}
