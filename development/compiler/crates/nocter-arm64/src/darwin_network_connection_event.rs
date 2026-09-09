use std::fmt;

use nocter_runtime_contract::{
    DarwinNetworkAdapterOperation, DarwinNetworkOwnerAbiSchema, DarwinNetworkOwnerField,
    DarwinNetworkOwnerKind,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64CodeBuilder,
    Arm64CodeError, Arm64DarwinNetworkAdapterImports, Arm64DarwinNetworkChannelError,
    Arm64DarwinNetworkOwnerError, Arm64DataRegister, Arm64DataSize, Arm64FunctionId,
    Arm64Instruction, Arm64LoadStoreSize, Arm64ProgramBuilder, Arm64ProgramError, Arm64Register,
    emit_darwin_network_event_receive_to_pointer, emit_darwin_network_owner_guard,
};

/// Native callable targets for observing and receiving connection events.
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
/// `x0`. `receive` accepts owner and event-record addresses in `x0` and `x1`; the complete-record
/// channel contract owns retry and fail-stop behavior.
///
/// # Errors
///
/// Propagates malformed runtime contracts and ARM64 program/code construction failures.
pub fn add_darwin_network_connection_event_targets(
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
    owner_prologue(&mut code);
    emit_darwin_network_owner_guard(
        &mut code,
        x(19),
        DarwinNetworkOwnerKind::Connection,
        DarwinNetworkAdapterOperation::EventDescriptor,
        imports,
    )?;
    load_owner_field(&mut code, x(0), x(19), DarwinNetworkOwnerField::EventReader)?;
    owner_epilogue(&mut code);
    code.finish().map_err(Into::into)
}

fn receive_code(
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<crate::Arm64Code, Arm64DarwinNetworkConnectionEventError> {
    let mut code = Arm64CodeBuilder::new();
    owner_prologue(&mut code);
    move_register(&mut code, x(20), x(1));
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
    emit_darwin_network_event_receive_to_pointer(&mut code, imports.channel(), x(21), x(20))?;
    owner_epilogue(&mut code);
    code.finish().map_err(Into::into)
}

fn owner_prologue(code: &mut Arm64CodeBuilder) {
    adjust_stack(code, Arm64AddSubtract::Subtract);
    for (register, offset) in saved_registers() {
        store_stack(code, register, offset);
    }
    move_register(code, x(19), x(0));
}

fn owner_epilogue(code: &mut Arm64CodeBuilder) {
    for (register, offset) in saved_registers() {
        load_stack(code, register, offset);
    }
    adjust_stack(code, Arm64AddSubtract::Add);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
}

fn saved_registers() -> [(Arm64Register, u32); 4] {
    [(x(19), 0), (x(20), 8), (x(21), 16), (x(30), 24)]
}

fn load_owner_field(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    owner: Arm64Register,
    field: DarwinNetworkOwnerField,
) -> Result<(), Arm64DarwinNetworkConnectionEventError> {
    let offset = u32::try_from(DarwinNetworkOwnerAbiSchema::ARM64_DARWIN.offset(field))
        .map_err(|_| Arm64DarwinNetworkConnectionEventError::ContractLayout)?;
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(owner),
        offset,
    });
    Ok(())
}

fn adjust_stack(code: &mut Arm64CodeBuilder, operation: Arm64AddSubtract) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation,
        set_flags: false,
        destination: Arm64AddSubtractDestination::StackPointer,
        source: Arm64BaseRegister::StackPointer,
        immediate: 32,
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

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).expect("closed ARM64 register is valid")
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
