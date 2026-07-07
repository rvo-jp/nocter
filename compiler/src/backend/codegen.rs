use crate::diagnostics::Diagnostic;
use crate::ir::{Instruction, IrModule};
use crate::target::arm64::{Encoder, MoveWideShift, WReg};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MachineCode {
    pub(crate) text: Vec<u8>,
}

pub(crate) fn generate_arm64(module: &IrModule) -> Result<MachineCode, Vec<Diagnostic>> {
    let mut encoder = Encoder::new();

    for function in &module.functions {
        for instruction in &function.instructions {
            emit_instruction(&mut encoder, instruction);
        }
    }

    Ok(MachineCode {
        text: encoder.finish(),
    })
}

fn emit_instruction(encoder: &mut Encoder, instruction: &Instruction) {
    match instruction {
        Instruction::ReturnI32(value) => {
            emit_mov_i32_to_w0(encoder, *value);
            encoder.emit_ret();
        }
        Instruction::ReturnVoid => {
            encoder.emit_ret();
        }
    }
}

fn emit_mov_i32_to_w0(encoder: &mut Encoder, value: i32) {
    let bits = value as u32;
    encoder.emit_movz_w(WReg::W0, bits as u16, MoveWideShift::Lsl0);

    let high = bits >> 16;
    if high != 0 {
        encoder.emit_movk_w(WReg::W0, high as u16, MoveWideShift::Lsl16);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Function, Type};

    #[test]
    fn generates_return_i32_zero() {
        let module = IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::I32,
            instructions: vec![Instruction::ReturnI32(0)],
        }]);

        let code = generate_arm64(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x00, 0x00, 0x80, 0x52, // movz w0, #0
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_return_i32_with_high_halfword() {
        let module = IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::I32,
            instructions: vec![Instruction::ReturnI32(0x1234_5678)],
        }]);

        let code = generate_arm64(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x00, 0xcf, 0x8a, 0x52, // movz w0, #0x5678
                0x80, 0x46, 0xa2, 0x72, // movk w0, #0x1234, lsl #16
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_return_i32_negative_one() {
        let module = IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::I32,
            instructions: vec![Instruction::ReturnI32(-1)],
        }]);

        let code = generate_arm64(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0xe0, 0xff, 0x9f, 0x52, // movz w0, #0xffff
                0xe0, 0xff, 0xbf, 0x72, // movk w0, #0xffff, lsl #16
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_return_void() {
        let module = IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::Void,
            instructions: vec![Instruction::ReturnVoid],
        }]);

        let code = generate_arm64(&module).unwrap();

        assert_eq!(code.text, vec![0xc0, 0x03, 0x5f, 0xd6]);
    }
}
