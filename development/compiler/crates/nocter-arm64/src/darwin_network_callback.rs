use std::fmt;

use nocter_runtime_contract::{
    DarwinNetworkAdapterFunction, DarwinNetworkCallbackEventAbiSchema, DarwinNetworkCallbackRole,
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

    immediate(&mut code, x(8), event.code());
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

fn immediate(code: &mut Arm64CodeBuilder, destination: Arm64Register, value: u64) {
    debug_assert!(value <= u64::from(u16::MAX));
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: crate::Arm64MoveWide::Zero,
        destination,
        immediate: value as u16,
        shift: 0,
    });
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

    use super::{Arm64DarwinNetworkCallbackError, add_darwin_network_state_callback};
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
}
