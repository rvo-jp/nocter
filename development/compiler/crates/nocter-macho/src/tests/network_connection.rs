use nocter_arm64::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DarwinNetworkAdapterImports, Arm64DataRegister, Arm64DataSize,
    Arm64FunctionId, Arm64Instruction, Arm64LoadStoreSize, Arm64Program, Arm64ProgramBuilder,
    add_darwin_pointer_capture_block_descriptor, emit_darwin_network_event_receive,
    emit_darwin_network_event_send, load_darwin_stack_block_address,
    materialize_darwin_pointer_capture_stack_block,
};
use nocter_runtime_contract::{
    DarwinNetworkAdapterData, DarwinNetworkAdapterFunction, DarwinNetworkCallbackEventAbiSchema,
    DarwinNetworkCallbackRole, DarwinNetworkConnectionState, DarwinNetworkEventKind,
};

use super::network_callback::{
    adjust_stack, call, immediate, load, load_word, move_register, stack_address, store,
    store_zero, x,
};
use crate::MachOImage;

fn connection_lifecycle_program() -> Arm64Program {
    let mut program = Arm64ProgramBuilder::new();
    let entry = program.declare_function();
    let invoke = program.declare_function();
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
    define_state_callback(&mut program, invoke, &imports);
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

fn define_state_callback(
    program: &mut Arm64ProgramBuilder,
    invoke: Arm64FunctionId,
    imports: &Arm64DarwinNetworkAdapterImports,
) {
    let schema = DarwinNetworkCallbackEventAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, 96);
    for (register, offset) in [(x(19), 64), (x(20), 72), (x(21), 80), (x(30), 88)] {
        store(&mut code, register, offset);
    }
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(x(19)),
        base: Arm64BaseRegister::General(x(0)),
        offset: 32,
    });
    code.append(Arm64Instruction::BitfieldExtend {
        size: Arm64DataSize::Bits64,
        signed: false,
        source_bits: 32,
        destination: x(20),
        source: x(1),
    });
    move_register(&mut code, x(21), x(2));

    let retained = code.create_label();
    compare_zero(&mut code, x(21));
    code.branch_conditional(retained, Arm64BranchCondition::Equal);
    move_register(&mut code, x(0), x(21));
    call(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::NetworkRetain),
    );
    code.bind(retained).unwrap();

    immediate(
        &mut code,
        x(8),
        DarwinNetworkEventKind::ConnectionState.code(),
    );
    store(
        &mut code,
        x(8),
        u32::try_from(schema.kind_offset()).unwrap(),
    );
    store(
        &mut code,
        x(20),
        u32::try_from(schema.payload_offset(0).unwrap()).unwrap(),
    );
    store(
        &mut code,
        x(21),
        u32::try_from(schema.payload_offset(1).unwrap()).unwrap(),
    );
    store_zero(
        &mut code,
        u32::try_from(schema.payload_offset(2).unwrap()).unwrap(),
    );
    store_zero(
        &mut code,
        u32::try_from(schema.payload_offset(3).unwrap()).unwrap(),
    );
    emit_darwin_network_event_send(&mut code, imports.channel(), x(19), 0).unwrap();

    for (register, offset) in [(x(19), 64), (x(20), 72), (x(21), 80), (x(30), 88)] {
        load(&mut code, register, offset);
    }
    adjust_stack(&mut code, Arm64AddSubtract::Add, 96);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
    program
        .define_function(invoke, code.finish().unwrap())
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
    const EVENT_OFFSET: u32 = 64;
    let schema = DarwinNetworkCallbackEventAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, 112);
    create_callback_channel(&mut code, imports);
    create_connection(&mut code, data, imports);
    install_handler(&mut code, invoke, descriptor, imports);

    move_register(&mut code, x(0), x(25));
    call(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::ConnectionStart),
    );
    move_register(&mut code, x(0), x(25));
    call(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::ConnectionCancel),
    );

    let receive = code.create_label();
    let error_released = code.create_label();
    code.bind(receive).unwrap();
    emit_darwin_network_event_receive(&mut code, imports.channel(), x(20), EVENT_OFFSET).unwrap();
    load(
        &mut code,
        x(26),
        EVENT_OFFSET + u32::try_from(schema.payload_offset(0).unwrap()).unwrap(),
    );
    load(
        &mut code,
        x(27),
        EVENT_OFFSET + u32::try_from(schema.payload_offset(1).unwrap()).unwrap(),
    );
    compare_zero(&mut code, x(27));
    code.branch_conditional(error_released, Arm64BranchCondition::Equal);
    move_register(&mut code, x(0), x(27));
    call(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::NetworkRelease),
    );
    code.bind(error_released).unwrap();
    compare_immediate(
        &mut code,
        x(26),
        DarwinNetworkConnectionState::Cancelled.code(),
    );
    code.branch_conditional(receive, Arm64BranchCondition::NotEqual);
    move_register(&mut code, x(0), x(22));
    immediate(&mut code, x(1), 0);
    code.load_function_address(barrier, x(2));
    call(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::DispatchSync),
    );
    release_connection_resources(&mut code, imports);
    immediate(&mut code, x(0), 0);
    immediate(&mut code, x(16), 1);
    code.append(Arm64Instruction::SupervisorCall { immediate: 0x80 });
    program
        .define_function(entry, code.finish().unwrap())
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
    const BLOCK_OFFSET: u32 = 16;
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

fn release_connection_resources(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
) {
    for register in [x(25), x(24), x(23)] {
        move_register(code, x(0), register);
        call(
            code,
            imports.function(DarwinNetworkAdapterFunction::NetworkRelease),
        );
    }
    move_register(code, x(0), x(22));
    call(
        code,
        imports.function(DarwinNetworkAdapterFunction::DispatchRelease),
    );
    for register in [x(21), x(20)] {
        move_register(code, x(0), register);
        call(code, imports.function(DarwinNetworkAdapterFunction::Close));
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
