use std::fmt;

use nocter_runtime_contract::{DarwinNetworkAdapterFunction, DarwinNetworkChannelIoContract};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DataRegister, Arm64DataSize, Arm64FunctionImportId,
    Arm64Instruction, Arm64LoadStoreSize, Arm64ProgramBuilder, Arm64ProgramError, Arm64Register,
};

/// Imported calls required by the fixed Darwin network event channel.
///
/// Construction selects every loader identity from the runtime catalog. Callers cannot substitute
/// a symbol while retaining this typed channel capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinNetworkChannelImports {
    send: Arm64FunctionImportId,
    receive: Arm64FunctionImportId,
    error_address: Arm64FunctionImportId,
    abort: Arm64FunctionImportId,
}

impl Arm64DarwinNetworkChannelImports {
    /// Adds the deduplicated imports required by callback-channel transfer.
    ///
    /// # Errors
    ///
    /// Propagates an ARM64 program data or alignment failure.
    pub fn declare(program: &mut Arm64ProgramBuilder) -> Result<Self, Arm64ProgramError> {
        Ok(Self {
            send: program.add_function_import(DarwinNetworkAdapterFunction::Send.import())?,
            receive: program.add_function_import(DarwinNetworkAdapterFunction::Receive.import())?,
            error_address: program
                .add_function_import(DarwinNetworkAdapterFunction::ErrorAddress.import())?,
            abort: program.add_function_import(DarwinNetworkAdapterFunction::Abort.import())?,
        })
    }
}

/// Sends the complete event at `event_stack_offset`, retrying only interrupted transfers.
///
/// Any short datagram or permanent channel failure is fatal because losing an ownership-bearing
/// provider event would make safe connection progress or release impossible. The emitted sequence
/// clobbers `x0` through `x3`, `x8`, `x16`, and condition flags.
///
/// # Errors
///
/// Rejects an event stack offset outside the immediate addressing form or an invalid local label.
pub fn emit_darwin_network_event_send(
    code: &mut Arm64CodeBuilder,
    imports: Arm64DarwinNetworkChannelImports,
    writer: Arm64Register,
    event_stack_offset: u32,
) -> Result<(), Arm64DarwinNetworkChannelError> {
    emit_transfer(code, imports, imports.send, writer, event_stack_offset)
}

/// Receives one complete event at `event_stack_offset`, retrying only interrupted transfers.
///
/// EOF, short datagrams, and permanent channel failures are fatal because no higher layer can
/// reconstruct a missing ownership-bearing event. The emitted sequence clobbers `x0` through `x3`,
/// `x8`, `x16`, and condition flags.
///
/// # Errors
///
/// Rejects an event stack offset outside the immediate addressing form or an invalid local label.
pub fn emit_darwin_network_event_receive(
    code: &mut Arm64CodeBuilder,
    imports: Arm64DarwinNetworkChannelImports,
    reader: Arm64Register,
    event_stack_offset: u32,
) -> Result<(), Arm64DarwinNetworkChannelError> {
    emit_transfer(code, imports, imports.receive, reader, event_stack_offset)
}

fn emit_transfer(
    code: &mut Arm64CodeBuilder,
    imports: Arm64DarwinNetworkChannelImports,
    transfer: Arm64FunctionImportId,
    descriptor: Arm64Register,
    event_stack_offset: u32,
) -> Result<(), Arm64DarwinNetworkChannelError> {
    if !(19..=28).contains(&descriptor.number()) {
        return Err(Arm64DarwinNetworkChannelError::VolatileDescriptorRegister(
            descriptor,
        ));
    }
    let offset = u16::try_from(event_stack_offset)
        .ok()
        .filter(|offset| *offset <= 4095)
        .ok_or(Arm64DarwinNetworkChannelError::StackOffset(
            event_stack_offset,
        ))?;
    let io = DarwinNetworkChannelIoContract::ARM64_DARWIN;
    let complete_count = u16::try_from(io.complete_count())
        .map_err(|_| Arm64DarwinNetworkChannelError::ContractLayout)?;
    let interrupted_errno = u16::try_from(io.interrupted_errno())
        .map_err(|_| Arm64DarwinNetworkChannelError::ContractLayout)?;
    let retry = code.create_label();
    let complete = code.create_label();
    let fatal = code.create_label();

    code.bind(retry)?;
    move_register(code, register(0), descriptor);
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(register(1)),
        source: Arm64BaseRegister::StackPointer,
        immediate: offset,
        shift_12: false,
    });
    move_immediate(code, register(2), complete_count);
    move_immediate(code, register(3), 0);
    call_import(code, transfer);

    compare_immediate(code, register(0), complete_count);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    compare_negative_one(code, register(0));
    code.branch_conditional(fatal, Arm64BranchCondition::NotEqual);
    call_import(code, imports.error_address);
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Word,
        destination: Arm64DataRegister::General(register(8)),
        base: Arm64BaseRegister::General(register(0)),
        offset: 0,
    });
    compare_immediate(code, register(8), interrupted_errno);
    code.branch_conditional(retry, Arm64BranchCondition::Equal);

    code.bind(fatal)?;
    call_import(code, imports.abort);
    code.bind(complete)?;
    Ok(())
}

fn register(number: u8) -> Arm64Register {
    Arm64Register::new(number).expect("closed ARM64 register is valid")
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

fn move_immediate(code: &mut Arm64CodeBuilder, destination: Arm64Register, immediate: u16) {
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: crate::Arm64MoveWide::Zero,
        destination,
        immediate,
        shift: 0,
    });
}

fn compare_immediate(code: &mut Arm64CodeBuilder, value: Arm64Register, immediate: u16) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate,
        shift_12: false,
    });
}

fn compare_negative_one(code: &mut Arm64CodeBuilder, value: Arm64Register) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: 1,
        shift_12: false,
    });
}

fn call_import(code: &mut Arm64CodeBuilder, target: Arm64FunctionImportId) {
    code.load_function_import(target, register(16));
    code.append(Arm64Instruction::BranchRegister {
        target: register(16),
        link: true,
    });
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64DarwinNetworkChannelError {
    StackOffset(u32),
    VolatileDescriptorRegister(Arm64Register),
    ContractLayout,
    Program(Arm64ProgramError),
    Code(Arm64CodeError),
}

impl fmt::Display for Arm64DarwinNetworkChannelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin network channel failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinNetworkChannelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Program(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::StackOffset(_) | Self::VolatileDescriptorRegister(_) | Self::ContractLayout => {
                None
            }
        }
    }
}

impl From<Arm64ProgramError> for Arm64DarwinNetworkChannelError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}

impl From<Arm64CodeError> for Arm64DarwinNetworkChannelError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Arm64DarwinNetworkChannelError, Arm64DarwinNetworkChannelImports,
        emit_darwin_network_event_send,
    };
    use crate::{Arm64CodeBuilder, Arm64ProgramBuilder, Arm64Register};

    #[test]
    fn channel_imports_are_catalog_owned_and_deduplicated() {
        let mut program = Arm64ProgramBuilder::new();
        let first = Arm64DarwinNetworkChannelImports::declare(&mut program).unwrap();
        let second = Arm64DarwinNetworkChannelImports::declare(&mut program).unwrap();
        assert_eq!(first, second);

        let mut code = Arm64CodeBuilder::new();
        emit_darwin_network_event_send(&mut code, first, Arm64Register::new(19).unwrap(), 0)
            .unwrap();
        code.finish().unwrap();

        let mut invalid = Arm64CodeBuilder::new();
        assert_eq!(
            emit_darwin_network_event_send(&mut invalid, first, Arm64Register::new(0).unwrap(), 0,),
            Err(Arm64DarwinNetworkChannelError::VolatileDescriptorRegister(
                Arm64Register::new(0).unwrap()
            ))
        );
    }
}
