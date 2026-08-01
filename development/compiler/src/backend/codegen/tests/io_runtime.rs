use super::*;

#[test]
fn open_read_uses_open_syscall() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Fallible(Box::new(Type::I32)),
        instructions: vec![
            Instruction::OpenRead {
                destination: I32Location::Return,
                path: UsizeValue::Const(4096),
                failure_mode: FallibleFailureMode::Trap,
            },
            Instruction::ReturnFallibleSuccess,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_bytes(
        &code.text,
        &encoded_mov_u32_to_w(WReg::W16, DARWIN_OPEN_SYSCALL)
    ));
    assert!(contains_instruction(&code.text, [0x01, 0x10, 0x00, 0xd4])); // svc #0x80
}

#[test]
fn generated_str_write_runs() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::WriteStr {
                fd: I32Value::Const(1),
                text: StrValue::StaticBytes(b"hello\n".to_vec()),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);
    let code = generate_arm64_darwin_entry(&module).unwrap();
    let image = crate::target::macho::write_arm64_macos_executable_with_data(
        &code.text,
        &code.read_only_data,
    );
    let executable = write_temp_executable("codegen-str-write-runs", &image.bytes);

    let output = std::process::Command::new(&executable).output().unwrap();
    let _ = std::fs::remove_file(executable);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"hello\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn generated_slice_write_runs() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::WriteSlice {
                fd: I32Value::Const(1),
                bytes: SliceValue::StrBytes(StrValue::StaticBytes(b"bytes\n".to_vec())),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);
    let code = generate_arm64_darwin_entry(&module).unwrap();
    let image = crate::target::macho::write_arm64_macos_executable_with_data(
        &code.text,
        &code.read_only_data,
    );
    let executable = write_temp_executable("codegen-slice-write-runs", &image.bytes);

    let output = std::process::Command::new(&executable).output().unwrap();
    let _ = std::fs::remove_file(executable);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"bytes\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn write_str_emits_loop_for_partial_writes() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::WriteStr {
                fd: I32Value::Const(1),
                text: StrValue::StaticBytes(b"hello\n".to_vec()),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);
    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(
        contains_backward_unconditional_branch(&code.text),
        "write lowering should loop until the requested byte count is exhausted"
    );
}

#[test]
fn generated_close_fd_closes_stdout() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::CloseFd {
                fd: I32Value::Const(1),
            },
            Instruction::WriteStr {
                fd: I32Value::Const(1),
                text: StrValue::StaticBytes(b"hidden\n".to_vec()),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);
    let code = generate_arm64_darwin_entry(&module).unwrap();
    let image = crate::target::macho::write_arm64_macos_executable_with_data(
        &code.text,
        &code.read_only_data,
    );
    let executable = write_temp_executable("codegen-close-fd-runs", &image.bytes);

    let output = std::process::Command::new(&executable).output().unwrap();
    let _ = std::fs::remove_file(executable);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn generated_read_zero_bytes_runs() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::ReadSlice {
                destination: UsizeLocation::Local(0),
                fd: I32Value::Const(0),
                buffer: SliceValue::StrBytes(StrValue::StaticBytes(Vec::new())),
                failure_mode: FallibleFailureMode::Trap,
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);
    let code = generate_arm64_darwin_entry(&module).unwrap();
    let image = crate::target::macho::write_arm64_macos_executable_with_data(
        &code.text,
        &code.read_only_data,
    );
    let executable = write_temp_executable("codegen-read-zero-runs", &image.bytes);

    let output = std::process::Command::new(&executable).output().unwrap();
    let _ = std::fs::remove_file(executable);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}
