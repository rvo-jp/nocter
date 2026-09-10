use nocter_runtime_contract::{DarwinNetworkAdapterData, DarwinNetworkAdapterFunction};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64CodeBuilder,
    Arm64DarwinBlockDescriptorId, Arm64DarwinBlockError, Arm64DarwinNetworkAdapterImports,
    Arm64DataRegister, Arm64DataSize, Arm64FunctionId, Arm64Instruction, Arm64LoadStoreSize,
    Arm64Register, load_darwin_stack_block_address, materialize_darwin_pointer_capture_stack_block,
};

/// Creates the fixed datagram callback channel into two adjacent stack words.
pub(crate) fn emit_darwin_network_create_channel(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    stack_offset: u16,
) {
    immediate(code, x(0), 1);
    immediate(code, x(1), 2);
    immediate(code, x(2), 0);
    stack_address(code, stack_offset, x(3));
    call(
        code,
        imports.function(DarwinNetworkAdapterFunction::SocketPair),
    );
}

/// Creates one serial callback queue with the supplied static label.
pub(crate) fn emit_darwin_network_create_serial_queue(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    label: crate::Arm64DataId,
) {
    code.load_data_address(label, x(0));
    immediate(code, x(1), 0);
    call(
        code,
        imports.function(DarwinNetworkAdapterFunction::DispatchQueueCreate),
    );
}

/// Installs one fixed pointer-capture callback on a native provider owner.
///
/// # Errors
///
/// Propagates fixed Block materialization or stack-address failures.
#[allow(
    clippy::too_many_arguments,
    reason = "native owner, callback identity, capture, and setter are independent ABI inputs"
)]
pub(crate) fn emit_darwin_network_install_pointer_handler(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    native_owner: Arm64Register,
    event_writer: Arm64Register,
    callback: Arm64FunctionId,
    descriptor: Arm64DarwinBlockDescriptorId,
    block_offset: u32,
    setter: DarwinNetworkAdapterFunction,
) -> Result<(), Arm64DarwinBlockError> {
    materialize_darwin_pointer_capture_stack_block(
        code,
        block_offset,
        imports.data(DarwinNetworkAdapterData::StackBlockClass),
        callback,
        descriptor,
        event_writer,
        x(8),
    )?;
    move_register(code, x(0), native_owner);
    load_darwin_stack_block_address(code, block_offset, x(1))?;
    call(code, imports.function(setter));
    Ok(())
}

/// Associates one native provider owner with its serial callback queue.
pub(crate) fn emit_darwin_network_set_owner_queue(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    native_owner: Arm64Register,
    queue: Arm64Register,
    setter: DarwinNetworkAdapterFunction,
) {
    move_register(code, x(0), native_owner);
    move_register(code, x(1), queue);
    call(code, imports.function(setter));
}

pub(crate) fn emit_darwin_network_release_network_object(
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

pub(crate) fn emit_darwin_network_release_dispatch_object(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    object: Arm64Register,
) {
    move_register(code, x(0), object);
    call(
        code,
        imports.function(DarwinNetworkAdapterFunction::DispatchRelease),
    );
}

pub(crate) fn emit_darwin_network_close_descriptor(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    descriptor: Arm64Register,
) {
    move_register(code, x(0), descriptor);
    call(code, imports.function(DarwinNetworkAdapterFunction::Close));
}

pub(crate) fn emit_darwin_network_load_imported_object(
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

fn immediate(code: &mut Arm64CodeBuilder, destination: Arm64Register, value: u16) {
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: crate::Arm64MoveWide::Zero,
        destination,
        immediate: value,
        shift: 0,
    });
}

fn stack_address(code: &mut Arm64CodeBuilder, offset: u16, destination: Arm64Register) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::StackPointer,
        immediate: offset,
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

fn call(code: &mut Arm64CodeBuilder, target: crate::Arm64FunctionImportId) {
    code.load_function_import(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
}

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).expect("closed ARM64 register is valid")
}
