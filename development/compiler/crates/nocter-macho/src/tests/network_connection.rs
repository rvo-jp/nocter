use nocter_arm64::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DarwinNetworkAdapterImports, Arm64DarwinNetworkOwnerResources,
    Arm64DataRegister, Arm64DataSize, Arm64FunctionId, Arm64Instruction, Arm64LoadStoreSize,
    Arm64Program, Arm64ProgramBuilder, add_darwin_network_state_callback,
    add_darwin_pointer_capture_block_descriptor, emit_darwin_network_event_receive,
    emit_darwin_network_owner_initialize, emit_darwin_network_owner_release,
    emit_darwin_network_owner_transition, load_darwin_stack_block_address,
    materialize_darwin_pointer_capture_stack_block,
};
use nocter_runtime_contract::{
    DarwinNetworkAdapterData, DarwinNetworkAdapterFunction, DarwinNetworkAdapterOperation,
    DarwinNetworkCallbackEventAbiSchema, DarwinNetworkCallbackRole, DarwinNetworkConnectionState,
    DarwinNetworkOwnerKind,
};

use super::network_callback::{
    adjust_stack, call, immediate, load, load_word, move_register, stack_address, x,
};
use crate::MachOImage;

fn connection_lifecycle_program() -> Arm64Program {
    let mut program = Arm64ProgramBuilder::new();
    let entry = program.declare_function();
    let barrier = program.declare_function();
    let descriptor = add_darwin_pointer_capture_block_descriptor(
        &mut program,
        DarwinNetworkCallbackRole::ConnectionState.block_signature(),
    )
    .unwrap();
    let host = program.add_data(b"127.0.0.1\0".as_slice(), 1).unwrap();
    let port = program.add_data(b"1\0".as_slice(), 1).unwrap();
    let queue_label = program
        .add_data(b"nocter.network.connection\0".as_slice(), 1)
        .unwrap();
    let imports = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
    let invoke = add_darwin_network_state_callback(
        &mut program,
        DarwinNetworkCallbackRole::ConnectionState,
        &imports,
    )
    .unwrap();
    define_dispatch_barrier(&mut program, barrier);
    define_connection_entry(
        &mut program,
        entry,
        invoke,
        barrier,
        descriptor,
        [host, port, queue_label],
        &imports,
    );
    program.set_entry(entry).unwrap();
    program.finish().unwrap()
}

fn define_dispatch_barrier(program: &mut Arm64ProgramBuilder, barrier: Arm64FunctionId) {
    let mut code = Arm64CodeBuilder::new();
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
    program
        .define_function(barrier, code.finish().unwrap())
        .unwrap();
}

fn define_connection_entry(
    program: &mut Arm64ProgramBuilder,
    entry: Arm64FunctionId,
    invoke: Arm64FunctionId,
    barrier: Arm64FunctionId,
    descriptor: nocter_arm64::Arm64DarwinBlockDescriptorId,
    data: [nocter_arm64::Arm64DataId; 3],
    imports: &Arm64DarwinNetworkAdapterImports,
) {
    const OWNER_OFFSET: u32 = 0;
    const EVENT_OFFSET: u32 = 96;
    let schema = DarwinNetworkCallbackEventAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, 160);
    create_callback_channel(&mut code, imports);
    create_connection(&mut code, data, imports);
    install_handler(&mut code, invoke, descriptor, imports);
    release_creation_temporaries(&mut code, imports);
    stack_address(&mut code, OWNER_OFFSET, x(26));
    emit_darwin_network_owner_initialize(
        &mut code,
        x(26),
        Arm64DarwinNetworkOwnerResources::new(x(25), x(22), x(20), x(21)),
    )
    .unwrap();
    start_then_cancel(&mut code, imports);

    let receive = code.create_label();
    let error_released = code.create_label();
    code.bind(receive).unwrap();
    emit_darwin_network_event_receive(&mut code, imports.channel(), x(20), EVENT_OFFSET).unwrap();
    load(
        &mut code,
        x(27),
        EVENT_OFFSET + u32::try_from(schema.payload_offset(0).unwrap()).unwrap(),
    );
    load(
        &mut code,
        x(28),
        EVENT_OFFSET + u32::try_from(schema.payload_offset(1).unwrap()).unwrap(),
    );
    compare_zero(&mut code, x(28));
    code.branch_conditional(error_released, Arm64BranchCondition::Equal);
    move_register(&mut code, x(0), x(28));
    call(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::NetworkRelease),
    );
    code.bind(error_released).unwrap();
    compare_immediate(
        &mut code,
        x(27),
        DarwinNetworkConnectionState::Cancelled.code(),
    );
    code.branch_conditional(receive, Arm64BranchCondition::NotEqual);
    complete_release_fence(&mut code, barrier, imports);
    immediate(&mut code, x(0), 0);
    immediate(&mut code, x(16), 1);
    code.append(Arm64Instruction::SupervisorCall { immediate: 0x80 });
    program
        .define_function(entry, code.finish().unwrap())
        .unwrap();
}

fn start_then_cancel(code: &mut Arm64CodeBuilder, imports: &Arm64DarwinNetworkAdapterImports) {
    emit_darwin_network_owner_transition(
        code,
        x(26),
        DarwinNetworkOwnerKind::Connection,
        DarwinNetworkAdapterOperation::Start,
        imports,
    )
    .unwrap();
    move_register(code, x(0), x(25));
    call(
        code,
        imports.function(DarwinNetworkAdapterFunction::ConnectionStart),
    );
    emit_darwin_network_owner_transition(
        code,
        x(26),
        DarwinNetworkOwnerKind::Connection,
        DarwinNetworkAdapterOperation::RequestCancel,
        imports,
    )
    .unwrap();
    move_register(code, x(0), x(25));
    call(
        code,
        imports.function(DarwinNetworkAdapterFunction::ConnectionCancel),
    );
}

fn complete_release_fence(
    code: &mut Arm64CodeBuilder,
    barrier: Arm64FunctionId,
    imports: &Arm64DarwinNetworkAdapterImports,
) {
    emit_darwin_network_owner_transition(
        code,
        x(26),
        DarwinNetworkOwnerKind::Connection,
        DarwinNetworkAdapterOperation::ObserveFinalState,
        imports,
    )
    .unwrap();
    move_register(code, x(0), x(22));
    immediate(code, x(1), 0);
    code.load_function_address(barrier, x(2));
    call(
        code,
        imports.function(DarwinNetworkAdapterFunction::DispatchSync),
    );
    emit_darwin_network_owner_transition(
        code,
        x(26),
        DarwinNetworkOwnerKind::Connection,
        DarwinNetworkAdapterOperation::CompleteReleaseBarrier,
        imports,
    )
    .unwrap();
    emit_darwin_network_owner_release(code, x(26), DarwinNetworkOwnerKind::Connection, imports)
        .unwrap();
}

fn create_callback_channel(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
) {
    immediate(code, x(0), 1);
    immediate(code, x(1), 2);
    immediate(code, x(2), 0);
    stack_address(code, 0, x(3));
    call(
        code,
        imports.function(DarwinNetworkAdapterFunction::SocketPair),
    );
    load_word(code, x(20), 0);
    load_word(code, x(21), 4);
}

fn create_connection(
    code: &mut Arm64CodeBuilder,
    [host, port, queue_label]: [nocter_arm64::Arm64DataId; 3],
    imports: &Arm64DarwinNetworkAdapterImports,
) {
    code.load_data_address(queue_label, x(0));
    immediate(code, x(1), 0);
    call(
        code,
        imports.function(DarwinNetworkAdapterFunction::DispatchQueueCreate),
    );
    move_register(code, x(22), x(0));
    code.load_data_address(host, x(0));
    code.load_data_address(port, x(1));
    call(
        code,
        imports.function(DarwinNetworkAdapterFunction::EndpointCreateHost),
    );
    move_register(code, x(23), x(0));
    code.load_data_import(
        imports.data(DarwinNetworkAdapterData::DefaultProtocolConfiguration),
        x(0),
    );
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(x(0)),
        base: Arm64BaseRegister::General(x(0)),
        offset: 0,
    });
    move_register(code, x(1), x(0));
    call(
        code,
        imports.function(DarwinNetworkAdapterFunction::ParametersCreateSecureTcp),
    );
    move_register(code, x(24), x(0));
    move_register(code, x(0), x(23));
    move_register(code, x(1), x(24));
    call(
        code,
        imports.function(DarwinNetworkAdapterFunction::ConnectionCreate),
    );
    move_register(code, x(25), x(0));
}

fn install_handler(
    code: &mut Arm64CodeBuilder,
    invoke: Arm64FunctionId,
    descriptor: nocter_arm64::Arm64DarwinBlockDescriptorId,
    imports: &Arm64DarwinNetworkAdapterImports,
) {
    const BLOCK_OFFSET: u32 = 48;
    materialize_darwin_pointer_capture_stack_block(
        code,
        BLOCK_OFFSET,
        imports.data(DarwinNetworkAdapterData::StackBlockClass),
        invoke,
        descriptor,
        x(21),
        x(8),
    )
    .unwrap();
    move_register(code, x(0), x(25));
    load_darwin_stack_block_address(code, BLOCK_OFFSET, x(1)).unwrap();
    call(
        code,
        imports.function(DarwinNetworkAdapterFunction::ConnectionSetStateHandler),
    );
    move_register(code, x(0), x(25));
    move_register(code, x(1), x(22));
    call(
        code,
        imports.function(DarwinNetworkAdapterFunction::ConnectionSetQueue),
    );
}

fn release_creation_temporaries(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
) {
    for register in [x(24), x(23)] {
        move_register(code, x(0), register);
        call(
            code,
            imports.function(DarwinNetworkAdapterFunction::NetworkRelease),
        );
    }
}

fn compare_zero(code: &mut Arm64CodeBuilder, value: nocter_arm64::Arm64Register) {
    compare_immediate(code, value, 0);
}

fn compare_immediate(
    code: &mut Arm64CodeBuilder,
    value: nocter_arm64::Arm64Register,
    immediate_value: u64,
) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: u16::try_from(immediate_value).unwrap(),
        shift_12: false,
    });
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn generated_image_releases_network_ownership_after_cancelled_and_queue_barrier() {
    use std::os::unix::fs::PermissionsExt;

    let image = MachOImage::build(&connection_lifecycle_program()).unwrap();
    let path = std::env::temp_dir().join(format!(
        "nocter-macho-network-lifecycle-test-{}",
        std::process::id()
    ));
    std::fs::write(&path, image.bytes()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let status = std::process::Command::new(&path).status().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(status.code(), Some(0));
}
