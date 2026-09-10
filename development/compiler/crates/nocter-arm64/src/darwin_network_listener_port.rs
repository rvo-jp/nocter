use std::fmt;

use nocter_runtime_contract::{
    DarwinNetworkAdapterFunction, DarwinNetworkAdapterOperation, DarwinNetworkOwnerAbiSchema,
    DarwinNetworkOwnerField, DarwinNetworkOwnerKind,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64CodeBuilder,
    Arm64CodeError, Arm64DarwinNetworkAdapterImports, Arm64DarwinNetworkOwnerError,
    Arm64DataRegister, Arm64DataSize, Arm64FunctionId, Arm64Instruction, Arm64LoadStoreSize,
    Arm64ProgramBuilder, Arm64ProgramError, Arm64Register, emit_darwin_network_owner_guard,
};

/// Adds the listener-only effective-port target.
///
/// The target accepts a listener owner in `x0` and returns Network.framework's effective local
/// port in `x0`. The runtime owner model rejects connection owners and listeners that have not
/// started.
///
/// # Errors
///
/// Propagates malformed owner layout and ARM64 program/code construction failures.
pub(crate) fn add_darwin_network_listener_port_target(
    program: &mut Arm64ProgramBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<Arm64FunctionId, Arm64DarwinNetworkListenerPortError> {
    let target = program.declare_function();
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract);
    store_stack(&mut code, x(19), 0);
    store_stack(&mut code, x(30), 8);
    move_register(&mut code, x(19), x(0));
    emit_darwin_network_owner_guard(
        &mut code,
        x(19),
        DarwinNetworkOwnerKind::Listener,
        DarwinNetworkAdapterOperation::ListenerPort,
        imports,
    )?;
    load_owner_field(
        &mut code,
        x(0),
        x(19),
        DarwinNetworkOwnerField::NativeObject,
    )?;
    code.load_function_import(
        imports.function(DarwinNetworkAdapterFunction::ListenerGetPort),
        x(16),
    );
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
    load_stack(&mut code, x(19), 0);
    load_stack(&mut code, x(30), 8);
    adjust_stack(&mut code, Arm64AddSubtract::Add);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
    program.define_function(target, code.finish()?)?;
    Ok(target)
}

fn adjust_stack(code: &mut Arm64CodeBuilder, operation: Arm64AddSubtract) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation,
        set_flags: false,
        destination: Arm64AddSubtractDestination::StackPointer,
        source: Arm64BaseRegister::StackPointer,
        immediate: 16,
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

fn load_owner_field(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    owner: Arm64Register,
    field: DarwinNetworkOwnerField,
) -> Result<(), Arm64DarwinNetworkListenerPortError> {
    let offset = u32::try_from(DarwinNetworkOwnerAbiSchema::ARM64_DARWIN.offset(field))
        .map_err(|_| Arm64DarwinNetworkListenerPortError::ContractLayout)?;
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(owner),
        offset,
    });
    Ok(())
}

const fn x(number: u8) -> Arm64Register {
    match Arm64Register::new(number) {
        Some(register) => register,
        None => panic!("closed ARM64 register is valid"),
    }
}

#[derive(Debug)]
pub enum Arm64DarwinNetworkListenerPortError {
    ContractLayout,
    Owner(Arm64DarwinNetworkOwnerError),
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinNetworkListenerPortError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin listener port failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinNetworkListenerPortError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Owner(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ContractLayout => None,
        }
    }
}

impl From<Arm64DarwinNetworkOwnerError> for Arm64DarwinNetworkListenerPortError {
    fn from(error: Arm64DarwinNetworkOwnerError) -> Self {
        Self::Owner(error)
    }
}

impl From<Arm64CodeError> for Arm64DarwinNetworkListenerPortError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinNetworkListenerPortError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}
