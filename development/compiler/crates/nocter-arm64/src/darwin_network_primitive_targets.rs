use std::collections::BTreeSet;
use std::fmt;

use nocter_machine::{
    MachineArgumentLocation, MachineLayoutKind, MachinePrimitiveTarget, MachineResultAbi,
    MachineResultLocation, MachineValueClass,
};
use nocter_runtime_contract::{
    DarwinNetworkConnectionEventObservationAbiSchema,
    DarwinNetworkListenerEventObservationAbiSchema, DarwinNetworkOwnerAbiSchema,
    DarwinNetworkOwnerCreateStatus, PrimitiveRole,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DarwinNetworkAdapterImports,
    Arm64DarwinNetworkConnectionError, Arm64DarwinNetworkConnectionTargets,
    Arm64DarwinNetworkListenerError, Arm64DarwinNetworkListenerTargets,
    Arm64DarwinTlsConnectionError, Arm64DataRegister, Arm64DataSize, Arm64FunctionId,
    Arm64Instruction, Arm64LoadStoreSize, Arm64ProgramBuilder, Arm64ProgramError, Arm64Register,
    add_darwin_plain_connection_targets, add_darwin_plain_listener_targets,
    add_darwin_tls_connection_create_target,
};

/// One source primitive in the closed plain-connection adapter family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64DarwinNetworkPrimitive {
    Create,
    TlsCreate,
    Start,
    EventDescriptor,
    BeginReceive,
    BeginSend,
    ReceiveEvent,
    CopyLocalAddress,
    CopyRemoteAddress,
    RequestCancel,
    ReleaseBarrier,
    Release,
    ListenerCreate,
    ListenerStart,
    ListenerEventDescriptor,
    ListenerReceiveEvent,
    ListenerPort,
    ListenerRequestCancel,
    ListenerReleaseBarrier,
    ListenerRelease,
}

/// The source ABIs whose aggregate result layouts must be known while declaring native targets.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Arm64DarwinNetworkPrimitiveAbis<'program> {
    connection_create: Option<&'program MachinePrimitiveTarget>,
    tls_connection_create: Option<&'program MachinePrimitiveTarget>,
    connection_receive_event: Option<&'program MachinePrimitiveTarget>,
    listener_create: Option<&'program MachinePrimitiveTarget>,
    listener_receive_event: Option<&'program MachinePrimitiveTarget>,
}

impl<'program> Arm64DarwinNetworkPrimitiveAbis<'program> {
    pub(crate) fn remember(
        &mut self,
        machine: &nocter_machine::MachineProgram,
        target: &'program MachinePrimitiveTarget,
    ) -> Result<(), Arm64DarwinNetworkPrimitiveError> {
        let slot = match target.role() {
            PrimitiveRole::NetworkConnectionCreate => &mut self.connection_create,
            PrimitiveRole::NetworkTlsConnectionCreate => &mut self.tls_connection_create,
            PrimitiveRole::NetworkConnectionReceiveEvent => &mut self.connection_receive_event,
            PrimitiveRole::NetworkListenerCreate => &mut self.listener_create,
            PrimitiveRole::NetworkListenerReceiveEvent => &mut self.listener_receive_event,
            _ => return Ok(()),
        };
        if let Some(existing) = *slot {
            if machine.primitive_abi(existing) != machine.primitive_abi(target) {
                return Err(Arm64DarwinNetworkPrimitiveError::PrimitiveAbi);
            }
        } else {
            *slot = Some(target);
        }
        Ok(())
    }
}

impl Arm64DarwinNetworkPrimitive {
    pub(crate) const fn from_role(role: PrimitiveRole) -> Option<Self> {
        match role {
            PrimitiveRole::NetworkConnectionCreate => Some(Self::Create),
            PrimitiveRole::NetworkTlsConnectionCreate => Some(Self::TlsCreate),
            PrimitiveRole::NetworkConnectionStart => Some(Self::Start),
            PrimitiveRole::NetworkConnectionEventDescriptor => Some(Self::EventDescriptor),
            PrimitiveRole::NetworkConnectionBeginReceive => Some(Self::BeginReceive),
            PrimitiveRole::NetworkConnectionBeginSend => Some(Self::BeginSend),
            PrimitiveRole::NetworkConnectionReceiveEvent => Some(Self::ReceiveEvent),
            PrimitiveRole::NetworkConnectionCopyLocalAddress => Some(Self::CopyLocalAddress),
            PrimitiveRole::NetworkConnectionCopyRemoteAddress => Some(Self::CopyRemoteAddress),
            PrimitiveRole::NetworkConnectionRequestCancel => Some(Self::RequestCancel),
            PrimitiveRole::NetworkConnectionReleaseBarrier => Some(Self::ReleaseBarrier),
            PrimitiveRole::NetworkConnectionRelease => Some(Self::Release),
            PrimitiveRole::NetworkListenerCreate => Some(Self::ListenerCreate),
            PrimitiveRole::NetworkListenerStart => Some(Self::ListenerStart),
            PrimitiveRole::NetworkListenerEventDescriptor => Some(Self::ListenerEventDescriptor),
            PrimitiveRole::NetworkListenerReceiveEvent => Some(Self::ListenerReceiveEvent),
            PrimitiveRole::NetworkListenerPort => Some(Self::ListenerPort),
            PrimitiveRole::NetworkListenerRequestCancel => Some(Self::ListenerRequestCancel),
            PrimitiveRole::NetworkListenerReleaseBarrier => Some(Self::ListenerReleaseBarrier),
            PrimitiveRole::NetworkListenerRelease => Some(Self::ListenerRelease),
            _ => None,
        }
    }
}

/// Source-ABI entries backed by one atomically declared production connection target set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinNetworkPrimitiveTargets {
    source_connection_create: Option<Arm64FunctionId>,
    source_tls_connection_create: Option<Arm64FunctionId>,
    source_listener_create: Option<Arm64FunctionId>,
    connection: Arm64DarwinNetworkConnectionTargets,
    listener: Option<Arm64DarwinNetworkListenerTargets>,
}

impl Arm64DarwinNetworkPrimitiveTargets {
    pub(crate) fn declare(
        machine: &nocter_machine::MachineProgram,
        roles: &BTreeSet<PrimitiveRole>,
        abis: Arm64DarwinNetworkPrimitiveAbis<'_>,
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
        let connection = add_darwin_plain_connection_targets(builder, &imports)?;
        validate_connection_event_result(machine, roles, abis.connection_receive_event)?;
        let source_connection_create = abis
            .connection_create
            .map(|target| {
                let layout = creation_layout(machine, target, 1)?;
                let wrapper = builder.declare_function();
                builder.define_function(
                    wrapper,
                    source_create_code(connection.create(), layout, 1)?,
                )?;
                Ok::<Arm64FunctionId, Arm64DarwinNetworkPrimitiveError>(wrapper)
            })
            .transpose()?;
        let source_tls_connection_create = abis
            .tls_connection_create
            .map(|target| {
                let layout = creation_layout(machine, target, 3)?;
                let production =
                    add_darwin_tls_connection_create_target(builder, &imports, connection)?;
                let wrapper = builder.declare_function();
                builder.define_function(wrapper, source_create_code(production, layout, 3)?)?;
                Ok::<Arm64FunctionId, Arm64DarwinNetworkPrimitiveError>(wrapper)
            })
            .transpose()?;
        let listener = roles
            .iter()
            .copied()
            .any(is_listener_role)
            .then(|| {
                add_darwin_plain_listener_targets(builder, &imports, connection.adopt_accepted())
            })
            .transpose()?;
        validate_listener_event_result(machine, roles, abis.listener_receive_event)?;
        let source_listener_create = match (abis.listener_create, listener) {
            (Some(target), Some(listener)) => {
                let layout = creation_layout(machine, target, 1)?;
                let wrapper = builder.declare_function();
                builder
                    .define_function(wrapper, source_create_code(listener.create(), layout, 1)?)?;
                Some(wrapper)
            }
            (None, _) => None,
            (Some(_), None) => return Err(Arm64DarwinNetworkPrimitiveError::PrimitiveAbi),
        };
        Ok(Some(Self {
            source_connection_create,
            source_tls_connection_create,
            source_listener_create,
            connection,
            listener,
        }))
    }

    #[must_use]
    pub fn target(self, primitive: Arm64DarwinNetworkPrimitive) -> Option<Arm64FunctionId> {
        let lifecycle = self.connection.lifecycle();
        let events = self.connection.events();
        match primitive {
            Arm64DarwinNetworkPrimitive::Create => self.source_connection_create,
            Arm64DarwinNetworkPrimitive::TlsCreate => self.source_tls_connection_create,
            Arm64DarwinNetworkPrimitive::Start => Some(lifecycle.start()),
            Arm64DarwinNetworkPrimitive::EventDescriptor => Some(events.descriptor()),
            Arm64DarwinNetworkPrimitive::BeginReceive => {
                Some(self.connection.transfers().begin_receive())
            }
            Arm64DarwinNetworkPrimitive::BeginSend => {
                Some(self.connection.transfers().begin_send())
            }
            Arm64DarwinNetworkPrimitive::ReceiveEvent => Some(events.receive()),
            Arm64DarwinNetworkPrimitive::CopyLocalAddress => {
                Some(self.connection.addresses().local())
            }
            Arm64DarwinNetworkPrimitive::CopyRemoteAddress => {
                Some(self.connection.addresses().remote())
            }
            Arm64DarwinNetworkPrimitive::RequestCancel => Some(lifecycle.request_cancel()),
            Arm64DarwinNetworkPrimitive::ReleaseBarrier => {
                Some(lifecycle.complete_release_barrier())
            }
            Arm64DarwinNetworkPrimitive::Release => Some(lifecycle.release()),
            Arm64DarwinNetworkPrimitive::ListenerCreate => self.source_listener_create,
            Arm64DarwinNetworkPrimitive::ListenerStart => {
                self.listener.map(|listener| listener.lifecycle().start())
            }
            Arm64DarwinNetworkPrimitive::ListenerEventDescriptor => self
                .listener
                .map(Arm64DarwinNetworkListenerTargets::event_descriptor),
            Arm64DarwinNetworkPrimitive::ListenerReceiveEvent => self
                .listener
                .map(|listener| listener.receive_event().function()),
            Arm64DarwinNetworkPrimitive::ListenerPort => {
                self.listener.map(Arm64DarwinNetworkListenerTargets::port)
            }
            Arm64DarwinNetworkPrimitive::ListenerRequestCancel => self
                .listener
                .map(|listener| listener.lifecycle().request_cancel()),
            Arm64DarwinNetworkPrimitive::ListenerReleaseBarrier => self
                .listener
                .map(|listener| listener.lifecycle().complete_release_barrier()),
            Arm64DarwinNetworkPrimitive::ListenerRelease => {
                self.listener.map(|listener| listener.lifecycle().release())
            }
        }
    }
}

const fn is_listener_role(role: PrimitiveRole) -> bool {
    matches!(
        role,
        PrimitiveRole::NetworkListenerCreate
            | PrimitiveRole::NetworkListenerStart
            | PrimitiveRole::NetworkListenerEventDescriptor
            | PrimitiveRole::NetworkListenerReceiveEvent
            | PrimitiveRole::NetworkListenerPort
            | PrimitiveRole::NetworkListenerRequestCancel
            | PrimitiveRole::NetworkListenerReleaseBarrier
            | PrimitiveRole::NetworkListenerRelease
    )
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
    argument_words: u8,
) -> Result<CreationLayout, Arm64DarwinNetworkPrimitiveError> {
    let abi = machine
        .primitive_abi(target)
        .ok_or(Arm64DarwinNetworkPrimitiveError::PrimitiveAbi)?;
    if abi.arguments().len() != usize::from(argument_words)
        || abi
            .arguments()
            .iter()
            .zip(0u8..)
            .any(|(argument, register)| {
                argument.class() != (MachineValueClass::Direct { words: 1 })
                    || !matches!(
                        argument.location(),
                        Some(MachineArgumentLocation::Registers(registers))
                            if registers.first() == register && registers.words() == 1
                    )
            })
    {
        return Err(Arm64DarwinNetworkPrimitiveError::PrimitiveAbi);
    }
    if abi.pack().is_some() || abi.stack_argument_size() != 0 {
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

fn validate_connection_event_result(
    machine: &nocter_machine::MachineProgram,
    roles: &BTreeSet<PrimitiveRole>,
    target: Option<&MachinePrimitiveTarget>,
) -> Result<(), Arm64DarwinNetworkPrimitiveError> {
    if !roles.contains(&PrimitiveRole::NetworkConnectionReceiveEvent) {
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
    let schema = DarwinNetworkConnectionEventObservationAbiSchema::ARM64_DARWIN;
    if result.class() != MachineValueClass::Indirect
        || result.location()
            != (MachineResultLocation::CallerStorage {
                pointer_register: 8,
            })
        || elements.len() != 5
        || elements[0].offset() != schema.kind_offset()
        || elements
            .iter()
            .skip(1)
            .enumerate()
            .any(|(lane, element)| schema.value_offset(lane) != Some(element.offset()))
        || layout.size() != schema.size()
        || layout.alignment() != schema.alignment()
    {
        return Err(Arm64DarwinNetworkPrimitiveError::ResultLayout);
    }
    Ok(())
}

fn validate_listener_event_result(
    machine: &nocter_machine::MachineProgram,
    roles: &BTreeSet<PrimitiveRole>,
    target: Option<&MachinePrimitiveTarget>,
) -> Result<(), Arm64DarwinNetworkPrimitiveError> {
    if !roles.contains(&PrimitiveRole::NetworkListenerReceiveEvent) {
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
    let schema = DarwinNetworkListenerEventObservationAbiSchema::ARM64_DARWIN;
    if result.class() != MachineValueClass::Indirect
        || result.location()
            != (MachineResultLocation::CallerStorage {
                pointer_register: 8,
            })
        || elements.len() != 5
        || elements[0].offset() != schema.kind_offset()
        || elements[1..4]
            .iter()
            .enumerate()
            .any(|(lane, element)| schema.value_offset(lane) != Some(element.offset()))
        || elements[4].offset() != schema.accepted_tag_offset()
        || layout.size() != schema.size()
        || layout.alignment() != schema.alignment()
    {
        return Err(Arm64DarwinNetworkPrimitiveError::ResultLayout);
    }
    let optional = machine
        .layouts()
        .get(elements[4].ty())
        .ok_or(Arm64DarwinNetworkPrimitiveError::ResultLayout)?;
    let MachineLayoutKind::Outcome {
        kind: nocter_machine::MachineOutcomeKind::Optional,
        tag_offset,
        payload_offset,
        primary: Some(owner),
        alternate: None,
    } = optional.kind()
    else {
        return Err(Arm64DarwinNetworkPrimitiveError::ResultLayout);
    };
    let owner_layout = machine
        .layouts()
        .get(*owner)
        .ok_or(Arm64DarwinNetworkPrimitiveError::ResultLayout)?;
    let owner_schema = DarwinNetworkOwnerAbiSchema::ARM64_DARWIN;
    if elements[4].offset().checked_add(*tag_offset) != Some(schema.accepted_tag_offset())
        || elements[4].offset().checked_add(*payload_offset) != Some(schema.accepted_owner_offset())
        || optional.size() != schema.accepted_optional_size()
        || optional.alignment() != schema.alignment()
        || owner_layout.size() != owner_schema.size()
        || owner_layout.alignment() != owner_schema.alignment()
    {
        return Err(Arm64DarwinNetworkPrimitiveError::ResultLayout);
    }
    Ok(())
}

fn source_create_code(
    production: Arm64FunctionId,
    layout: CreationLayout,
    argument_words: u8,
) -> Result<crate::Arm64Code, Arm64DarwinNetworkPrimitiveError> {
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract);
    for (register, offset) in [(x(19), 0), (x(30), 8)] {
        store_stack(&mut code, register, offset);
    }
    for register in 0..argument_words {
        store_stack(&mut code, x(register), 16 + u32::from(register) * 8);
    }
    move_register(&mut code, x(19), x(8));
    add_immediate(&mut code, x(0), x(19), layout.payload_offset);
    for register in 0..argument_words {
        load_stack(&mut code, x(register + 1), 16 + u32::from(register) * 8);
    }
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
    for (register, offset) in [(x(19), 0), (x(30), 8)] {
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
        immediate: 48,
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
    Listener(Arm64DarwinNetworkListenerError),
    Tls(Arm64DarwinTlsConnectionError),
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
            Self::Listener(error) => Some(error),
            Self::Tls(error) => Some(error),
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

impl From<Arm64DarwinNetworkListenerError> for Arm64DarwinNetworkPrimitiveError {
    fn from(error: Arm64DarwinNetworkListenerError) -> Self {
        Self::Listener(error)
    }
}

impl From<Arm64DarwinTlsConnectionError> for Arm64DarwinNetworkPrimitiveError {
    fn from(error: Arm64DarwinTlsConnectionError) -> Self {
        Self::Tls(error)
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
