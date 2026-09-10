use std::fmt;

use nocter_runtime_contract::{
    DarwinBlockAbiSchema, DarwinTlsAdapterFunction, DarwinTlsConfigurationAbiSchema,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DarwinTlsAdapterImports, Arm64DataRegister,
    Arm64DataSize, Arm64FunctionId, Arm64Instruction, Arm64LoadStoreSize, Arm64MoveWide,
    Arm64ProgramBuilder, Arm64ProgramError, Arm64Register,
};

/// Adds the fixed synchronous TLS-parameter configuration callback.
///
/// The callback borrows one closed configuration record through its sole pointer capture. It
/// obtains a retained Security protocol-options object, installs the server name, optional ALPN,
/// and TLS 1.2 minimum, releases the temporary object, then records successful configuration.
/// It cannot retain source storage or escape the parameter-construction call.
///
/// # Errors
///
/// Propagates contract-layout and ARM64 program/code construction failures.
pub fn add_darwin_tls_configuration_callback(
    program: &mut Arm64ProgramBuilder,
    imports: &Arm64DarwinTlsAdapterImports,
) -> Result<Arm64FunctionId, Arm64DarwinTlsCallbackError> {
    let target = program.declare_function();
    let mut code = Arm64CodeBuilder::new();
    let schema = DarwinTlsConfigurationAbiSchema::ARM64_DARWIN;
    let block = DarwinBlockAbiSchema::ARM64_DARWIN;
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, 64);
    for (register, offset) in [(x(19), 32), (x(20), 40), (x(21), 48), (x(30), 56)] {
        store_stack(&mut code, register, offset);
    }
    load_from(&mut code, x(19), x(0), offset(block.captures_offset())?);
    move_register(&mut code, x(0), x(1));
    call(
        &mut code,
        imports.function(DarwinTlsAdapterFunction::CopySecurityProtocolOptions),
    );
    move_register(&mut code, x(20), x(0));

    let release = code.create_label();
    let configured = code.create_label();
    let complete = code.create_label();
    compare_zero(&mut code, x(20));
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    load_from(
        &mut code,
        x(21),
        x(19),
        offset(schema.server_name_offset())?,
    );
    compare_zero(&mut code, x(21));
    code.branch_conditional(release, Arm64BranchCondition::Equal);

    move_register(&mut code, x(0), x(20));
    move_register(&mut code, x(1), x(21));
    call(
        &mut code,
        imports.function(DarwinTlsAdapterFunction::SetServerName),
    );
    move_register(&mut code, x(0), x(20));
    immediate(&mut code, x(1), schema.minimum_protocol_version());
    call(
        &mut code,
        imports.function(DarwinTlsAdapterFunction::SetMinimumProtocolVersion),
    );

    load_from(
        &mut code,
        x(21),
        x(19),
        offset(schema.application_protocol_offset())?,
    );
    compare_zero(&mut code, x(21));
    code.branch_conditional(configured, Arm64BranchCondition::Equal);
    move_register(&mut code, x(0), x(20));
    move_register(&mut code, x(1), x(21));
    call(
        &mut code,
        imports.function(DarwinTlsAdapterFunction::AddApplicationProtocol),
    );

    code.bind(configured)?;
    move_register(&mut code, x(0), x(20));
    call(
        &mut code,
        imports.function(DarwinTlsAdapterFunction::SecurityRelease),
    );
    immediate(&mut code, x(21), 1);
    store_to(&mut code, x(21), x(19), offset(schema.configured_offset())?);
    code.branch(complete, false);

    code.bind(release)?;
    move_register(&mut code, x(0), x(20));
    call(
        &mut code,
        imports.function(DarwinTlsAdapterFunction::SecurityRelease),
    );
    code.bind(complete)?;
    for (register, offset) in [(x(19), 32), (x(20), 40), (x(21), 48), (x(30), 56)] {
        load_stack(&mut code, register, offset);
    }
    adjust_stack(&mut code, Arm64AddSubtract::Add, 64);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
    program.define_function(target, code.finish()?)?;
    Ok(target)
}

fn offset(value: u64) -> Result<u32, Arm64DarwinTlsCallbackError> {
    u32::try_from(value).map_err(|_| Arm64DarwinTlsCallbackError::ContractLayout)
}

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).expect("closed ARM64 register is valid")
}

fn immediate(code: &mut Arm64CodeBuilder, destination: Arm64Register, value: u16) {
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination,
        immediate: value,
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

fn load_from(
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

fn store_to(code: &mut Arm64CodeBuilder, source: Arm64Register, base: Arm64Register, offset: u32) {
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::General(base),
        offset,
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

fn call(code: &mut Arm64CodeBuilder, target: crate::Arm64FunctionImportId) {
    code.load_function_import(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
}

#[derive(Debug)]
pub enum Arm64DarwinTlsCallbackError {
    ContractLayout,
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinTlsCallbackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin TLS callback failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinTlsCallbackError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ContractLayout => None,
        }
    }
}

impl From<Arm64CodeError> for Arm64DarwinTlsCallbackError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinTlsCallbackError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}

#[cfg(test)]
mod tests {
    use super::add_darwin_tls_configuration_callback;
    use crate::{Arm64DarwinTlsAdapterImports, Arm64ProgramBuilder};

    #[test]
    fn configuration_callback_uses_the_capability_scoped_imports() {
        let mut program = Arm64ProgramBuilder::new();
        let imports = Arm64DarwinTlsAdapterImports::declare(&mut program).unwrap();
        add_darwin_tls_configuration_callback(&mut program, &imports).unwrap();
    }
}
