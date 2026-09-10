use nocter_arm64::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64CodeBuilder,
    Arm64DarwinBlockDescriptorId, Arm64DataImportId, Arm64DataRegister, Arm64DataSize,
    Arm64FunctionId, Arm64FunctionImportId, Arm64Instruction, Arm64LoadStoreSize, Arm64MoveWide,
    Arm64Program, Arm64ProgramBuilder, Arm64Register, add_darwin_pointer_capture_block_descriptor,
    load_darwin_stack_block_address, materialize_darwin_pointer_capture_stack_block,
};
use nocter_runtime_contract::{
    DarwinNetworkAdapterData, DarwinNetworkAdapterFunction, DarwinTlsCallbackRole,
};

use crate::MachOImage;

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).unwrap()
}

struct TlsConfigurationBlockResources {
    descriptor: Arm64DarwinBlockDescriptorId,
    stack_block_class: Arm64DataImportId,
    default_configuration: Arm64DataImportId,
    create_parameters: Arm64FunctionImportId,
    release: Arm64FunctionImportId,
}

fn declare_resources(program: &mut Arm64ProgramBuilder) -> TlsConfigurationBlockResources {
    let descriptor = add_darwin_pointer_capture_block_descriptor(
        program,
        DarwinTlsCallbackRole::ConfigureProtocol.block_signature(),
    )
    .unwrap();
    let stack_block_class = program
        .add_data_import(DarwinNetworkAdapterData::StackBlockClass.import())
        .unwrap();
    let default_configuration = program
        .add_data_import(DarwinNetworkAdapterData::DefaultProtocolConfiguration.import())
        .unwrap();
    let create_parameters = program
        .add_function_import(DarwinNetworkAdapterFunction::ParametersCreateSecureTcp.import())
        .unwrap();
    let release = program
        .add_function_import(DarwinNetworkAdapterFunction::NetworkRelease.import())
        .unwrap();
    TlsConfigurationBlockResources {
        descriptor,
        stack_block_class,
        default_configuration,
        create_parameters,
        release,
    }
}

fn define_configuration_callback(program: &mut Arm64ProgramBuilder, invoke: Arm64FunctionId) {
    let mut code = Arm64CodeBuilder::new();
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(x(8)),
        base: Arm64BaseRegister::General(x(0)),
        offset: 32,
    });
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination: x(9),
        immediate: 1,
        shift: 0,
    });
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::General(x(9)),
        base: Arm64BaseRegister::General(x(8)),
        offset: 0,
    });
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
    resources: &TlsConfigurationBlockResources,
) {
    const MAILBOX_OFFSET: u32 = 40;
    const FRAME_SIZE: u16 = 64;

    let mut code = Arm64CodeBuilder::new();
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64AddSubtractDestination::StackPointer,
        source: Arm64BaseRegister::StackPointer,
        immediate: FRAME_SIZE,
        shift_12: false,
    });
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(x(8)),
        source: Arm64BaseRegister::StackPointer,
        immediate: u16::try_from(MAILBOX_OFFSET).unwrap(),
        shift_12: false,
    });
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::Zero,
        base: Arm64BaseRegister::StackPointer,
        offset: MAILBOX_OFFSET,
    });
    materialize_darwin_pointer_capture_stack_block(
        &mut code,
        0,
        resources.stack_block_class,
        invoke,
        resources.descriptor,
        x(8),
        x(9),
    )
    .unwrap();
    load_darwin_stack_block_address(&mut code, 0, x(0)).unwrap();
    code.load_data_import(resources.default_configuration, x(1));
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(x(1)),
        base: Arm64BaseRegister::General(x(1)),
        offset: 0,
    });
    code.load_function_import(resources.create_parameters, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(x(19)),
        source: Arm64BaseRegister::General(x(0)),
        immediate: 0,
        shift_12: false,
    });
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(x(0)),
        source: Arm64BaseRegister::General(x(19)),
        immediate: 0,
        shift_12: false,
    });
    code.load_function_import(resources.release, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(x(0)),
        base: Arm64BaseRegister::StackPointer,
        offset: MAILBOX_OFFSET,
    });
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination: x(16),
        immediate: 1,
        shift: 0,
    });
    code.append(Arm64Instruction::SupervisorCall { immediate: 0x80 });
    program
        .define_function(entry, code.finish().unwrap())
        .unwrap();
}

fn tls_configuration_block_program() -> Arm64Program {
    let mut program = Arm64ProgramBuilder::new();
    let entry = program.declare_function();
    let invoke = program.declare_function();
    let resources = declare_resources(&mut program);
    define_configuration_callback(&mut program, invoke);
    define_entry(&mut program, entry, invoke, &resources);
    program.set_entry(entry).unwrap();
    program.finish().unwrap()
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn generated_image_passes_tls_configuration_block_to_network_framework() {
    use std::os::unix::fs::PermissionsExt;

    let image = MachOImage::build(&tls_configuration_block_program()).unwrap();
    let path = std::env::temp_dir().join(format!(
        "nocter-macho-tls-configuration-test-{}",
        std::process::id()
    ));
    std::fs::write(&path, image.bytes()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let status = std::process::Command::new(&path).status().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(status.code(), Some(1));
}
