use crate::diagnostics::Diagnostic;
use crate::ir::{Function, Instruction, IrModule, Type};
use crate::target::arm64::{Encoder, MoveWideShift, WReg, XReg};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MachineCode {
    pub(crate) text: Vec<u8>,
    pub(crate) read_only_data: Vec<u8>,
}

pub(crate) fn generate_arm64_darwin_entry(
    module: &IrModule,
) -> Result<MachineCode, Vec<Diagnostic>> {
    let mut emitter = EntryEmitter::new();
    emitter.emit_module(module)?;
    emitter.finish()
}

#[derive(Debug, Default)]
struct EntryEmitter {
    encoder: Encoder,
    read_only_data: Vec<u8>,
    data_address_patches: Vec<DataAddressPatch>,
    function_offsets: HashMap<String, usize>,
    call_patches: Vec<FunctionCallPatch>,
    tail_call_patches: Vec<FunctionCallPatch>,
}

impl EntryEmitter {
    fn new() -> Self {
        Self {
            encoder: Encoder::new(),
            read_only_data: Vec::new(),
            data_address_patches: Vec::new(),
            function_offsets: HashMap::new(),
            call_patches: Vec::new(),
            tail_call_patches: Vec::new(),
        }
    }

    fn emit_module(&mut self, module: &IrModule) -> Result<(), Vec<Diagnostic>> {
        let Some(program) = module
            .functions
            .iter()
            .find(|function| function.name == "program")
        else {
            return Err(vec![Diagnostic::error(
                "E9002",
                "codegen requires a lowered `program` function",
            )]);
        };

        self.emit_process_entry(program);

        for function in &module.functions {
            self.emit_function(function);
        }

        Ok(())
    }

    fn emit_process_entry(&mut self, program: &Function) {
        self.emit_call(&program.name);
        if matches!(program.return_type.success_type(), Type::Void) {
            emit_mov_i32_to_w0(&mut self.encoder, 0);
        }
        emit_darwin_exit_syscall(&mut self.encoder);
    }

    fn emit_function(&mut self, function: &Function) {
        self.function_offsets
            .insert(function.name.clone(), self.encoder.position());

        for instruction in &function.instructions {
            self.emit_instruction(instruction);
        }
    }

    fn emit_instruction(&mut self, instruction: &Instruction) {
        match instruction {
            Instruction::WriteStaticStderr(bytes) => {
                self.emit_write_static_stderr(bytes);
            }
            Instruction::LoadI32Const(value) => {
                emit_mov_i32_to_w0(&mut self.encoder, *value);
            }
            Instruction::TailCall(function) => {
                self.emit_tail_call(function);
            }
            Instruction::Return => {
                self.encoder.emit_ret();
            }
        }
    }

    fn emit_call(&mut self, function: &str) {
        let instruction_offset = self.encoder.position();
        self.encoder.emit_bl(0);
        self.call_patches.push(FunctionCallPatch {
            instruction_offset,
            function: function.to_string(),
        });
    }

    fn emit_tail_call(&mut self, function: &str) {
        let instruction_offset = self.encoder.position();
        self.encoder.emit_b(0);
        self.tail_call_patches.push(FunctionCallPatch {
            instruction_offset,
            function: function.to_string(),
        });
    }

    fn emit_write_static_stderr(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }

        let data_offset = self.read_only_data.len();
        self.read_only_data.extend_from_slice(bytes);

        emit_mov_u64_to_x(&mut self.encoder, XReg::X0, STDERR_FILENO);
        let instruction_offset = self.encoder.position();
        self.encoder.emit_adr_x(XReg::X1, 0);
        self.data_address_patches.push(DataAddressPatch {
            instruction_offset,
            data_offset,
        });
        emit_mov_u64_to_x(&mut self.encoder, XReg::X2, bytes.len() as u64);
        emit_darwin_write_syscall(&mut self.encoder);
    }

    fn finish(mut self) -> Result<MachineCode, Vec<Diagnostic>> {
        self.patch_function_calls()?;

        let read_only_data_base_offset = align_usize(self.encoder.position(), 8);

        for patch in &self.data_address_patches {
            let data_offset = read_only_data_base_offset + patch.data_offset;
            let byte_offset = data_offset as i64 - patch.instruction_offset as i64;
            if !(ADR_MIN_BYTE_OFFSET..=ADR_MAX_BYTE_OFFSET).contains(&byte_offset) {
                return Err(vec![Diagnostic::error(
                    "E9001",
                    "static data is too far from generated code for ARM64 `adr`",
                )]);
            }

            self.encoder
                .patch_adr_x(patch.instruction_offset, XReg::X1, byte_offset as i32);
        }

        Ok(MachineCode {
            text: self.encoder.finish(),
            read_only_data: self.read_only_data,
        })
    }

    fn patch_function_calls(&mut self) -> Result<(), Vec<Diagnostic>> {
        for patch in &self.call_patches {
            let byte_offset = self.function_call_byte_offset(patch, "bl")?;
            self.encoder
                .patch_bl(patch.instruction_offset, byte_offset as i32);
        }

        for patch in &self.tail_call_patches {
            let byte_offset = self.function_call_byte_offset(patch, "b")?;
            self.encoder
                .patch_b(patch.instruction_offset, byte_offset as i32);
        }

        Ok(())
    }

    fn function_call_byte_offset(
        &self,
        patch: &FunctionCallPatch,
        instruction: &str,
    ) -> Result<i64, Vec<Diagnostic>> {
        let Some(target_offset) = self.function_offsets.get(&patch.function) else {
            return Err(vec![Diagnostic::error(
                "E9002",
                format!("codegen could not resolve function `{}`", patch.function),
            )]);
        };

        let byte_offset = *target_offset as i64 - patch.instruction_offset as i64;
        if !(BRANCH_MIN_BYTE_OFFSET..=BRANCH_MAX_BYTE_OFFSET).contains(&byte_offset) {
            return Err(vec![Diagnostic::error(
                "E9002",
                format!(
                    "function `{}` is too far from call site for ARM64 `{instruction}`",
                    patch.function
                ),
            )]);
        }

        Ok(byte_offset)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DataAddressPatch {
    instruction_offset: usize,
    data_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionCallPatch {
    instruction_offset: usize,
    function: String,
}

fn emit_mov_i32_to_w0(encoder: &mut Encoder, value: i32) {
    emit_mov_u32_to_w(encoder, WReg::W0, value as u32);
}

fn emit_mov_u32_to_w(encoder: &mut Encoder, register: WReg, value: u32) {
    encoder.emit_movz_w(register, value as u16, MoveWideShift::Lsl0);

    let high = value >> 16;
    if high != 0 {
        encoder.emit_movk_w(register, high as u16, MoveWideShift::Lsl16);
    }
}

fn emit_mov_u64_to_x(encoder: &mut Encoder, register: XReg, value: u64) {
    encoder.emit_movz_x(register, value as u16, MoveWideShift::Lsl0);

    for (shift, amount) in [
        (MoveWideShift::Lsl16, 16),
        (MoveWideShift::Lsl32, 32),
        (MoveWideShift::Lsl48, 48),
    ] {
        let chunk = (value >> amount) as u16;
        if chunk != 0 {
            encoder.emit_movk_x(register, chunk, shift);
        }
    }
}

fn emit_darwin_exit_syscall(encoder: &mut Encoder) {
    emit_mov_u32_to_w(encoder, WReg::W16, DARWIN_EXIT_SYSCALL);
    encoder.emit_svc(DARWIN_SYSCALL_TRAP);
}

fn emit_darwin_write_syscall(encoder: &mut Encoder) {
    emit_mov_u32_to_w(encoder, WReg::W16, DARWIN_WRITE_SYSCALL);
    encoder.emit_svc(DARWIN_SYSCALL_TRAP);
}

fn align_usize(value: usize, alignment: usize) -> usize {
    debug_assert!(alignment.is_power_of_two());
    (value + alignment - 1) & !(alignment - 1)
}

const STDERR_FILENO: u64 = 2;
const ADR_MIN_BYTE_OFFSET: i64 = -(1 << 20);
const ADR_MAX_BYTE_OFFSET: i64 = (1 << 20) - 1;
const BRANCH_MIN_BYTE_OFFSET: i64 = -(1 << 27);
const BRANCH_MAX_BYTE_OFFSET: i64 = (1 << 27) - 4;
const DARWIN_WRITE_SYSCALL: u32 = 0x0200_0004;
const DARWIN_EXIT_SYSCALL: u32 = 0x0200_0001;
const DARWIN_SYSCALL_TRAP: u16 = 0x80;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Function, Type};

    #[test]
    fn generates_exit_zero_for_return_i32_zero() {
        let module = IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::I32,
            instructions: vec![Instruction::LoadI32Const(0), Instruction::Return],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl program
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x00, 0x00, 0x80, 0x52, // movz w0, #0
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_exit_code_for_return_i32_with_high_halfword() {
        let module = IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::I32,
            instructions: vec![Instruction::LoadI32Const(0x1234_5678), Instruction::Return],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl program
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x00, 0xcf, 0x8a, 0x52, // movz w0, #0x5678
                0x80, 0x46, 0xa2, 0x72, // movk w0, #0x1234, lsl #16
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_exit_code_for_return_i32_negative_one() {
        let module = IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::I32,
            instructions: vec![Instruction::LoadI32Const(-1), Instruction::Return],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl program
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0xe0, 0xff, 0x9f, 0x52, // movz w0, #0xffff
                0xe0, 0xff, 0xbf, 0x72, // movk w0, #0xffff, lsl #16
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_exit_code_for_fallible_success_return_i32() {
        let module = IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![Instruction::LoadI32Const(7), Instruction::Return],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl program
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0xe0, 0x00, 0x80, 0x52, // movz w0, #7
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_same_file_function_call() {
        let module = IrModule::new(vec![
            Function {
                name: "program".to_string(),
                return_type: Type::I32,
                instructions: vec![Instruction::TailCall("answer".to_string())],
            },
            Function {
                name: "answer".to_string(),
                return_type: Type::I32,
                instructions: vec![Instruction::LoadI32Const(7), Instruction::Return],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl program
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x01, 0x00, 0x00, 0x14, // b answer
                0xe0, 0x00, 0x80, 0x52, // movz w0, #7
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_static_stderr_write_with_data_reference() {
        let module = IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::I32,
            instructions: vec![
                Instruction::WriteStaticStderr(b"error\n".to_vec()),
                Instruction::LoadI32Const(1),
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(code.read_only_data, b"error\n");
        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl program
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x40, 0x00, 0x80, 0xd2, // movz x0, #2
                0xe1, 0x00, 0x00, 0x10, // adr x1, #28
                0xc2, 0x00, 0x80, 0xd2, // movz x2, #6
                0x90, 0x00, 0x80, 0x52, // movz w16, #4
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x20, 0x00, 0x80, 0x52, // movz w0, #1
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn generated_static_stderr_write_runs() {
        let module = IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::I32,
            instructions: vec![
                Instruction::WriteStaticStderr(b"failed\n".to_vec()),
                Instruction::LoadI32Const(3),
                Instruction::Return,
            ],
        }]);
        let code = generate_arm64_darwin_entry(&module).unwrap();
        let image = crate::target::macho::write_arm64_macos_executable_with_data(
            &code.text,
            &code.read_only_data,
        );
        let executable = write_temp_executable("codegen-stderr-runs", &image.bytes);

        let output = std::process::Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(executable);

        assert_eq!(output.status.code(), Some(3));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"failed\n");
    }

    #[test]
    fn generates_exit_zero_for_return_void() {
        let module = IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x05, 0x00, 0x00, 0x94, // bl program
                0x00, 0x00, 0x80, 0x52, // movz w0, #0
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn write_temp_executable(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let unique = format!(
            "nocter-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let executable = std::env::temp_dir().join(unique);
        std::fs::write(&executable, bytes).unwrap();
        let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&executable, permissions).unwrap();
        executable
    }
}
