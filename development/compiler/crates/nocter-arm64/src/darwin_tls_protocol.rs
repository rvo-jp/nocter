use std::fmt;

use nocter_runtime_contract::{
    DarwinNetworkAdapterFunction, DarwinNetworkOwnerAbiSchema, DarwinNetworkOwnerField,
    DarwinTlsAdapterFunction,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DarwinNetworkAdapterImports,
    Arm64DarwinTlsAdapterImports, Arm64DataRegister, Arm64DataSize, Arm64FunctionId,
    Arm64Instruction, Arm64LoadStoreSize, Arm64MoveWide, Arm64ProgramBuilder, Arm64ProgramError,
    Arm64Register,
};

/// Adds the TLS-only negotiated application-protocol predicate.
///
/// The target borrows an established network owner, obtains retained TLS metadata through
/// Network.framework, compares the provider-owned negotiated protocol while that metadata is
/// alive, and releases every temporary native object before returning one boolean. No native
/// metadata or string pointer crosses into source code.
///
/// # Errors
///
/// Propagates owner-layout and ARM64 program/code construction failures.
#[allow(
    clippy::too_many_lines,
    reason = "metadata acquisition and its reverse-order cleanup path must remain visible together"
)]
pub(crate) fn add_darwin_tls_application_protocol_match_target(
    program: &mut Arm64ProgramBuilder,
    network: &Arm64DarwinNetworkAdapterImports,
    tls: &Arm64DarwinTlsAdapterImports,
) -> Result<Arm64FunctionId, Arm64DarwinTlsProtocolError> {
    let target = program.declare_function();
    let mut code = Arm64CodeBuilder::new();
    let owner = DarwinNetworkOwnerAbiSchema::ARM64_DARWIN;
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, 64);
    for (register, offset) in [
        (x(19), 16),
        (x(20), 24),
        (x(21), 32),
        (x(22), 40),
        (x(23), 48),
        (x(30), 56),
    ] {
        store_stack(&mut code, register, offset);
    }

    load_from(
        &mut code,
        x(19),
        x(0),
        offset(owner.offset(DarwinNetworkOwnerField::NativeObject))?,
    );
    move_register(&mut code, x(20), x(1));
    immediate(&mut code, x(23), 0);
    let complete = code.create_label();
    compare_zero(&mut code, x(19));
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    compare_zero(&mut code, x(20));
    code.branch_conditional(complete, Arm64BranchCondition::Equal);

    call(
        &mut code,
        tls.function(DarwinTlsAdapterFunction::CopyTlsDefinition),
    );
    move_register(&mut code, x(21), x(0));
    compare_zero(&mut code, x(21));
    code.branch_conditional(complete, Arm64BranchCondition::Equal);

    move_register(&mut code, x(0), x(19));
    move_register(&mut code, x(1), x(21));
    call(
        &mut code,
        tls.function(DarwinTlsAdapterFunction::ConnectionCopyProtocolMetadata),
    );
    move_register(&mut code, x(22), x(0));
    move_register(&mut code, x(0), x(21));
    call(
        &mut code,
        network.function(DarwinNetworkAdapterFunction::NetworkRelease),
    );
    compare_zero(&mut code, x(22));
    code.branch_conditional(complete, Arm64BranchCondition::Equal);

    move_register(&mut code, x(0), x(22));
    call(
        &mut code,
        tls.function(DarwinTlsAdapterFunction::CopySecurityProtocolMetadata),
    );
    move_register(&mut code, x(21), x(0));
    move_register(&mut code, x(0), x(22));
    call(
        &mut code,
        network.function(DarwinNetworkAdapterFunction::NetworkRelease),
    );
    compare_zero(&mut code, x(21));
    code.branch_conditional(complete, Arm64BranchCondition::Equal);

    move_register(&mut code, x(0), x(21));
    call(
        &mut code,
        tls.function(DarwinTlsAdapterFunction::GetNegotiatedProtocol),
    );
    move_register(&mut code, x(22), x(0));
    let release_security_metadata = code.create_label();
    compare_zero(&mut code, x(22));
    code.branch_conditional(release_security_metadata, Arm64BranchCondition::Equal);
    move_register(&mut code, x(0), x(22));
    move_register(&mut code, x(1), x(20));
    call(
        &mut code,
        tls.function(DarwinTlsAdapterFunction::StringCompare),
    );
    compare_zero(&mut code, x(0));
    code.branch_conditional(release_security_metadata, Arm64BranchCondition::NotEqual);
    immediate(&mut code, x(23), 1);

    code.bind(release_security_metadata)?;
    move_register(&mut code, x(0), x(21));
    call(
        &mut code,
        tls.function(DarwinTlsAdapterFunction::SecurityRelease),
    );
    code.bind(complete)?;
    move_register(&mut code, x(0), x(23));
    for (register, offset) in [
        (x(19), 16),
        (x(20), 24),
        (x(21), 32),
        (x(22), 40),
        (x(23), 48),
        (x(30), 56),
    ] {
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

fn offset(value: u64) -> Result<u32, Arm64DarwinTlsProtocolError> {
    u32::try_from(value).map_err(|_| Arm64DarwinTlsProtocolError::ContractLayout)
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
pub enum Arm64DarwinTlsProtocolError {
    ContractLayout,
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinTlsProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ARM64 Darwin TLS protocol validation failed: {self:?}"
        )
    }
}

impl std::error::Error for Arm64DarwinTlsProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ContractLayout => None,
        }
    }
}

impl From<Arm64CodeError> for Arm64DarwinTlsProtocolError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinTlsProtocolError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}

#[cfg(test)]
mod tests {
    use nocter_runtime_contract::{DarwinNetworkAdapterFunction, DarwinTlsAdapterFunction};

    use super::add_darwin_tls_application_protocol_match_target;
    use crate::{
        Arm64DarwinNetworkAdapterImports, Arm64DarwinTlsAdapterImports, Arm64ProgramBuilder,
    };

    #[test]
    fn protocol_match_target_consumes_only_closed_tls_and_network_imports() {
        let mut program = Arm64ProgramBuilder::new();
        let network = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
        let tls = Arm64DarwinTlsAdapterImports::declare(&mut program).unwrap();
        add_darwin_tls_application_protocol_match_target(&mut program, &network, &tls).unwrap();
        assert!(
            DarwinTlsAdapterFunction::ALL
                .contains(&DarwinTlsAdapterFunction::GetNegotiatedProtocol)
        );
        assert!(
            DarwinNetworkAdapterFunction::ALL
                .contains(&DarwinNetworkAdapterFunction::NetworkRelease)
        );
    }
}
