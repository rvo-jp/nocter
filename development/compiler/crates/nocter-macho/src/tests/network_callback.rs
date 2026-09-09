use nocter_arm64::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64CodeBuilder,
    Arm64DataImportId, Arm64DataRegister, Arm64DataSize, Arm64FunctionId, Arm64FunctionImportId,
    Arm64Instruction, Arm64LoadStoreSize, Arm64MoveWide, Arm64Program, Arm64ProgramBuilder,
    Arm64Register, add_darwin_pointer_capture_block_descriptor, load_darwin_stack_block_address,
    materialize_darwin_pointer_capture_stack_block,
};
use nocter_runtime_contract::{
    DarwinNetworkCallbackEventAbiSchema, DarwinNetworkEventKind, RuntimeDataImport,
    RuntimeFunctionImport, RuntimeLibraryIdentity,
};

use crate::MachOImage;

const BLOCK_OFFSET: u32 = 16;
const EVENT_OFFSET: u32 = 64;
const FRAME_SIZE: u16 = 112;

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).unwrap()
}

struct CallbackChannelImports {
    stack_block_class: Arm64DataImportId,
    socket_pair: Arm64FunctionImportId,
    send: Arm64FunctionImportId,
    receive: Arm64FunctionImportId,
    close: Arm64FunctionImportId,
    queue_create: Arm64FunctionImportId,
    dispatch_async: Arm64FunctionImportId,
    dispatch_release: Arm64FunctionImportId,
}

fn function_import(program: &mut Arm64ProgramBuilder, symbol: &str) -> Arm64FunctionImportId {
    program
        .add_function_import(
            RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinSystem, symbol).unwrap(),
        )
        .unwrap()
}

fn declare_imports(program: &mut Arm64ProgramBuilder) -> CallbackChannelImports {
    CallbackChannelImports {
        stack_block_class: program
            .add_data_import(
                RuntimeDataImport::new(
                    RuntimeLibraryIdentity::DarwinSystem,
                    "__NSConcreteStackBlock",
                )
                .unwrap(),
            )
            .unwrap(),
        socket_pair: function_import(program, "_socketpair"),
        send: function_import(program, "_send"),
        receive: function_import(program, "_recv"),
        close: function_import(program, "_close"),
        queue_create: function_import(program, "_dispatch_queue_create"),
        dispatch_async: function_import(program, "_dispatch_async"),
        dispatch_release: function_import(program, "_dispatch_release"),
    }
}

fn callback_channel_program() -> Arm64Program {
    let mut program = Arm64ProgramBuilder::new();
    let entry = program.declare_function();
    let invoke = program.declare_function();
    let descriptor = add_darwin_pointer_capture_block_descriptor(&mut program, b"v8@?0\0").unwrap();
    let queue_label = program
        .add_data(b"nocter.callback\0".as_slice(), 1)
        .unwrap();
    let imports = declare_imports(&mut program);

    define_callback(&mut program, invoke, imports.send);
    define_entry(
        &mut program,
        entry,
        invoke,
        descriptor,
        queue_label,
        &imports,
    );
    program.set_entry(entry).unwrap();
    program.finish().unwrap()
}

fn define_callback(
    program: &mut Arm64ProgramBuilder,
    invoke: Arm64FunctionId,
    send: Arm64FunctionImportId,
) {
    let schema = DarwinNetworkCallbackEventAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, 64);
    store(&mut code, x(30), 48);
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(x(0)),
        base: Arm64BaseRegister::General(x(0)),
        offset: 32,
    });
    for offset in [
        schema.kind_offset(),
        schema.payload_offset(1).unwrap(),
        schema.payload_offset(2).unwrap(),
        schema.payload_offset(3).unwrap(),
    ] {
        store_zero(&mut code, u32::try_from(offset).unwrap());
    }
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination: x(8),
        immediate: 42,
        shift: 0,
    });
    store(
        &mut code,
        x(8),
        u32::try_from(schema.payload_offset(0).unwrap()).unwrap(),
    );
    stack_address(&mut code, 0, x(1));
    immediate(&mut code, x(2), schema.size());
    immediate(&mut code, x(3), 0);
    call(&mut code, send);
    load(&mut code, x(30), 48);
    adjust_stack(&mut code, Arm64AddSubtract::Add, 64);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
    program
        .define_function(invoke, code.finish().unwrap())
        .unwrap();
}

fn define_entry(
    program: &mut Arm64ProgramBuilder,
    entry: Arm64FunctionId,
    invoke: Arm64FunctionId,
    descriptor: nocter_arm64::Arm64DarwinBlockDescriptorId,
    queue_label: nocter_arm64::Arm64DataId,
    imports: &CallbackChannelImports,
) {
    let schema = DarwinNetworkCallbackEventAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, FRAME_SIZE);

    immediate(&mut code, x(0), 1);
    immediate(&mut code, x(1), 2);
    immediate(&mut code, x(2), 0);
    stack_address(&mut code, 0, x(3));
    call(&mut code, imports.socket_pair);
    load_word(&mut code, x(20), 0);
    load_word(&mut code, x(21), 4);

    code.load_data_address(queue_label, x(0));
    immediate(&mut code, x(1), 0);
    call(&mut code, imports.queue_create);
    move_register(&mut code, x(22), x(0));

    materialize_darwin_pointer_capture_stack_block(
        &mut code,
        BLOCK_OFFSET,
        imports.stack_block_class,
        invoke,
        descriptor,
        x(21),
        x(8),
    )
    .unwrap();
    move_register(&mut code, x(0), x(22));
    load_darwin_stack_block_address(&mut code, BLOCK_OFFSET, x(1)).unwrap();
    call(&mut code, imports.dispatch_async);

    move_register(&mut code, x(0), x(20));
    stack_address(&mut code, EVENT_OFFSET, x(1));
    immediate(&mut code, x(2), schema.size());
    immediate(&mut code, x(3), 0);
    call(&mut code, imports.receive);
    load(
        &mut code,
        x(23),
        EVENT_OFFSET + u32::try_from(schema.payload_offset(0).unwrap()).unwrap(),
    );

    move_register(&mut code, x(0), x(22));
    call(&mut code, imports.dispatch_release);
    move_register(&mut code, x(0), x(21));
    call(&mut code, imports.close);
    move_register(&mut code, x(0), x(20));
    call(&mut code, imports.close);
    move_register(&mut code, x(0), x(23));
    immediate(&mut code, x(16), 1);
    code.append(Arm64Instruction::SupervisorCall { immediate: 0x80 });

    program
        .define_function(entry, code.finish().unwrap())
        .unwrap();
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

fn stack_address(code: &mut Arm64CodeBuilder, offset: u32, destination: Arm64Register) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::StackPointer,
        immediate: u16::try_from(offset).unwrap(),
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

fn immediate(code: &mut Arm64CodeBuilder, destination: Arm64Register, value: u64) {
    let immediate = u16::try_from(value).expect("test immediate fits one move-wide halfword");
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination,
        immediate,
        shift: 0,
    });
}

fn load(code: &mut Arm64CodeBuilder, destination: Arm64Register, offset: u32) {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::StackPointer,
        offset,
    });
}

fn load_word(code: &mut Arm64CodeBuilder, destination: Arm64Register, offset: u32) {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Word,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::StackPointer,
        offset,
    });
}

fn store(code: &mut Arm64CodeBuilder, source: Arm64Register, offset: u32) {
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::StackPointer,
        offset,
    });
}

fn store_zero(code: &mut Arm64CodeBuilder, offset: u32) {
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::Zero,
        base: Arm64BaseRegister::StackPointer,
        offset,
    });
}

fn call(code: &mut Arm64CodeBuilder, target: Arm64FunctionImportId) {
    code.load_function_import(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn generated_image_delivers_a_cross_thread_callback_as_one_reactor_datagram() {
    use std::os::unix::fs::PermissionsExt;

    assert_eq!(DarwinNetworkEventKind::ConnectionState.code(), 0);
    let image = MachOImage::build(&callback_channel_program()).unwrap();
    let path = std::env::temp_dir().join(format!(
        "nocter-macho-callback-channel-test-{}",
        std::process::id()
    ));
    std::fs::write(&path, image.bytes()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let status = std::process::Command::new(&path).status().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(status.code(), Some(42));
}
