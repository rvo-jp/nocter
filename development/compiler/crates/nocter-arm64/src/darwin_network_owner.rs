use std::fmt;

use nocter_runtime_contract::{
    DarwinNetworkAdapterFunction, DarwinNetworkAdapterOperation, DarwinNetworkOwnerAbiSchema,
    DarwinNetworkOwnerField, DarwinNetworkOwnerKind, DarwinNetworkOwnerResourceFamily,
    DarwinNetworkOwnerState,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DarwinNetworkAdapterImports, Arm64DataRegister,
    Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize, Arm64Register,
};

/// Registers containing the resources published into one fully initialized owner record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinNetworkOwnerResources {
    native_object: Arm64Register,
    serial_queue: Arm64Register,
    event_reader: Arm64Register,
    event_writer: Arm64Register,
}

impl Arm64DarwinNetworkOwnerResources {
    #[must_use]
    pub const fn new(
        native_object: Arm64Register,
        serial_queue: Arm64Register,
        event_reader: Arm64Register,
        event_writer: Arm64Register,
    ) -> Self {
        Self {
            native_object,
            serial_queue,
            event_reader,
            event_writer,
        }
    }
}

/// Publishes one fully initialized native owner record.
///
/// Partial construction must be cleaned before this boundary. Once published, terminal cleanup is
/// owned exclusively by [`emit_darwin_network_owner_release`].
///
/// # Errors
///
/// Rejects a volatile owner-base register, an aliased resource register, or a malformed runtime
/// owner schema.
pub fn emit_darwin_network_owner_initialize(
    code: &mut Arm64CodeBuilder,
    owner: Arm64Register,
    resources: Arm64DarwinNetworkOwnerResources,
) -> Result<(), Arm64DarwinNetworkOwnerError> {
    validate_owner_register(owner)?;
    let resource_registers = [
        resources.native_object,
        resources.serial_queue,
        resources.event_reader,
        resources.event_writer,
    ];
    for (index, resource) in resource_registers.iter().copied().enumerate() {
        if resource == owner {
            return Err(Arm64DarwinNetworkOwnerError::AliasedResource(owner));
        }
        if resource_registers[..index].contains(&resource) {
            return Err(Arm64DarwinNetworkOwnerError::DuplicateResourceRegister(
                resource,
            ));
        }
    }
    let schema = DarwinNetworkOwnerAbiSchema::ARM64_DARWIN;
    for (field, source) in [
        (
            DarwinNetworkOwnerField::NativeObject,
            resources.native_object,
        ),
        (DarwinNetworkOwnerField::SerialQueue, resources.serial_queue),
        (DarwinNetworkOwnerField::EventReader, resources.event_reader),
        (DarwinNetworkOwnerField::EventWriter, resources.event_writer),
    ] {
        store(code, owner, field_offset(schema, field)?, source);
    }
    immediate(code, x(8), DarwinNetworkOwnerState::Initialized.code())?;
    store(
        code,
        owner,
        field_offset(schema, DarwinNetworkOwnerField::Lifecycle)?,
        x(8),
    );
    Ok(())
}

/// Emits one runtime-checked lifecycle transition from the closed operation authority.
///
/// # Errors
///
/// Rejects an operation with no deterministic uniform next state, a volatile owner register, or a
/// malformed runtime owner schema.
pub fn emit_darwin_network_owner_transition(
    code: &mut Arm64CodeBuilder,
    owner: Arm64Register,
    kind: DarwinNetworkOwnerKind,
    operation: DarwinNetworkAdapterOperation,
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<(), Arm64DarwinNetworkOwnerError> {
    validate_owner_register(owner)?;
    let (accepted, next) = transition_domain(kind, operation)?;
    emit_state_guard(code, owner, &accepted, imports)?;
    immediate(code, x(8), next.code())?;
    store(
        code,
        owner,
        field_offset(
            DarwinNetworkOwnerAbiSchema::ARM64_DARWIN,
            DarwinNetworkOwnerField::Lifecycle,
        )?,
        x(8),
    );
    Ok(())
}

/// Releases every native owner resource after validating the complete release fence.
///
/// The operation is fail-stop on an invalid lifecycle. Each resource field is cleared after its
/// matching runtime-family release and the lifecycle becomes `released`, making a second release
/// fail before it can touch native storage.
///
/// # Errors
///
/// Rejects a volatile owner register or malformed owner/transition contracts.
pub fn emit_darwin_network_owner_release(
    code: &mut Arm64CodeBuilder,
    owner: Arm64Register,
    kind: DarwinNetworkOwnerKind,
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<(), Arm64DarwinNetworkOwnerError> {
    validate_owner_register(owner)?;
    let (accepted, next) = transition_domain(kind, DarwinNetworkAdapterOperation::Release)?;
    emit_state_guard(code, owner, &accepted, imports)?;
    let schema = DarwinNetworkOwnerAbiSchema::ARM64_DARWIN;
    for field in DarwinNetworkOwnerAbiSchema::RELEASE_ORDER {
        let offset = field_offset(schema, *field)?;
        load(code, x(0), owner, offset);
        let role = match field.resource_family() {
            DarwinNetworkOwnerResourceFamily::NetworkObject => {
                DarwinNetworkAdapterFunction::NetworkRelease
            }
            DarwinNetworkOwnerResourceFamily::DispatchObject => {
                DarwinNetworkAdapterFunction::DispatchRelease
            }
            DarwinNetworkOwnerResourceFamily::FileDescriptor => DarwinNetworkAdapterFunction::Close,
            DarwinNetworkOwnerResourceFamily::Value => {
                return Err(Arm64DarwinNetworkOwnerError::InvalidReleaseField(*field));
            }
        };
        call(code, imports.function(role));
        store_zero(code, owner, offset);
    }
    immediate(code, x(8), next.code())?;
    store(
        code,
        owner,
        field_offset(schema, DarwinNetworkOwnerField::Lifecycle)?,
        x(8),
    );
    Ok(())
}

fn transition_domain(
    kind: DarwinNetworkOwnerKind,
    operation: DarwinNetworkAdapterOperation,
) -> Result<(Vec<DarwinNetworkOwnerState>, DarwinNetworkOwnerState), Arm64DarwinNetworkOwnerError> {
    let mut accepted = Vec::new();
    let mut next = None;
    for state in DarwinNetworkOwnerState::ALL.iter().copied() {
        let Ok(candidate) = operation.transition(kind, state) else {
            continue;
        };
        if next.is_some_and(|next| next != candidate) {
            return Err(Arm64DarwinNetworkOwnerError::NonUniformTransition(
                operation,
            ));
        }
        next = Some(candidate);
        accepted.push(state);
    }
    let next = next.ok_or(Arm64DarwinNetworkOwnerError::NoTransition(operation))?;
    Ok((accepted, next))
}

fn emit_state_guard(
    code: &mut Arm64CodeBuilder,
    owner: Arm64Register,
    accepted: &[DarwinNetworkOwnerState],
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<(), Arm64DarwinNetworkOwnerError> {
    let accepted_label = code.create_label();
    load(
        code,
        x(8),
        owner,
        field_offset(
            DarwinNetworkOwnerAbiSchema::ARM64_DARWIN,
            DarwinNetworkOwnerField::Lifecycle,
        )?,
    );
    for state in accepted {
        compare_immediate(code, x(8), state.code())?;
        code.branch_conditional(accepted_label, Arm64BranchCondition::Equal);
    }
    call(code, imports.function(DarwinNetworkAdapterFunction::Abort));
    code.bind(accepted_label)?;
    Ok(())
}

fn validate_owner_register(owner: Arm64Register) -> Result<(), Arm64DarwinNetworkOwnerError> {
    if (19..=28).contains(&owner.number()) {
        Ok(())
    } else {
        Err(Arm64DarwinNetworkOwnerError::VolatileOwnerRegister(owner))
    }
}

fn field_offset(
    schema: DarwinNetworkOwnerAbiSchema,
    field: DarwinNetworkOwnerField,
) -> Result<u32, Arm64DarwinNetworkOwnerError> {
    u32::try_from(schema.offset(field)).map_err(|_| Arm64DarwinNetworkOwnerError::ContractLayout)
}

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).expect("closed ARM64 register is valid")
}

fn immediate(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    value: u64,
) -> Result<(), Arm64DarwinNetworkOwnerError> {
    let value = u16::try_from(value).map_err(|_| Arm64DarwinNetworkOwnerError::ContractLayout)?;
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: crate::Arm64MoveWide::Zero,
        destination,
        immediate: value,
        shift: 0,
    });
    Ok(())
}

fn compare_immediate(
    code: &mut Arm64CodeBuilder,
    value: Arm64Register,
    expected: u64,
) -> Result<(), Arm64DarwinNetworkOwnerError> {
    let immediate =
        u16::try_from(expected).map_err(|_| Arm64DarwinNetworkOwnerError::ContractLayout)?;
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

fn store(code: &mut Arm64CodeBuilder, owner: Arm64Register, offset: u32, source: Arm64Register) {
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::General(owner),
        offset,
    });
}

fn store_zero(code: &mut Arm64CodeBuilder, owner: Arm64Register, offset: u32) {
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::Zero,
        base: Arm64BaseRegister::General(owner),
        offset,
    });
}

fn load(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    owner: Arm64Register,
    offset: u32,
) {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(owner),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Arm64DarwinNetworkOwnerError {
    VolatileOwnerRegister(Arm64Register),
    AliasedResource(Arm64Register),
    DuplicateResourceRegister(Arm64Register),
    ContractLayout,
    NoTransition(DarwinNetworkAdapterOperation),
    NonUniformTransition(DarwinNetworkAdapterOperation),
    InvalidReleaseField(DarwinNetworkOwnerField),
    Code(Arm64CodeError),
}

impl fmt::Display for Arm64DarwinNetworkOwnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin network owner failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinNetworkOwnerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Code(error) => Some(error),
            Self::VolatileOwnerRegister(_)
            | Self::AliasedResource(_)
            | Self::DuplicateResourceRegister(_)
            | Self::ContractLayout
            | Self::NoTransition(_)
            | Self::NonUniformTransition(_)
            | Self::InvalidReleaseField(_) => None,
        }
    }
}

impl From<Arm64CodeError> for Arm64DarwinNetworkOwnerError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

#[cfg(test)]
mod tests {
    use nocter_runtime_contract::{
        DarwinNetworkAdapterOperation, DarwinNetworkOwnerKind, DarwinNetworkOwnerState,
    };

    use super::{
        Arm64DarwinNetworkOwnerError, Arm64DarwinNetworkOwnerResources,
        emit_darwin_network_owner_initialize, emit_darwin_network_owner_transition,
    };
    use crate::{
        Arm64CodeBuilder, Arm64DarwinNetworkAdapterImports, Arm64ProgramBuilder, Arm64Register,
    };

    #[test]
    fn owner_emitters_reject_volatile_or_non_transition_inputs() {
        let mut program = Arm64ProgramBuilder::new();
        let imports = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
        let resources = Arm64DarwinNetworkOwnerResources::new(x(20), x(21), x(22), x(23));
        assert_eq!(
            emit_darwin_network_owner_initialize(&mut Arm64CodeBuilder::new(), x(0), resources),
            Err(Arm64DarwinNetworkOwnerError::VolatileOwnerRegister(x(0)))
        );
        let duplicate = Arm64DarwinNetworkOwnerResources::new(x(20), x(20), x(22), x(23));
        assert_eq!(
            emit_darwin_network_owner_initialize(&mut Arm64CodeBuilder::new(), x(19), duplicate),
            Err(Arm64DarwinNetworkOwnerError::DuplicateResourceRegister(x(
                20
            )))
        );
        assert_eq!(
            emit_darwin_network_owner_transition(
                &mut Arm64CodeBuilder::new(),
                x(19),
                DarwinNetworkOwnerKind::Connection,
                DarwinNetworkAdapterOperation::EventDescriptor,
                &imports,
            ),
            Err(Arm64DarwinNetworkOwnerError::NonUniformTransition(
                DarwinNetworkAdapterOperation::EventDescriptor
            ))
        );
        assert_eq!(DarwinNetworkOwnerState::Released.code(), 5);
    }

    fn x(number: u8) -> Arm64Register {
        Arm64Register::new(number).unwrap()
    }
}
