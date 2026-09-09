use std::fmt;

use nocter_runtime_contract::{
    DarwinNetworkAdapterFunction, DarwinNetworkAdapterOperation, DarwinNetworkOwnerAbiSchema,
    DarwinNetworkOwnerField, DarwinNetworkOwnerKind, DarwinNetworkSocketAddressAbiSchema,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DarwinNetworkAdapterImports,
    Arm64DarwinNetworkOwnerError, Arm64DataRegister, Arm64DataSize, Arm64FunctionId,
    Arm64Instruction, Arm64LoadStoreSize, Arm64ProgramBuilder, Arm64ProgramError, Arm64Register,
    emit_darwin_network_owner_guard,
};

/// Callable targets that copy effective connection addresses out of provider ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinNetworkConnectionAddressTargets {
    local: Arm64FunctionId,
    remote: Arm64FunctionId,
}

impl Arm64DarwinNetworkConnectionAddressTargets {
    #[must_use]
    pub const fn local(self) -> Arm64FunctionId {
        self.local
    }

    #[must_use]
    pub const fn remote(self) -> Arm64FunctionId {
        self.remote
    }
}

#[derive(Clone, Copy)]
enum AddressKind {
    Local,
    Remote,
}

/// Adds address targets with the source ABI `(owner, destination) -> copied_length`.
///
/// A zero result means that no complete supported address is currently available. A non-zero
/// result is exactly the length byte of a copied Darwin IPv4 or IPv6 socket-address record.
/// Provider paths and endpoints are released before either result crosses the boundary.
pub(crate) fn add_darwin_network_connection_address_targets(
    program: &mut Arm64ProgramBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<Arm64DarwinNetworkConnectionAddressTargets, Arm64DarwinNetworkConnectionAddressError> {
    let local = program.declare_function();
    let remote = program.declare_function();
    program.define_function(local, address_code(AddressKind::Local, imports)?)?;
    program.define_function(remote, address_code(AddressKind::Remote, imports)?)?;
    Ok(Arm64DarwinNetworkConnectionAddressTargets { local, remote })
}

#[allow(
    clippy::too_many_lines,
    reason = "the provider ownership and reverse-order cleanup path must remain visible together"
)]
fn address_code(
    kind: AddressKind,
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<crate::Arm64Code, Arm64DarwinNetworkConnectionAddressError> {
    const FRAME_SIZE: u16 = 64;
    let saved = [
        (x(19), 8),
        (x(20), 16),
        (x(21), 24),
        (x(22), 32),
        (x(23), 40),
        (x(24), 48),
        (x(30), 56),
    ];
    let schema = DarwinNetworkSocketAddressAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, FRAME_SIZE);
    for (register, offset) in saved {
        store_stack(&mut code, register, offset);
    }
    move_register(&mut code, x(19), x(0));
    move_register(&mut code, x(20), x(1));
    emit_darwin_network_owner_guard(
        &mut code,
        x(19),
        DarwinNetworkOwnerKind::Connection,
        DarwinNetworkAdapterOperation::CopyAddress,
        imports,
    )?;

    let path_ready = code.create_label();
    let endpoint_ready = code.create_label();
    let address_ready = code.create_label();
    let check_ipv6 = code.create_label();
    let valid_length = code.create_label();
    let invalid_address = code.create_label();
    let address_copied = code.create_label();
    let release_endpoint = code.create_label();
    let release_path = code.create_label();
    let complete = code.create_label();

    load_owner_field(
        &mut code,
        x(0),
        x(19),
        DarwinNetworkOwnerField::NativeObject,
    )?;
    call(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::ConnectionCopyCurrentPath),
    );
    move_register(&mut code, x(21), x(0));
    compare_zero(&mut code, x(21));
    code.branch_conditional(path_ready, Arm64BranchCondition::NotEqual);
    immediate(&mut code, x(24), 0)?;
    code.branch(complete, false);
    code.bind(path_ready)?;

    move_register(&mut code, x(0), x(21));
    call(
        &mut code,
        imports.function(match kind {
            AddressKind::Local => DarwinNetworkAdapterFunction::PathCopyEffectiveLocalEndpoint,
            AddressKind::Remote => DarwinNetworkAdapterFunction::PathCopyEffectiveRemoteEndpoint,
        }),
    );
    move_register(&mut code, x(22), x(0));
    compare_zero(&mut code, x(22));
    code.branch_conditional(endpoint_ready, Arm64BranchCondition::NotEqual);
    immediate(&mut code, x(24), 0)?;
    code.branch(release_path, false);
    code.bind(endpoint_ready)?;

    move_register(&mut code, x(0), x(22));
    call(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::EndpointGetAddress),
    );
    move_register(&mut code, x(23), x(0));
    compare_zero(&mut code, x(23));
    code.branch_conditional(address_ready, Arm64BranchCondition::NotEqual);
    immediate(&mut code, x(24), 0)?;
    code.branch(release_endpoint, false);
    code.bind(address_ready)?;

    load_byte(&mut code, x(24), x(23), schema.length_offset())?;
    load_byte(&mut code, x(8), x(23), schema.family_offset())?;
    let (ipv4_size, ipv4_family) = schema.ipv4();
    let (ipv6_size, ipv6_family) = schema.ipv6();
    compare_immediate(&mut code, x(24), ipv4_size)?;
    code.branch_conditional(check_ipv6, Arm64BranchCondition::NotEqual);
    compare_immediate(&mut code, x(8), ipv4_family)?;
    code.branch_conditional(valid_length, Arm64BranchCondition::Equal);
    code.branch(invalid_address, false);
    code.bind(check_ipv6)?;
    compare_immediate(&mut code, x(24), ipv6_size)?;
    code.branch_conditional(invalid_address, Arm64BranchCondition::NotEqual);
    compare_immediate(&mut code, x(8), ipv6_family)?;
    code.branch_conditional(valid_length, Arm64BranchCondition::Equal);
    code.bind(invalid_address)?;
    immediate(&mut code, x(24), 0)?;
    code.branch(release_endpoint, false);
    code.bind(valid_length)?;
    for offset in 0..16 {
        load_byte(&mut code, x(8), x(23), offset)?;
        store_byte(&mut code, x(8), x(20), offset)?;
    }
    compare_immediate(&mut code, x(24), 16)?;
    code.branch_conditional(address_copied, Arm64BranchCondition::Equal);
    for offset in 16..schema.maximum_size() {
        load_byte(&mut code, x(8), x(23), offset)?;
        store_byte(&mut code, x(8), x(20), offset)?;
    }
    code.bind(address_copied)?;

    code.bind(release_endpoint)?;
    release_network_object(&mut code, imports, x(22));
    code.bind(release_path)?;
    release_network_object(&mut code, imports, x(21));
    code.bind(complete)?;
    move_register(&mut code, x(0), x(24));
    for (register, offset) in saved {
        load_stack(&mut code, register, offset);
    }
    adjust_stack(&mut code, Arm64AddSubtract::Add, FRAME_SIZE);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
    code.finish().map_err(Into::into)
}

fn load_owner_field(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    owner: Arm64Register,
    field: DarwinNetworkOwnerField,
) -> Result<(), Arm64DarwinNetworkConnectionAddressError> {
    let offset = u32::try_from(DarwinNetworkOwnerAbiSchema::ARM64_DARWIN.offset(field))
        .map_err(|_| Arm64DarwinNetworkConnectionAddressError::ContractLayout)?;
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(owner),
        offset,
    });
    Ok(())
}

fn load_byte(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    base: Arm64Register,
    offset: u64,
) -> Result<(), Arm64DarwinNetworkConnectionAddressError> {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Byte,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(base),
        offset: u32::try_from(offset)
            .map_err(|_| Arm64DarwinNetworkConnectionAddressError::ContractLayout)?,
    });
    Ok(())
}

fn store_byte(
    code: &mut Arm64CodeBuilder,
    source: Arm64Register,
    base: Arm64Register,
    offset: u64,
) -> Result<(), Arm64DarwinNetworkConnectionAddressError> {
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Byte,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::General(base),
        offset: u32::try_from(offset)
            .map_err(|_| Arm64DarwinNetworkConnectionAddressError::ContractLayout)?,
    });
    Ok(())
}

fn release_network_object(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    object: Arm64Register,
) {
    move_register(code, x(0), object);
    call(
        code,
        imports.function(DarwinNetworkAdapterFunction::NetworkRelease),
    );
}

fn call(code: &mut Arm64CodeBuilder, target: crate::Arm64FunctionImportId) {
    code.load_function_import(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
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

fn compare_immediate(
    code: &mut Arm64CodeBuilder,
    value: Arm64Register,
    expected: u64,
) -> Result<(), Arm64DarwinNetworkConnectionAddressError> {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: u16::try_from(expected)
            .map_err(|_| Arm64DarwinNetworkConnectionAddressError::ContractLayout)?,
        shift_12: false,
    });
    Ok(())
}

fn immediate(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    value: u64,
) -> Result<(), Arm64DarwinNetworkConnectionAddressError> {
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: crate::Arm64MoveWide::Zero,
        destination,
        immediate: u16::try_from(value)
            .map_err(|_| Arm64DarwinNetworkConnectionAddressError::ContractLayout)?,
        shift: 0,
    });
    Ok(())
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

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).expect("closed ARM64 register is valid")
}

#[derive(Debug)]
pub enum Arm64DarwinNetworkConnectionAddressError {
    ContractLayout,
    Owner(Arm64DarwinNetworkOwnerError),
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinNetworkConnectionAddressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin network address failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinNetworkConnectionAddressError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Owner(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ContractLayout => None,
        }
    }
}

impl From<Arm64DarwinNetworkOwnerError> for Arm64DarwinNetworkConnectionAddressError {
    fn from(error: Arm64DarwinNetworkOwnerError) -> Self {
        Self::Owner(error)
    }
}

impl From<Arm64CodeError> for Arm64DarwinNetworkConnectionAddressError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinNetworkConnectionAddressError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}

#[cfg(test)]
mod tests {
    use super::add_darwin_network_connection_address_targets;
    use crate::{Arm64DarwinNetworkAdapterImports, Arm64ProgramBuilder};

    #[test]
    fn local_and_remote_addresses_have_distinct_targets() {
        let mut program = Arm64ProgramBuilder::new();
        let imports = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
        let targets =
            add_darwin_network_connection_address_targets(&mut program, &imports).unwrap();
        assert_ne!(targets.local(), targets.remote());
    }
}
