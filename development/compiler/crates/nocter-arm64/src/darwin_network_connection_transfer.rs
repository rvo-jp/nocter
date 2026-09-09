use std::fmt;

use nocter_runtime_contract::{
    DarwinNetworkAdapterData, DarwinNetworkAdapterFunction, DarwinNetworkAdapterOperation,
    DarwinNetworkCallbackRole, DarwinNetworkOwnerAbiSchema, DarwinNetworkOwnerField,
    DarwinNetworkOwnerKind,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DarwinBlockDescriptorId, Arm64DarwinBlockError,
    Arm64DarwinNetworkAdapterImports, Arm64DarwinNetworkCallbackError,
    Arm64DarwinNetworkOwnerError, Arm64DataRegister, Arm64DataSize, Arm64FunctionId,
    Arm64Instruction, Arm64LoadStoreSize, Arm64ProgramBuilder, Arm64ProgramError, Arm64Register,
    add_darwin_network_completion_callback, add_darwin_pointer_capture_block_descriptor,
    emit_darwin_network_owner_guard, load_darwin_stack_block_address,
    materialize_darwin_pointer_capture_stack_block,
};

/// Native entries and fixed callback metadata for connection data transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinNetworkConnectionTransferTargets {
    begin_receive: Arm64FunctionId,
    begin_send: Arm64FunctionId,
    receive_callback: Arm64FunctionId,
    send_callback: Arm64FunctionId,
}

impl Arm64DarwinNetworkConnectionTransferTargets {
    #[must_use]
    pub const fn begin_receive(self) -> Arm64FunctionId {
        self.begin_receive
    }

    #[must_use]
    pub const fn begin_send(self) -> Arm64FunctionId {
        self.begin_send
    }

    #[must_use]
    pub const fn receive_callback(self) -> Arm64FunctionId {
        self.receive_callback
    }

    #[must_use]
    pub const fn send_callback(self) -> Arm64FunctionId {
        self.send_callback
    }
}

/// Adds the two asynchronous provider-operation starters as one ownership unit.
///
/// `begin_receive` accepts `(owner, maximum_length)` in `x0..x1`; the length is a source `u32`.
/// `begin_send` accepts `(owner, bytes, length)` in `x0..x2` and returns one in `x0` after the
/// system has copied the bytes into a dispatch-data owner, or zero if that copy owner could not be
/// created. Both provider calls copy their stack Block before returning. Their sole capture is the
/// owner's independently lived callback-channel writer descriptor.
pub(crate) fn add_darwin_network_connection_transfer_targets(
    program: &mut Arm64ProgramBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<Arm64DarwinNetworkConnectionTransferTargets, Arm64DarwinNetworkConnectionTransferError>
{
    let receive_block = add_darwin_pointer_capture_block_descriptor(
        program,
        DarwinNetworkCallbackRole::ConnectionReceive.block_signature(),
    )?;
    let send_block = add_darwin_pointer_capture_block_descriptor(
        program,
        DarwinNetworkCallbackRole::ConnectionSend.block_signature(),
    )?;
    let receive_callback = add_darwin_network_completion_callback(
        program,
        DarwinNetworkCallbackRole::ConnectionReceive,
        imports,
    )?;
    let send_callback = add_darwin_network_completion_callback(
        program,
        DarwinNetworkCallbackRole::ConnectionSend,
        imports,
    )?;
    let begin_receive = program.declare_function();
    let begin_send = program.declare_function();
    program.define_function(
        begin_receive,
        begin_receive_code(imports, receive_callback, receive_block)?,
    )?;
    program.define_function(
        begin_send,
        begin_send_code(imports, send_callback, send_block)?,
    )?;
    Ok(Arm64DarwinNetworkConnectionTransferTargets {
        begin_receive,
        begin_send,
        receive_callback,
        send_callback,
    })
}

fn begin_receive_code(
    imports: &Arm64DarwinNetworkAdapterImports,
    callback: Arm64FunctionId,
    descriptor: Arm64DarwinBlockDescriptorId,
) -> Result<crate::Arm64Code, Arm64DarwinNetworkConnectionTransferError> {
    const FRAME_SIZE: u16 = 96;
    const BLOCK_OFFSET: u32 = 16;
    let saved = [(x(19), 64), (x(20), 72), (x(21), 80), (x(30), 88)];
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
        DarwinNetworkAdapterOperation::BeginReceive,
        imports,
    )?;
    load_owner_field(
        &mut code,
        x(21),
        x(19),
        DarwinNetworkOwnerField::EventWriter,
    )?;
    materialize_darwin_pointer_capture_stack_block(
        &mut code,
        BLOCK_OFFSET,
        imports.data(DarwinNetworkAdapterData::StackBlockClass),
        callback,
        descriptor,
        x(21),
        x(8),
    )?;
    load_owner_field(
        &mut code,
        x(0),
        x(19),
        DarwinNetworkOwnerField::NativeObject,
    )?;
    immediate(&mut code, x(1), 1)?;
    move_register(&mut code, x(2), x(20));
    load_darwin_stack_block_address(&mut code, BLOCK_OFFSET, x(3))?;
    call(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::ConnectionReceive),
    );
    finish(&mut code, &saved, FRAME_SIZE);
    code.finish().map_err(Into::into)
}

#[allow(
    clippy::too_many_lines,
    reason = "dispatch-data creation, provider transfer, and matching release are one ownership path"
)]
fn begin_send_code(
    imports: &Arm64DarwinNetworkAdapterImports,
    callback: Arm64FunctionId,
    descriptor: Arm64DarwinBlockDescriptorId,
) -> Result<crate::Arm64Code, Arm64DarwinNetworkConnectionTransferError> {
    const FRAME_SIZE: u16 = 112;
    const BLOCK_OFFSET: u32 = 16;
    let saved = [
        (x(19), 64),
        (x(20), 72),
        (x(21), 80),
        (x(22), 88),
        (x(23), 96),
        (x(30), 104),
    ];
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, FRAME_SIZE);
    for (register, offset) in saved {
        store_stack(&mut code, register, offset);
    }
    move_register(&mut code, x(19), x(0));
    move_register(&mut code, x(20), x(1));
    move_register(&mut code, x(21), x(2));
    emit_darwin_network_owner_guard(
        &mut code,
        x(19),
        DarwinNetworkOwnerKind::Connection,
        DarwinNetworkAdapterOperation::BeginSend,
        imports,
    )?;

    move_register(&mut code, x(0), x(20));
    move_register(&mut code, x(1), x(21));
    immediate(&mut code, x(2), 0)?;
    immediate(&mut code, x(3), 0)?;
    call(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::DispatchDataCreate),
    );
    move_register(&mut code, x(22), x(0));
    let data_ready = code.create_label();
    let complete = code.create_label();
    compare_zero(&mut code, x(22));
    code.branch_conditional(data_ready, Arm64BranchCondition::NotEqual);
    immediate(&mut code, x(23), 0)?;
    code.branch(complete, false);
    code.bind(data_ready)?;

    load_owner_field(
        &mut code,
        x(23),
        x(19),
        DarwinNetworkOwnerField::EventWriter,
    )?;
    materialize_darwin_pointer_capture_stack_block(
        &mut code,
        BLOCK_OFFSET,
        imports.data(DarwinNetworkAdapterData::StackBlockClass),
        callback,
        descriptor,
        x(23),
        x(8),
    )?;
    load_owner_field(
        &mut code,
        x(0),
        x(19),
        DarwinNetworkOwnerField::NativeObject,
    )?;
    move_register(&mut code, x(1), x(22));
    load_imported_object(
        &mut code,
        imports.data(DarwinNetworkAdapterData::DefaultMessageContext),
        x(2),
    );
    immediate(&mut code, x(3), 1)?;
    load_darwin_stack_block_address(&mut code, BLOCK_OFFSET, x(4))?;
    call(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::ConnectionSend),
    );
    move_register(&mut code, x(0), x(22));
    call(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::DispatchRelease),
    );
    immediate(&mut code, x(23), 1)?;
    code.bind(complete)?;
    move_register(&mut code, x(0), x(23));
    finish(&mut code, &saved, FRAME_SIZE);
    code.finish().map_err(Into::into)
}

fn load_owner_field(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    owner: Arm64Register,
    field: DarwinNetworkOwnerField,
) -> Result<(), Arm64DarwinNetworkConnectionTransferError> {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(owner),
        offset: u32::try_from(DarwinNetworkOwnerAbiSchema::ARM64_DARWIN.offset(field))
            .map_err(|_| Arm64DarwinNetworkConnectionTransferError::ContractLayout)?,
    });
    Ok(())
}

fn load_imported_object(
    code: &mut Arm64CodeBuilder,
    source: crate::Arm64DataImportId,
    destination: Arm64Register,
) {
    code.load_data_import(source, destination);
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(destination),
        offset: 0,
    });
}

fn finish(code: &mut Arm64CodeBuilder, saved: &[(Arm64Register, u32)], frame_size: u16) {
    for (register, offset) in saved {
        load_stack(code, *register, *offset);
    }
    adjust_stack(code, Arm64AddSubtract::Add, frame_size);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
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

fn immediate(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    value: u64,
) -> Result<(), Arm64DarwinNetworkConnectionTransferError> {
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: crate::Arm64MoveWide::Zero,
        destination,
        immediate: u16::try_from(value)
            .map_err(|_| Arm64DarwinNetworkConnectionTransferError::ContractLayout)?,
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
pub enum Arm64DarwinNetworkConnectionTransferError {
    ContractLayout,
    Block(Arm64DarwinBlockError),
    Callback(Arm64DarwinNetworkCallbackError),
    Owner(Arm64DarwinNetworkOwnerError),
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinNetworkConnectionTransferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin network transfer failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinNetworkConnectionTransferError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Block(error) => Some(error),
            Self::Callback(error) => Some(error),
            Self::Owner(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ContractLayout => None,
        }
    }
}

macro_rules! convert_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for Arm64DarwinNetworkConnectionTransferError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}

convert_error!(Arm64DarwinBlockError, Block);
convert_error!(Arm64DarwinNetworkCallbackError, Callback);
convert_error!(Arm64DarwinNetworkOwnerError, Owner);
convert_error!(Arm64CodeError, Code);
convert_error!(Arm64ProgramError, Program);

#[cfg(test)]
mod tests {
    use super::add_darwin_network_connection_transfer_targets;
    use crate::{Arm64DarwinNetworkAdapterImports, Arm64ProgramBuilder};

    #[test]
    fn transfer_starters_and_callbacks_are_distinct() {
        let mut program = Arm64ProgramBuilder::new();
        let imports = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
        let targets =
            add_darwin_network_connection_transfer_targets(&mut program, &imports).unwrap();
        let entries = [
            targets.begin_receive(),
            targets.begin_send(),
            targets.receive_callback(),
            targets.send_callback(),
        ];
        for (index, entry) in entries.iter().enumerate() {
            assert!(!entries[..index].contains(entry));
        }
    }
}
