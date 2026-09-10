use std::fmt;

use nocter_runtime_contract::{
    DarwinNetworkAdapterOperation, DarwinNetworkOwnerAbiSchema, DarwinNetworkOwnerField,
    DarwinNetworkOwnerKind,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64CodeBuilder,
    Arm64CodeError, Arm64DarwinNetworkAdapterImports, Arm64DarwinNetworkOwnerError,
    Arm64DataRegister, Arm64DataSize, Arm64FunctionId, Arm64Instruction, Arm64LoadStoreSize,
    Arm64ProgramBuilder, Arm64ProgramError, Arm64Register, emit_darwin_network_owner_guard,
};

/// Adds the sole reactor-descriptor projection for one provider-owner family.
///
/// # Errors
///
/// Propagates malformed owner contracts and ARM64 program/code construction failures.
pub(crate) fn add_darwin_network_owner_event_descriptor_target(
    program: &mut Arm64ProgramBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    kind: DarwinNetworkOwnerKind,
) -> Result<Arm64FunctionId, Arm64DarwinNetworkOwnerEventError> {
    let target = program.declare_function();
    let mut code = Arm64CodeBuilder::new();
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64AddSubtractDestination::StackPointer,
        source: Arm64BaseRegister::StackPointer,
        immediate: 16,
        shift_12: false,
    });
    store_stack(&mut code, x(19), 0);
    store_stack(&mut code, x(30), 8);
    move_register(&mut code, x(19), x(0));
    emit_darwin_network_owner_guard(
        &mut code,
        x(19),
        kind,
        DarwinNetworkAdapterOperation::EventDescriptor,
        imports,
    )?;
    let offset = u32::try_from(
        DarwinNetworkOwnerAbiSchema::ARM64_DARWIN.offset(DarwinNetworkOwnerField::EventReader),
    )
    .map_err(|_| Arm64DarwinNetworkOwnerEventError::ContractLayout)?;
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(x(0)),
        base: Arm64BaseRegister::General(x(19)),
        offset,
    });
    load_stack(&mut code, x(19), 0);
    load_stack(&mut code, x(30), 8);
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::StackPointer,
        source: Arm64BaseRegister::StackPointer,
        immediate: 16,
        shift_12: false,
    });
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
    program.define_function(target, code.finish()?)?;
    Ok(target)
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
pub enum Arm64DarwinNetworkOwnerEventError {
    ContractLayout,
    Owner(Arm64DarwinNetworkOwnerError),
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinNetworkOwnerEventError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ARM64 Darwin network owner event failed: {self:?}"
        )
    }
}

impl std::error::Error for Arm64DarwinNetworkOwnerEventError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Owner(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ContractLayout => None,
        }
    }
}

impl From<Arm64DarwinNetworkOwnerError> for Arm64DarwinNetworkOwnerEventError {
    fn from(error: Arm64DarwinNetworkOwnerError) -> Self {
        Self::Owner(error)
    }
}

impl From<Arm64CodeError> for Arm64DarwinNetworkOwnerEventError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinNetworkOwnerEventError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}
