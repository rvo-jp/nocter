use nocter_arm64::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64CodeBuilder,
    Arm64DarwinBlockDescriptorId, Arm64DataImportId, Arm64DataRegister, Arm64DataSize,
    Arm64FunctionId, Arm64FunctionImportId, Arm64Instruction, Arm64LoadStoreSize, Arm64MoveWide,
    Arm64Program, Arm64ProgramBuilder, Arm64Register, add_darwin_pointer_capture_block_descriptor,
    load_darwin_stack_block_address, materialize_darwin_pointer_capture_stack_block,
};
use nocter_runtime_contract::{RuntimeDataImport, RuntimeFunctionImport, RuntimeLibraryIdentity};

use crate::MachOImage;

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).unwrap()
}

fn exit_program(status: u16) -> Arm64Program {
    let mut program = Arm64ProgramBuilder::new();
    let entry = program.declare_function();
    let text = program
        .add_data(b"unused static text".as_slice(), 1)
        .unwrap();
    let mut code = Arm64CodeBuilder::new();
    code.load_data_address(text, x(3));
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination: x(0),
        immediate: status,
        shift: 0,
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
    program.set_entry(entry).unwrap();
    program.finish().unwrap()
}

fn imported_call_program() -> Arm64Program {
    let mut program = Arm64ProgramBuilder::new();
    let entry = program.declare_function();
    let function = program
        .add_function_import(
            RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinSystem, "_getpid").unwrap(),
        )
        .unwrap();
    let duplicate = program
        .add_function_import(
            RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinSystem, "_getpid").unwrap(),
        )
        .unwrap();
    assert_eq!(function, duplicate);

    let mut code = Arm64CodeBuilder::new();
    code.load_function_import(function, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination: x(0),
        immediate: 0,
        shift: 0,
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
    program.set_entry(entry).unwrap();
    program.finish().unwrap()
}

fn imported_exit_program(symbol: &str) -> Arm64Program {
    imported_exit_program_from(RuntimeLibraryIdentity::DarwinSystem, symbol)
}

fn imported_exit_program_from(library: RuntimeLibraryIdentity, symbol: &str) -> Arm64Program {
    let mut program = Arm64ProgramBuilder::new();
    let entry = program.declare_function();
    let _ = program
        .add_function_import(RuntimeFunctionImport::new(library, symbol).unwrap())
        .unwrap();
    let mut code = Arm64CodeBuilder::new();
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination: x(0),
        immediate: 0,
        shift: 0,
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
    program.set_entry(entry).unwrap();
    program.finish().unwrap()
}

fn imported_security_program() -> Arm64Program {
    let mut program = Arm64ProgramBuilder::new();
    let entry = program.declare_function();
    let create_context = program
        .add_function_import(
            RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinSecurity, "_SSLCreateContext")
                .unwrap(),
        )
        .unwrap();
    let release = program
        .add_function_import(
            RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinCoreFoundation, "_CFRelease")
                .unwrap(),
        )
        .unwrap();

    let mut code = Arm64CodeBuilder::new();
    for destination in [x(0), x(2)] {
        code.append(Arm64Instruction::MoveWide {
            size: Arm64DataSize::Bits64,
            operation: Arm64MoveWide::Zero,
            destination,
            immediate: 0,
            shift: 0,
        });
    }
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination: x(1),
        immediate: 1,
        shift: 0,
    });
    for function in [create_context, release] {
        code.load_function_import(function, x(16));
        code.append(Arm64Instruction::BranchRegister {
            target: x(16),
            link: true,
        });
    }
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination: x(0),
        immediate: 0,
        shift: 0,
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
    program.set_entry(entry).unwrap();
    program.finish().unwrap()
}

fn imported_network_program() -> Arm64Program {
    let mut program = Arm64ProgramBuilder::new();
    let entry = program.declare_function();
    let default_configuration = program
        .add_data_import(
            RuntimeDataImport::new(
                RuntimeLibraryIdentity::DarwinNetwork,
                "__nw_parameters_configure_protocol_default_configuration",
            )
            .unwrap(),
        )
        .unwrap();
    let create_parameters = program
        .add_function_import(
            RuntimeFunctionImport::new(
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_parameters_create_secure_tcp",
            )
            .unwrap(),
        )
        .unwrap();
    let release = program
        .add_function_import(
            RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinNetwork, "_nw_release")
                .unwrap(),
        )
        .unwrap();

    let mut code = Arm64CodeBuilder::new();
    code.load_data_import(default_configuration, x(0));
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(x(0)),
        base: Arm64BaseRegister::General(x(0)),
        offset: 0,
    });
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(x(1)),
        source: Arm64BaseRegister::General(x(0)),
        immediate: 0,
        shift_12: false,
    });
    for function in [create_parameters, release] {
        code.load_function_import(function, x(16));
        code.append(Arm64Instruction::BranchRegister {
            target: x(16),
            link: true,
        });
    }
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination: x(0),
        immediate: 0,
        shift: 0,
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
    program.set_entry(entry).unwrap();
    program.finish().unwrap()
}

fn imported_network_block_program() -> Arm64Program {
    let mut program = Arm64ProgramBuilder::new();
    let entry = program.declare_function();
    let invoke = program.declare_function();
    let resources = declare_network_block_resources(&mut program);
    define_mailbox_block_invoke(&mut program, invoke);
    define_network_block_entry(&mut program, entry, invoke, &resources);
    program.set_entry(entry).unwrap();
    program.finish().unwrap()
}

struct NetworkBlockResources {
    descriptor: Arm64DarwinBlockDescriptorId,
    stack_block_class: Arm64DataImportId,
    default_configuration: Arm64DataImportId,
    create_parameters: Arm64FunctionImportId,
    release: Arm64FunctionImportId,
}

fn declare_network_block_resources(program: &mut Arm64ProgramBuilder) -> NetworkBlockResources {
    let descriptor = add_darwin_pointer_capture_block_descriptor(
        program,
        b"v16@?0^{nw_protocol_options=}8\0".as_slice(),
    )
    .unwrap();
    let stack_block_class = program
        .add_data_import(
            RuntimeDataImport::new(
                RuntimeLibraryIdentity::DarwinSystem,
                "__NSConcreteStackBlock",
            )
            .unwrap(),
        )
        .unwrap();
    let default_configuration = program
        .add_data_import(
            RuntimeDataImport::new(
                RuntimeLibraryIdentity::DarwinNetwork,
                "__nw_parameters_configure_protocol_default_configuration",
            )
            .unwrap(),
        )
        .unwrap();
    let create_parameters = program
        .add_function_import(
            RuntimeFunctionImport::new(
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_parameters_create_secure_tcp",
            )
            .unwrap(),
        )
        .unwrap();
    let release = program
        .add_function_import(
            RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinNetwork, "_nw_release")
                .unwrap(),
        )
        .unwrap();
    NetworkBlockResources {
        descriptor,
        stack_block_class,
        default_configuration,
        create_parameters,
        release,
    }
}

fn define_mailbox_block_invoke(program: &mut Arm64ProgramBuilder, invoke: Arm64FunctionId) {
    let mut invoke_code = Arm64CodeBuilder::new();
    invoke_code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(x(8)),
        base: Arm64BaseRegister::General(x(0)),
        offset: 32,
    });
    invoke_code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination: x(9),
        immediate: 1,
        shift: 0,
    });
    invoke_code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::General(x(9)),
        base: Arm64BaseRegister::General(x(8)),
        offset: 0,
    });
    invoke_code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
    program
        .define_function(invoke, invoke_code.finish().unwrap())
        .unwrap();
}

fn define_network_block_entry(
    program: &mut Arm64ProgramBuilder,
    entry: Arm64FunctionId,
    invoke: Arm64FunctionId,
    resources: &NetworkBlockResources,
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

#[test]
fn image_is_deterministic_and_owns_all_required_load_commands() {
    let program = exit_program(42);
    let first = MachOImage::build(&program).unwrap();
    let second = MachOImage::build(&program).unwrap();
    assert_eq!(first, second);
    let bytes = first.bytes();

    assert_eq!(
        u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
        0xfeed_facf
    );
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        0x0100_000c
    );
    assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 2);
    assert_eq!(u32::from_le_bytes(bytes[16..20].try_into().unwrap()), 11);
    assert!(
        bytes
            .windows(b"/usr/lib/dyld\0".len())
            .any(|part| part == b"/usr/lib/dyld\0")
    );
    assert!(
        bytes
            .windows(b"/usr/lib/libSystem.B.dylib\0".len())
            .any(|part| part == b"/usr/lib/libSystem.B.dylib\0")
    );
    assert!(
        bytes
            .windows(4)
            .any(|part| part == 0xfade_0cc0_u32.to_be_bytes())
    );
}

#[test]
fn imported_symbol_identity_contributes_to_the_complete_image() {
    let getpid = MachOImage::build(&imported_exit_program("_getpid")).unwrap();
    let getuid = MachOImage::build(&imported_exit_program("_getuid")).unwrap();
    assert_ne!(getpid, getuid);
}

#[test]
fn imported_library_identity_contributes_to_the_complete_image() {
    let system = MachOImage::build(&imported_exit_program_from(
        RuntimeLibraryIdentity::DarwinSystem,
        "_same_symbol",
    ))
    .unwrap();
    let security = MachOImage::build(&imported_exit_program_from(
        RuntimeLibraryIdentity::DarwinSecurity,
        "_same_symbol",
    ))
    .unwrap();
    assert_ne!(system, security);
    let complete_provider = MachOImage::build(&imported_security_program()).unwrap();
    assert_eq!(
        u32::from_le_bytes(complete_provider.bytes()[16..20].try_into().unwrap()),
        13
    );
    for path in [
        b"/System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation\0"
            .as_slice(),
        b"/System/Library/Frameworks/Security.framework/Versions/A/Security\0".as_slice(),
    ] {
        assert!(
            complete_provider
                .bytes()
                .windows(path.len())
                .any(|candidate| candidate == path)
        );
    }
}

#[test]
fn function_and_data_import_kinds_remain_distinct() {
    let program = imported_network_program();
    assert_eq!(program.runtime_imports().len(), 3);
    assert!(matches!(
        program.runtime_imports()[0].import(),
        nocter_runtime_contract::RuntimeImport::Data(_)
    ));
    assert!(program.runtime_imports()[1..].iter().all(|import| matches!(
        import.import(),
        nocter_runtime_contract::RuntimeImport::Function(_)
    )));
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn generated_image_executes_without_external_linking_or_signing() {
    use std::os::unix::fs::PermissionsExt;

    let image = MachOImage::build(&exit_program(42)).unwrap();
    let path = std::env::temp_dir().join(format!("nocter-macho-test-{}", std::process::id()));
    std::fs::write(&path, image.bytes()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let status = std::process::Command::new(&path).status().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(status.code(), Some(42));
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn generated_image_binds_and_calls_one_system_function() {
    use std::os::unix::fs::PermissionsExt;

    let image = MachOImage::build(&imported_call_program()).unwrap();
    let path =
        std::env::temp_dir().join(format!("nocter-macho-import-test-{}", std::process::id()));
    std::fs::write(&path, image.bytes()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let status = std::process::Command::new(&path).status().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(status.code(), Some(0));
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn generated_image_binds_and_calls_security_and_core_foundation() {
    use std::os::unix::fs::PermissionsExt;

    let image = MachOImage::build(&imported_security_program()).unwrap();
    let path =
        std::env::temp_dir().join(format!("nocter-macho-security-test-{}", std::process::id()));
    std::fs::write(&path, image.bytes()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let status = std::process::Command::new(&path).status().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(status.code(), Some(0));
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn generated_image_binds_and_reads_network_framework_data() {
    use std::os::unix::fs::PermissionsExt;

    let image = MachOImage::build(&imported_network_program()).unwrap();
    let path =
        std::env::temp_dir().join(format!("nocter-macho-network-test-{}", std::process::id()));
    std::fs::write(&path, image.bytes()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let status = std::process::Command::new(&path).status().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(status.code(), Some(0));
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn generated_image_passes_a_captured_block_to_network_framework() {
    use std::os::unix::fs::PermissionsExt;

    let image = MachOImage::build(&imported_network_block_program()).unwrap();
    let path = std::env::temp_dir().join(format!(
        "nocter-macho-network-block-test-{}",
        std::process::id()
    ));
    std::fs::write(&path, image.bytes()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let status = std::process::Command::new(&path).status().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(status.code(), Some(1));
}
