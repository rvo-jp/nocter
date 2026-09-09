use std::collections::BTreeSet;
use std::fmt;

use nocter_machine::{
    MachineArgumentLocation, MachineLayoutKind, MachinePrimitiveTarget, MachineResultAbi,
    MachineResultLocation, MachineValueClass,
};
use nocter_runtime_contract::{
    DarwinNetworkConnectionStateObservationAbiSchema, DarwinNetworkOwnerAbiSchema,
    DarwinNetworkOwnerCreateStatus, PrimitiveRole,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DarwinNetworkAdapterImports,
    Arm64DarwinNetworkConnectionError, Arm64DarwinNetworkConnectionTargets, Arm64DataRegister,
    Arm64DataSize, Arm64FunctionId, Arm64Instruction, Arm64LoadStoreSize, Arm64ProgramBuilder,
    Arm64ProgramError, Arm64Register, add_darwin_plain_connection_targets,
};

/// One source primitive in the closed plain-connection adapter family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64DarwinNetworkPrimitive {
    Create,
    Start,
    EventDescriptor,
    ReceiveState,
    CopyLocalAddress,
    CopyRemoteAddress,
    RequestCancel,
    ReleaseBarrier,
    Release,
}

impl Arm64DarwinNetworkPrimitive {
    pub(crate) const fn from_role(role: PrimitiveRole) -> Option<Self> {
        match role {
            PrimitiveRole::NetworkConnectionCreate => Some(Self::Create),
            PrimitiveRole::NetworkConnectionStart => Some(Self::Start),
            PrimitiveRole::NetworkConnectionEventDescriptor => Some(Self::EventDescriptor),
            PrimitiveRole::NetworkConnectionReceiveState => Some(Self::ReceiveState),
            PrimitiveRole::NetworkConnectionCopyLocalAddress => Some(Self::CopyLocalAddress),
            PrimitiveRole::NetworkConnectionCopyRemoteAddress => Some(Self::CopyRemoteAddress),
            PrimitiveRole::NetworkConnectionRequestCancel => Some(Self::RequestCancel),
            PrimitiveRole::NetworkConnectionReleaseBarrier => Some(Self::ReleaseBarrier),
            PrimitiveRole::NetworkConnectionRelease => Some(Self::Release),
            _ => None,
        }
    }
}

/// Source-ABI entries backed by one atomically declared production connection target set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinNetworkPrimitiveTargets {
    source_create: Option<Arm64FunctionId>,
    production: Arm64DarwinNetworkConnectionTargets,
}

impl Arm64DarwinNetworkPrimitiveTargets {
    pub(crate) fn declare(
        machine: &nocter_machine::MachineProgram,
        roles: &BTreeSet<PrimitiveRole>,
        create: Option<&MachinePrimitiveTarget>,
        receive_state: Option<&MachinePrimitiveTarget>,
        builder: &mut Arm64ProgramBuilder,
    ) -> Result<Option<Self>, Arm64DarwinNetworkPrimitiveError> {
        if !roles
            .iter()
            .copied()
            .any(|role| Arm64DarwinNetworkPrimitive::from_role(role).is_some())
        {
            return Ok(None);
        }
        let imports = Arm64DarwinNetworkAdapterImports::declare(builder)?;
        let production = add_darwin_plain_connection_targets(builder, &imports)?;
        validate_state_result(machine, roles, receive_state)?;
        let source_create = create
            .map(|target| {
                let layout = creation_layout(machine, target)?;
                let wrapper = builder.declare_function();
                builder
                    .define_function(wrapper, source_create_code(production.create(), layout)?)?;
                Ok::<Arm64FunctionId, Arm64DarwinNetworkPrimitiveError>(wrapper)
            })
            .transpose()?;
        Ok(Some(Self {
            source_create,
            production,
        }))
    }

    #[must_use]
    pub const fn target(self, primitive: Arm64DarwinNetworkPrimitive) -> Option<Arm64FunctionId> {
        let lifecycle = self.production.lifecycle();
        let events = self.production.events();
        match primitive {
            Arm64DarwinNetworkPrimitive::Create => self.source_create,
            Arm64DarwinNetworkPrimitive::Start => Some(lifecycle.start()),
            Arm64DarwinNetworkPrimitive::EventDescriptor => Some(events.descriptor()),
            Arm64DarwinNetworkPrimitive::ReceiveState => Some(events.receive_state()),
            Arm64DarwinNetworkPrimitive::CopyLocalAddress => {
                Some(self.production.addresses().local())
            }
            Arm64DarwinNetworkPrimitive::CopyRemoteAddress => {
                Some(self.production.addresses().remote())
            }
            Arm64DarwinNetworkPrimitive::RequestCancel => Some(lifecycle.request_cancel()),
            Arm64DarwinNetworkPrimitive::ReleaseBarrier => {
                Some(lifecycle.complete_release_barrier())
            }
            Arm64DarwinNetworkPrimitive::Release => Some(lifecycle.release()),
        }
    }
}

#[derive(Clone, Copy)]
struct CreationLayout {
    tag_offset: u32,
    payload_offset: u16,
    present_tag: u64,
    absent_tag: u64,
}

fn creation_layout(
    machine: &nocter_machine::MachineProgram,
    target: &MachinePrimitiveTarget,
) -> Result<CreationLayout, Arm64DarwinNetworkPrimitiveError> {
    let abi = machine
        .primitive_abi(target)
        .ok_or(Arm64DarwinNetworkPrimitiveError::PrimitiveAbi)?;
    let [argument] = abi.arguments() else {
        return Err(Arm64DarwinNetworkPrimitiveError::PrimitiveAbi);
    };
    if argument.class() != (MachineValueClass::Direct { words: 1 })
        || !matches!(
            argument.location(),
            Some(MachineArgumentLocation::Registers(registers))
                if registers.first() == 0 && registers.words() == 1
        )
        || abi.pack().is_some()
        || abi.stack_argument_size() != 0
    {
        return Err(Arm64DarwinNetworkPrimitiveError::PrimitiveAbi);
    }
    let MachineResultAbi::Value(result) = abi.result() else {
        return Err(Arm64DarwinNetworkPrimitiveError::PrimitiveAbi);
    };
    if result.class() != MachineValueClass::Indirect
        || result.location()
            != (MachineResultLocation::CallerStorage {
                pointer_register: 8,
            })
    {
        return Err(Arm64DarwinNetworkPrimitiveError::PrimitiveAbi);
    }
    let layout = machine
        .layouts()
        .get(result.ty())
        .ok_or(Arm64DarwinNetworkPrimitiveError::ResultLayout)?;
    let MachineLayoutKind::Outcome {
        kind: nocter_machine::MachineOutcomeKind::Optional,
        tag_offset,
        payload_offset,
        primary: Some(owner),
        alternate: None,
    } = layout.kind()
    else {
        return Err(Arm64DarwinNetworkPrimitiveError::ResultLayout);
    };
    let owner_layout = machine
        .layouts()
        .get(*owner)
        .ok_or(Arm64DarwinNetworkPrimitiveError::ResultLayout)?;
    let expected = DarwinNetworkOwnerAbiSchema::ARM64_DARWIN;
    if owner_layout.size() != expected.size()
        || owner_layout.alignment() != expected.alignment()
        || payload_offset
            .checked_add(owner_layout.size())
            .is_none_or(|end| end > layout.size())
    {
        return Err(Arm64DarwinNetworkPrimitiveError::ResultLayout);
    }
    Ok(CreationLayout {
        tag_offset: u32::try_from(*tag_offset)
            .map_err(|_| Arm64DarwinNetworkPrimitiveError::ResultLayout)?,
        payload_offset: u16::try_from(*payload_offset)
            .map_err(|_| Arm64DarwinNetworkPrimitiveError::ResultLayout)?,
        present_tag: u64::from(nocter_machine::MachineOutcomeKind::Optional.primary_tag()),
        absent_tag: u64::from(nocter_machine::MachineOutcomeKind::Optional.alternate_tag()),
    })
}

fn validate_state_result(
    machine: &nocter_machine::MachineProgram,
    roles: &BTreeSet<PrimitiveRole>,
    target: Option<&MachinePrimitiveTarget>,
) -> Result<(), Arm64DarwinNetworkPrimitiveError> {
    if !roles.contains(&PrimitiveRole::NetworkConnectionReceiveState) {
        return Ok(());
    }
    let target = target.ok_or(Arm64DarwinNetworkPrimitiveError::PrimitiveAbi)?;
    let abi = machine
        .primitive_abi(target)
        .ok_or(Arm64DarwinNetworkPrimitiveError::PrimitiveAbi)?;
    let MachineResultAbi::Value(result) = abi.result() else {
        return Err(Arm64DarwinNetworkPrimitiveError::PrimitiveAbi);
    };
    let layout = machine
        .layouts()
        .get(result.ty())
        .ok_or(Arm64DarwinNetworkPrimitiveError::ResultLayout)?;
    let MachineLayoutKind::Tuple { elements } = layout.kind() else {
        return Err(Arm64DarwinNetworkPrimitiveError::ResultLayout);
    };
    let schema = DarwinNetworkConnectionStateObservationAbiSchema::ARM64_DARWIN;
    if result.class() != MachineValueClass::Indirect
        || result.location()
            != (MachineResultLocation::CallerStorage {
                pointer_register: 8,
            })
        || elements.len() != 3
        || elements[0].offset() != schema.state_offset()
        || elements[1].offset() != schema.error_domain_offset()
        || elements[2].offset() != schema.error_code_offset()
        || layout.size() != schema.size()
        || layout.alignment() != schema.alignment()
    {
        return Err(Arm64DarwinNetworkPrimitiveError::ResultLayout);
    }
    Ok(())
}

fn source_create_code(
    production: Arm64FunctionId,
    layout: CreationLayout,
) -> Result<crate::Arm64Code, Arm64DarwinNetworkPrimitiveError> {
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract);
    for (register, offset) in [(x(19), 0), (x(20), 8), (x(30), 16)] {
        store_stack(&mut code, register, offset);
    }
    move_register(&mut code, x(19), x(8));
    move_register(&mut code, x(20), x(0));
    add_immediate(&mut code, x(0), x(19), layout.payload_offset);
    move_register(&mut code, x(1), x(20));
    call_function(&mut code, production);
    compare_immediate(
        &mut code,
        x(0),
        DarwinNetworkOwnerCreateStatus::Created.code(),
    )?;
    let absent = code.create_label();
    let complete = code.create_label();
    code.branch_conditional(absent, Arm64BranchCondition::NotEqual);
    store_immediate(&mut code, x(19), layout.tag_offset, layout.present_tag)?;
    code.branch(complete, false);
    code.bind(absent)?;
    store_immediate(&mut code, x(19), layout.tag_offset, layout.absent_tag)?;
    code.bind(complete)?;
    for (register, offset) in [(x(19), 0), (x(20), 8), (x(30), 16)] {
        load_stack(&mut code, register, offset);
    }
    adjust_stack(&mut code, Arm64AddSubtract::Add);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
    code.finish().map_err(Into::into)
}

fn store_immediate(
    code: &mut Arm64CodeBuilder,
    base: Arm64Register,
    offset: u32,
    value: u64,
) -> Result<(), Arm64DarwinNetworkPrimitiveError> {
    let value = u16::try_from(value).map_err(|_| Arm64DarwinNetworkPrimitiveError::ResultLayout)?;
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: crate::Arm64MoveWide::Zero,
        destination: x(8),
        immediate: value,
        shift: 0,
    });
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Byte,
        source: Arm64DataRegister::General(x(8)),
        base: Arm64BaseRegister::General(base),
        offset,
    });
    Ok(())
}

fn compare_immediate(
    code: &mut Arm64CodeBuilder,
    value: Arm64Register,
    expected: u64,
) -> Result<(), Arm64DarwinNetworkPrimitiveError> {
    let immediate =
        u16::try_from(expected).map_err(|_| Arm64DarwinNetworkPrimitiveError::ResultLayout)?;
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

fn add_immediate(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    source: Arm64Register,
    immediate: u16,
) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::General(source),
        immediate,
        shift_12: false,
    });
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
    add_immediate(code, destination, source, 0);
}

fn call_function(code: &mut Arm64CodeBuilder, target: Arm64FunctionId) {
    code.load_function_address(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
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
pub enum Arm64DarwinNetworkPrimitiveError {
    PrimitiveAbi,
    ResultLayout,
    Connection(Arm64DarwinNetworkConnectionError),
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinNetworkPrimitiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin network primitive failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinNetworkPrimitiveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Connection(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::PrimitiveAbi | Self::ResultLayout => None,
        }
    }
}

impl From<Arm64DarwinNetworkConnectionError> for Arm64DarwinNetworkPrimitiveError {
    fn from(error: Arm64DarwinNetworkConnectionError) -> Self {
        Self::Connection(error)
    }
}

impl From<Arm64CodeError> for Arm64DarwinNetworkPrimitiveError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinNetworkPrimitiveError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}
