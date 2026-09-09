use std::fmt;

use nocter_runtime_contract::{
    DarwinNetworkAdapterFunction, DarwinNetworkAdapterOperation,
    DarwinNetworkCallbackEventAbiSchema, DarwinNetworkConnectionState,
    DarwinNetworkConnectionStateObservationAbiSchema, DarwinNetworkEventKind,
    DarwinNetworkOwnerAbiSchema, DarwinNetworkOwnerField, DarwinNetworkOwnerKind,
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
    receive_state: Arm64FunctionId,
}

impl Arm64DarwinNetworkConnectionEventTargets {
    #[must_use]
    pub const fn descriptor(self) -> Arm64FunctionId {
        self.descriptor
    }

    #[must_use]
    pub const fn receive_state(self) -> Arm64FunctionId {
        self.receive_state
    }
}

/// Adds the only production targets that expose the callback channel of a connection owner.
///
/// `descriptor` accepts the owner address in `x0` and returns the reactor-readable descriptor in
/// `x0`. `receive_state` accepts the owner address in `x0`, receives and consumes exactly one
/// connection-state event, and writes normalized `(state, error domain, error code)` words to the
/// caller storage addressed by the Nocter ABI result register `x8`. Zero error-domain and
/// error-code words mean that the provider supplied no error.
///
/// The receive target owns event-kind and state validation, retained-error release, record
/// clearing, and the final-state owner transition. No caller can observe or forget an ownership-
/// bearing native event record.
///
/// # Errors
///
/// Propagates malformed runtime contracts and ARM64 program/code construction failures.
pub(crate) fn add_darwin_network_connection_event_targets(
    program: &mut Arm64ProgramBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<Arm64DarwinNetworkConnectionEventTargets, Arm64DarwinNetworkConnectionEventError> {
    let descriptor = program.declare_function();
    let receive_state = program.declare_function();
    program.define_function(descriptor, descriptor_code(imports)?)?;
    program.define_function(receive_state, receive_state_code(imports)?)?;
    Ok(Arm64DarwinNetworkConnectionEventTargets {
        descriptor,
        receive_state,
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

fn receive_state_code(
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<crate::Arm64Code, Arm64DarwinNetworkConnectionEventError> {
    const FRAME_SIZE: u16 = 112;
    const SAVED: [(Arm64Register, u32); 8] = [
        (x(19), 40),
        (x(20), 48),
        (x(21), 56),
        (x(22), 64),
        (x(23), 72),
        (x(24), 80),
        (x(25), 88),
        (x(30), 96),
    ];
    let schema = DarwinNetworkCallbackEventAbiSchema::ARM64_DARWIN;
    let observation = DarwinNetworkConnectionStateObservationAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, FRAME_SIZE);
    for (register, offset) in SAVED {
        store_stack(&mut code, register, offset);
    }
    move_register(&mut code, x(19), x(0));
    move_register(&mut code, x(25), x(8));
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

    load_event_field(&mut code, x(8), x(20), schema.kind_offset())?;
    compare_immediate(
        &mut code,
        x(8),
        DarwinNetworkEventKind::ConnectionState.code(),
    )?;
    let valid_kind = code.create_label();
    code.branch_conditional(valid_kind, Arm64BranchCondition::Equal);
    call_import(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::Abort),
    );
    code.bind(valid_kind)?;

    load_event_field(&mut code, x(21), x(20), event_payload_offset(schema, 0)?)?;
    validate_connection_state(&mut code, x(21), imports)?;
    load_event_field(&mut code, x(22), x(20), event_payload_offset(schema, 1)?)?;
    immediate(&mut code, x(23), 0)?;
    immediate(&mut code, x(24), 0)?;
    let error_consumed = code.create_label();
    compare_immediate(&mut code, x(22), 0)?;
    code.branch_conditional(error_consumed, Arm64BranchCondition::Equal);
    move_register(&mut code, x(0), x(22));
    call_import(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::NetworkErrorGetDomain),
    );
    move_register(&mut code, x(23), x(0));
    move_register(&mut code, x(0), x(22));
    call_import(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::NetworkErrorGetCode),
    );
    move_register(&mut code, x(24), x(0));
    move_register(&mut code, x(0), x(22));
    call_import(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::NetworkRelease),
    );
    code.bind(error_consumed)?;
    clear_event(&mut code, x(20), schema)?;

    let not_final = code.create_label();
    compare_immediate(
        &mut code,
        x(21),
        DarwinNetworkConnectionState::Cancelled.code(),
    )?;
    code.branch_conditional(not_final, Arm64BranchCondition::NotEqual);
    emit_darwin_network_owner_transition(
        &mut code,
        x(19),
        DarwinNetworkOwnerKind::Connection,
        DarwinNetworkAdapterOperation::ObserveFinalState,
        imports,
    )?;
    code.bind(not_final)?;

    store_at(&mut code, x(25), observation.state_offset(), x(21))?;
    store_at(&mut code, x(25), observation.error_domain_offset(), x(23))?;
    store_at(&mut code, x(25), observation.error_code_offset(), x(24))?;
    for (register, offset) in SAVED {
        load_stack(&mut code, register, offset);
    }
    adjust_stack(&mut code, Arm64AddSubtract::Add, FRAME_SIZE);
    return_from_function(&mut code);
    code.finish().map_err(Into::into)
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
        assert_ne!(targets.descriptor(), targets.receive_state());
    }
}
