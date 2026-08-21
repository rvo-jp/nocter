use nocter_arm64::{
    Arm64CodeBuilder, Arm64DataSize, Arm64Instruction, Arm64MoveWide, Arm64Program,
    Arm64ProgramBuilder, Arm64Register,
};

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
    assert_eq!(u32::from_le_bytes(bytes[16..20].try_into().unwrap()), 9);
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
