use std::fmt;

use nocter_runtime_contract::{
    DarwinNetworkAdapterData, DarwinNetworkAdapterFunction, DarwinNetworkCallbackRole,
    DarwinNetworkOwnerCreateStatus, DarwinNetworkOwnerKind,
};

use crate::darwin_network_listener_event::add_darwin_network_listener_event_target;
use crate::darwin_network_owner_creation::{
    emit_darwin_network_close_descriptor, emit_darwin_network_create_channel,
    emit_darwin_network_create_serial_queue, emit_darwin_network_install_pointer_handler,
    emit_darwin_network_load_imported_object, emit_darwin_network_release_dispatch_object,
    emit_darwin_network_release_network_object, emit_darwin_network_set_owner_queue,
};
use crate::darwin_network_owner_event::add_darwin_network_owner_event_descriptor_target;
use crate::darwin_network_owner_lifecycle::add_darwin_network_owner_lifecycle_targets;
use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DarwinAcceptedConnectionAdoptionTarget,
    Arm64DarwinBlockDescriptorId, Arm64DarwinBlockError, Arm64DarwinNetworkAdapterImports,
    Arm64DarwinNetworkCallbackError, Arm64DarwinNetworkListenerEventError,
    Arm64DarwinNetworkListenerEventTarget, Arm64DarwinNetworkOwnerError,
    Arm64DarwinNetworkOwnerEventError, Arm64DarwinNetworkOwnerLifecycleError,
    Arm64DarwinNetworkOwnerLifecycleTargets, Arm64DarwinNetworkOwnerResources, Arm64DataRegister,
    Arm64DataSize, Arm64FunctionId, Arm64Instruction, Arm64LoadStoreSize, Arm64ProgramBuilder,
    Arm64ProgramError, Arm64Register, add_darwin_network_completion_callback,
    add_darwin_network_state_callback, add_darwin_pointer_capture_block_descriptor,
    emit_darwin_network_owner_initialize,
};

/// Native construction and lifecycle entries for one plain Network.framework listener.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinNetworkListenerTargets {
    create: Arm64FunctionId,
    state_callback: Arm64FunctionId,
    accept_callback: Arm64FunctionId,
    state_block: Arm64DarwinBlockDescriptorId,
    accept_block: Arm64DarwinBlockDescriptorId,
    event_descriptor: Arm64FunctionId,
    receive_event: Arm64DarwinNetworkListenerEventTarget,
    lifecycle: Arm64DarwinNetworkOwnerLifecycleTargets,
}

impl Arm64DarwinNetworkListenerTargets {
    #[must_use]
    pub const fn create(self) -> Arm64FunctionId {
        self.create
    }

    #[must_use]
    pub const fn state_callback(self) -> Arm64FunctionId {
        self.state_callback
    }

    #[must_use]
    pub const fn accept_callback(self) -> Arm64FunctionId {
        self.accept_callback
    }

    #[must_use]
    pub const fn state_block(self) -> Arm64DarwinBlockDescriptorId {
        self.state_block
    }

    #[must_use]
    pub const fn accept_block(self) -> Arm64DarwinBlockDescriptorId {
        self.accept_block
    }

    #[must_use]
    pub const fn event_descriptor(self) -> Arm64FunctionId {
        self.event_descriptor
    }

    #[must_use]
    pub const fn receive_event(self) -> Arm64DarwinNetworkListenerEventTarget {
        self.receive_event
    }

    #[must_use]
    pub const fn lifecycle(self) -> Arm64DarwinNetworkOwnerLifecycleTargets {
        self.lifecycle
    }
}

/// Adds the fixed plain listener constructor, callbacks, event descriptor, and terminal lifecycle.
///
/// The constructor accepts `(owner destination, native socket address)` in `x0..x1` and returns
/// [`DarwinNetworkOwnerCreateStatus::code`] in `x0`. No partially created provider, dispatch, or
/// descriptor resource crosses the publication boundary.
///
/// # Errors
///
/// Propagates runtime-contract layout and ARM64 program/code construction failures.
pub fn add_darwin_plain_listener_targets(
    program: &mut Arm64ProgramBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    adopt_accepted: Arm64DarwinAcceptedConnectionAdoptionTarget,
) -> Result<Arm64DarwinNetworkListenerTargets, Arm64DarwinNetworkListenerError> {
    let state_block = add_darwin_pointer_capture_block_descriptor(
        program,
        DarwinNetworkCallbackRole::ListenerState.block_signature(),
    )?;
    let accept_block = add_darwin_pointer_capture_block_descriptor(
        program,
        DarwinNetworkCallbackRole::ListenerAccept.block_signature(),
    )?;
    let state_callback = add_darwin_network_state_callback(
        program,
        DarwinNetworkCallbackRole::ListenerState,
        imports,
    )?;
    let accept_callback = add_darwin_network_completion_callback(
        program,
        DarwinNetworkCallbackRole::ListenerAccept,
        imports,
    )?;
    let queue_label = program.add_data(b"nocter.network.listener\0".as_slice(), 1)?;
    let create = program.declare_function();
    let code = listener_create_code(
        imports,
        state_callback,
        state_block,
        accept_callback,
        accept_block,
        queue_label,
    )?;
    program.define_function(create, code.finish()?)?;
    let event_descriptor = add_darwin_network_owner_event_descriptor_target(
        program,
        imports,
        DarwinNetworkOwnerKind::Listener,
    )?;
    let receive_event = add_darwin_network_listener_event_target(program, imports, adopt_accepted)?;
    let lifecycle = add_darwin_network_owner_lifecycle_targets(
        program,
        imports,
        DarwinNetworkOwnerKind::Listener,
        DarwinNetworkAdapterFunction::ListenerStart,
        DarwinNetworkAdapterFunction::ListenerCancel,
    )?;
    Ok(Arm64DarwinNetworkListenerTargets {
        create,
        state_callback,
        accept_callback,
        state_block,
        accept_block,
        event_descriptor,
        receive_event,
        lifecycle,
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "the success path and reverse-order partial-resource cleanup are one atomic proof"
)]
fn listener_create_code(
    imports: &Arm64DarwinNetworkAdapterImports,
    state_callback: Arm64FunctionId,
    state_block: Arm64DarwinBlockDescriptorId,
    accept_callback: Arm64FunctionId,
    accept_block: Arm64DarwinBlockDescriptorId,
    queue_label: crate::Arm64DataId,
) -> Result<Arm64CodeBuilder, Arm64DarwinNetworkListenerError> {
    const FRAME_SIZE: u16 = 192;
    const STATE_BLOCK_OFFSET: u32 = 16;
    const ACCEPT_BLOCK_OFFSET: u32 = 56;
    let saved = [
        (x(19), 104),
        (x(20), 112),
        (x(21), 120),
        (x(22), 128),
        (x(23), 136),
        (x(24), 144),
        (x(25), 152),
        (x(26), 160),
        (x(27), 168),
        (x(30), 184),
    ];
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, FRAME_SIZE);
    for (register, offset) in saved {
        store_stack(&mut code, register, offset);
    }
    move_register(&mut code, x(19), x(0));
    move_register(&mut code, x(20), x(1));

    let channel_ready = code.create_label();
    let queue_ready = code.create_label();
    let endpoint_ready = code.create_label();
    let parameters_ready = code.create_label();
    let listener_ready = code.create_label();
    let cleanup_parameters = code.create_label();
    let cleanup_endpoint = code.create_label();
    let cleanup_queue = code.create_label();
    let cleanup_channel = code.create_label();
    let complete = code.create_label();

    emit_darwin_network_create_channel(&mut code, imports, 0);
    compare_zero(&mut code, x(0));
    code.branch_conditional(channel_ready, Arm64BranchCondition::Equal);
    status(
        &mut code,
        DarwinNetworkOwnerCreateStatus::ChannelUnavailable,
    )?;
    code.branch(complete, false);
    code.bind(channel_ready)?;
    load_stack_word(&mut code, x(21), 0);
    load_stack_word(&mut code, x(22), 4);

    emit_darwin_network_create_serial_queue(&mut code, imports, queue_label);
    compare_zero(&mut code, x(0));
    code.branch_conditional(queue_ready, Arm64BranchCondition::NotEqual);
    status(&mut code, DarwinNetworkOwnerCreateStatus::QueueUnavailable)?;
    code.branch(cleanup_channel, false);
    code.bind(queue_ready)?;
    move_register(&mut code, x(23), x(0));

    move_register(&mut code, x(0), x(20));
    call_import(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::EndpointCreateAddress),
    );
    compare_zero(&mut code, x(0));
    code.branch_conditional(endpoint_ready, Arm64BranchCondition::NotEqual);
    status(
        &mut code,
        DarwinNetworkOwnerCreateStatus::EndpointUnavailable,
    )?;
    code.branch(cleanup_queue, false);
    code.bind(endpoint_ready)?;
    move_register(&mut code, x(24), x(0));

    emit_darwin_network_load_imported_object(
        &mut code,
        imports.data(DarwinNetworkAdapterData::DisableProtocolConfiguration),
        x(0),
    );
    emit_darwin_network_load_imported_object(
        &mut code,
        imports.data(DarwinNetworkAdapterData::DefaultProtocolConfiguration),
        x(1),
    );
    call_import(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::ParametersCreateSecureTcp),
    );
    compare_zero(&mut code, x(0));
    code.branch_conditional(parameters_ready, Arm64BranchCondition::NotEqual);
    status(
        &mut code,
        DarwinNetworkOwnerCreateStatus::ParametersUnavailable,
    )?;
    code.branch(cleanup_endpoint, false);
    code.bind(parameters_ready)?;
    move_register(&mut code, x(25), x(0));

    move_register(&mut code, x(0), x(25));
    move_register(&mut code, x(1), x(24));
    call_import(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::ParametersSetLocalEndpoint),
    );
    move_register(&mut code, x(0), x(25));
    call_import(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::ListenerCreate),
    );
    compare_zero(&mut code, x(0));
    code.branch_conditional(listener_ready, Arm64BranchCondition::NotEqual);
    status(
        &mut code,
        DarwinNetworkOwnerCreateStatus::NativeOwnerUnavailable,
    )?;
    code.branch(cleanup_parameters, false);
    code.bind(listener_ready)?;
    move_register(&mut code, x(26), x(0));

    emit_darwin_network_install_pointer_handler(
        &mut code,
        imports,
        x(26),
        x(22),
        state_callback,
        state_block,
        STATE_BLOCK_OFFSET,
        DarwinNetworkAdapterFunction::ListenerSetStateHandler,
    )?;
    emit_darwin_network_install_pointer_handler(
        &mut code,
        imports,
        x(26),
        x(22),
        accept_callback,
        accept_block,
        ACCEPT_BLOCK_OFFSET,
        DarwinNetworkAdapterFunction::ListenerSetNewConnectionHandler,
    )?;
    emit_darwin_network_set_owner_queue(
        &mut code,
        imports,
        x(26),
        x(23),
        DarwinNetworkAdapterFunction::ListenerSetQueue,
    );
    emit_darwin_network_release_network_object(&mut code, imports, x(25));
    emit_darwin_network_release_network_object(&mut code, imports, x(24));
    emit_darwin_network_owner_initialize(
        &mut code,
        x(19),
        Arm64DarwinNetworkOwnerResources::new(x(26), x(23), x(21), x(22)),
    )?;
    status(&mut code, DarwinNetworkOwnerCreateStatus::Created)?;
    code.branch(complete, false);

    code.bind(cleanup_parameters)?;
    emit_darwin_network_release_network_object(&mut code, imports, x(25));
    code.bind(cleanup_endpoint)?;
    emit_darwin_network_release_network_object(&mut code, imports, x(24));
    code.bind(cleanup_queue)?;
    emit_darwin_network_release_dispatch_object(&mut code, imports, x(23));
    code.bind(cleanup_channel)?;
    emit_darwin_network_close_descriptor(&mut code, imports, x(22));
    emit_darwin_network_close_descriptor(&mut code, imports, x(21));

    code.bind(complete)?;
    move_register(&mut code, x(0), x(27));
    for (register, offset) in saved {
        load_stack(&mut code, register, offset);
    }
    adjust_stack(&mut code, Arm64AddSubtract::Add, FRAME_SIZE);
    return_from_function(&mut code);
    Ok(code)
}

fn status(
    code: &mut Arm64CodeBuilder,
    status: DarwinNetworkOwnerCreateStatus,
) -> Result<(), Arm64DarwinNetworkListenerError> {
    immediate(code, x(27), status.code())
}

fn immediate(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    value: u64,
) -> Result<(), Arm64DarwinNetworkListenerError> {
    let value =
        u16::try_from(value).map_err(|_| Arm64DarwinNetworkListenerError::ContractLayout)?;
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: crate::Arm64MoveWide::Zero,
        destination,
        immediate: value,
        shift: 0,
    });
    Ok(())
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

fn load_stack_word(code: &mut Arm64CodeBuilder, destination: Arm64Register, offset: u32) {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Word,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::StackPointer,
        offset,
    });
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

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).expect("closed ARM64 register is valid")
}

#[derive(Debug)]
pub enum Arm64DarwinNetworkListenerError {
    ContractLayout,
    Block(Arm64DarwinBlockError),
    Callback(Arm64DarwinNetworkCallbackError),
    Owner(Arm64DarwinNetworkOwnerError),
    OwnerEvent(Arm64DarwinNetworkOwnerEventError),
    Event(Arm64DarwinNetworkListenerEventError),
    Lifecycle(Arm64DarwinNetworkOwnerLifecycleError),
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinNetworkListenerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin network listener failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinNetworkListenerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Block(error) => Some(error),
            Self::Callback(error) => Some(error),
            Self::Owner(error) => Some(error),
            Self::OwnerEvent(error) => Some(error),
            Self::Event(error) => Some(error),
            Self::Lifecycle(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ContractLayout => None,
        }
    }
}

macro_rules! convert_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for Arm64DarwinNetworkListenerError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}

convert_error!(Arm64DarwinBlockError, Block);
convert_error!(Arm64DarwinNetworkCallbackError, Callback);
convert_error!(Arm64DarwinNetworkOwnerError, Owner);
convert_error!(Arm64DarwinNetworkOwnerEventError, OwnerEvent);
convert_error!(Arm64DarwinNetworkListenerEventError, Event);
convert_error!(Arm64DarwinNetworkOwnerLifecycleError, Lifecycle);
convert_error!(Arm64CodeError, Code);
convert_error!(Arm64ProgramError, Program);

#[cfg(test)]
mod tests {
    use super::add_darwin_plain_listener_targets;
    use crate::{
        Arm64DarwinNetworkAdapterImports, Arm64ProgramBuilder, add_darwin_plain_connection_targets,
    };

    #[test]
    fn listener_construction_and_lifecycle_targets_are_distinct() {
        let mut program = Arm64ProgramBuilder::new();
        let imports = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
        let connection = add_darwin_plain_connection_targets(&mut program, &imports).unwrap();
        let targets =
            add_darwin_plain_listener_targets(&mut program, &imports, connection.adopt_accepted())
                .unwrap();
        let lifecycle = targets.lifecycle();
        let functions = [
            targets.create(),
            targets.state_callback(),
            targets.accept_callback(),
            targets.event_descriptor(),
            targets.receive_event().function(),
            lifecycle.start(),
            lifecycle.request_cancel(),
            lifecycle.complete_release_barrier(),
            lifecycle.release(),
        ];
        for (index, function) in functions.iter().enumerate() {
            assert!(!functions[..index].contains(function));
        }
        assert_ne!(targets.state_block(), targets.accept_block());
    }
}
