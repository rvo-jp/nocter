use std::fmt;

use nocter_runtime_contract::{DarwinNetworkAdapterFunction, DarwinNetworkOwnerCreateStatus};

use crate::darwin_network_owner_creation::{
    emit_darwin_network_close_descriptor, emit_darwin_network_create_channel,
    emit_darwin_network_create_serial_queue, emit_darwin_network_install_pointer_handler,
    emit_darwin_network_release_network_object, emit_darwin_network_set_owner_queue,
};
use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DarwinBlockDescriptorId, Arm64DarwinBlockError,
    Arm64DarwinNetworkAdapterImports, Arm64DarwinNetworkOwnerError,
    Arm64DarwinNetworkOwnerResources, Arm64DataRegister, Arm64DataSize, Arm64FunctionId,
    Arm64Instruction, Arm64LoadStoreSize, Arm64ProgramBuilder, Arm64ProgramError, Arm64Register,
    emit_darwin_network_owner_initialize,
};

/// A target that wholly consumes one retained accepted Network.framework connection.
///
/// It either publishes a complete connection owner or releases the native connection and every
/// partial resource. The typed identity prevents listener code from calling an arbitrary target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinAcceptedConnectionAdoptionTarget(Arm64FunctionId);

impl Arm64DarwinAcceptedConnectionAdoptionTarget {
    #[must_use]
    pub(crate) const fn function(self) -> Arm64FunctionId {
        self.0
    }
}

/// Adds the sole retained-connection adoption target.
///
/// The target accepts `(owner destination, retained native connection)` in `x0..x1` and returns a
/// [`DarwinNetworkOwnerCreateStatus`] code in `x0`.
///
/// # Errors
///
/// Propagates fixed Block, owner-layout, and ARM64 program/code construction failures.
pub(crate) fn add_darwin_accepted_connection_adoption_target(
    program: &mut Arm64ProgramBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    state_callback: Arm64FunctionId,
    state_block: Arm64DarwinBlockDescriptorId,
    queue_label: crate::Arm64DataId,
) -> Result<Arm64DarwinAcceptedConnectionAdoptionTarget, Arm64DarwinConnectionAdoptionError> {
    let target = program.declare_function();
    program.define_function(
        target,
        adoption_code(imports, state_callback, state_block, queue_label)?.finish()?,
    )?;
    Ok(Arm64DarwinAcceptedConnectionAdoptionTarget(target))
}

fn adoption_code(
    imports: &Arm64DarwinNetworkAdapterImports,
    state_callback: Arm64FunctionId,
    state_block: Arm64DarwinBlockDescriptorId,
    queue_label: crate::Arm64DataId,
) -> Result<Arm64CodeBuilder, Arm64DarwinConnectionAdoptionError> {
    const FRAME_SIZE: u16 = 112;
    const BLOCK_OFFSET: u32 = 16;
    let saved = [
        (x(19), 48),
        (x(20), 56),
        (x(21), 64),
        (x(22), 72),
        (x(23), 80),
        (x(27), 88),
        (x(30), 104),
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
    let cleanup_channel = code.create_label();
    let cleanup_connection = code.create_label();
    let complete = code.create_label();

    emit_darwin_network_create_channel(&mut code, imports, 0);
    compare_zero(&mut code, x(0));
    code.branch_conditional(channel_ready, Arm64BranchCondition::Equal);
    status(
        &mut code,
        DarwinNetworkOwnerCreateStatus::ChannelUnavailable,
    )?;
    code.branch(cleanup_connection, false);
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

    emit_darwin_network_install_pointer_handler(
        &mut code,
        imports,
        x(20),
        x(22),
        state_callback,
        state_block,
        BLOCK_OFFSET,
        DarwinNetworkAdapterFunction::ConnectionSetStateHandler,
    )?;
    emit_darwin_network_set_owner_queue(
        &mut code,
        imports,
        x(20),
        x(23),
        DarwinNetworkAdapterFunction::ConnectionSetQueue,
    );
    emit_darwin_network_owner_initialize(
        &mut code,
        x(19),
        Arm64DarwinNetworkOwnerResources::new(x(20), x(23), x(21), x(22)),
    )?;
    status(&mut code, DarwinNetworkOwnerCreateStatus::Created)?;
    code.branch(complete, false);

    code.bind(cleanup_channel)?;
    emit_darwin_network_close_descriptor(&mut code, imports, x(22));
    emit_darwin_network_close_descriptor(&mut code, imports, x(21));
    code.bind(cleanup_connection)?;
    emit_darwin_network_release_network_object(&mut code, imports, x(20));

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
) -> Result<(), Arm64DarwinConnectionAdoptionError> {
    let immediate = u16::try_from(status.code())
        .map_err(|_| Arm64DarwinConnectionAdoptionError::ContractLayout)?;
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: crate::Arm64MoveWide::Zero,
        destination: x(27),
        immediate,
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
pub enum Arm64DarwinConnectionAdoptionError {
    ContractLayout,
    Block(Arm64DarwinBlockError),
    Owner(Arm64DarwinNetworkOwnerError),
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinConnectionAdoptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ARM64 Darwin accepted connection adoption failed: {self:?}"
        )
    }
}

impl std::error::Error for Arm64DarwinConnectionAdoptionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Block(error) => Some(error),
            Self::Owner(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ContractLayout => None,
        }
    }
}

macro_rules! convert_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for Arm64DarwinConnectionAdoptionError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}

convert_error!(Arm64DarwinBlockError, Block);
convert_error!(Arm64DarwinNetworkOwnerError, Owner);
convert_error!(Arm64CodeError, Code);
convert_error!(Arm64ProgramError, Program);
