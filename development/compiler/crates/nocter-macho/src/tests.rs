use nocter_arm64::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64CodeBuilder,
    Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize, Arm64MoveWide,
    Arm64Program, Arm64ProgramBuilder, Arm64Register,
};
use nocter_runtime_contract::{
    DarwinNetworkAdapterData, DarwinNetworkAdapterFunction, RuntimeFunctionImport,
    RuntimeLibraryIdentity,
};

use crate::MachOImage;

mod network_callback;
mod network_connection;
mod tls_configuration;

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
        .add_data_import(DarwinNetworkAdapterData::DefaultProtocolConfiguration.import())
        .unwrap();
    let create_parameters = program
        .add_function_import(DarwinNetworkAdapterFunction::ParametersCreateSecureTcp.import())
        .unwrap();
    let release = program
        .add_function_import(DarwinNetworkAdapterFunction::NetworkRelease.import())
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
