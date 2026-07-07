use crate::diagnostics::Diagnostic;
use crate::ir::{Instruction, IrModule};
use crate::target::arm64::{Encoder, MoveWideShift, WReg};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MachineCode {
    pub(crate) text: Vec<u8>,
}

pub(crate) fn generate_arm64_darwin_entry(
    module: &IrModule,
) -> Result<MachineCode, Vec<Diagnostic>> {
    let mut encoder = Encoder::new();

    for function in &module.functions {
        for instruction in &function.instructions {
            emit_entry_instruction(&mut encoder, instruction);
        }
    }

    Ok(MachineCode {
        text: encoder.finish(),
    })
}

fn emit_entry_instruction(encoder: &mut Encoder, instruction: &Instruction) {
    match instruction {
        Instruction::ReturnI32(value) => {
            emit_mov_i32_to_w0(encoder, *value);
            emit_darwin_exit_syscall(encoder);
        }
        Instruction::ReturnVoid => {
            emit_mov_i32_to_w0(encoder, 0);
            emit_darwin_exit_syscall(encoder);
        }
    }
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

fn emit_darwin_exit_syscall(encoder: &mut Encoder) {
    emit_mov_u32_to_w(encoder, WReg::W16, DARWIN_EXIT_SYSCALL);
    encoder.emit_svc(DARWIN_SYSCALL_TRAP);
}

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
            instructions: vec![Instruction::ReturnI32(0)],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x00, 0x00, 0x80, 0x52, // movz w0, #0
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
            ]
        );
    }

    #[test]
    fn generates_exit_code_for_return_i32_with_high_halfword() {
        let module = IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::I32,
            instructions: vec![Instruction::ReturnI32(0x1234_5678)],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x00, 0xcf, 0x8a, 0x52, // movz w0, #0x5678
                0x80, 0x46, 0xa2, 0x72, // movk w0, #0x1234, lsl #16
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
            ]
        );
    }

    #[test]
    fn generates_exit_code_for_return_i32_negative_one() {
        let module = IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::I32,
            instructions: vec![Instruction::ReturnI32(-1)],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0xe0, 0xff, 0x9f, 0x52, // movz w0, #0xffff
                0xe0, 0xff, 0xbf, 0x72, // movk w0, #0xffff, lsl #16
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
            ]
        );
    }

    #[test]
    fn generates_exit_zero_for_return_void() {
        let module = IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::Void,
            instructions: vec![Instruction::ReturnVoid],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x00, 0x00, 0x80, 0x52, // movz w0, #0
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
            ]
        );
    }
}
