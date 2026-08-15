use super::*;
use crate::ir::IntegerBinaryOperator;

#[test]
fn generates_i32_local_binding_return() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(42),
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0x49, 0x05, 0x80, 0x52, // movz w9, #42
            0xe0, 0x03, 0x09, 0x2a, // mov w0, w9
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_i32_local_addition_binding_return() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(40),
            },
            Instruction::I32Binary {
                operator: IntegerBinaryOperator::Add,
                destination: I32Location::Local(1),
                left: i32_local(0),
                right: i32_const(2),
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(1),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0x09, 0x05, 0x80, 0x52, // movz w9, #40
            0xf0, 0x03, 0x09, 0x2a, // mov w16, w9
            0x4a, 0x00, 0x80, 0x52, // movz w10, #2
            0x0a, 0x02, 0x0a, 0x2b, // adds w10, w16, w10
            0x47, 0x00, 0x00, 0x54, // b.vc +8
            0x00, 0x00, 0x20, 0xd4, // brk #0
            0xe0, 0x03, 0x0a, 0x2a, // mov w0, w10
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_i32_addition_with_overflow_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::I32Binary {
                operator: IntegerBinaryOperator::Add,
                destination: I32Location::Return,
                left: i32_const(40),
                right: i32_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x02, 0x00, 0x2b])); // adds w0, w16, w0
    assert!(contains_instruction(&code.text, [0x47, 0x00, 0x00, 0x54])); // b.vc +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_i32_subtraction_with_overflow_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::I32Binary {
                operator: IntegerBinaryOperator::Subtract,
                destination: I32Location::Return,
                left: i32_const(40),
                right: i32_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x02, 0x00, 0x6b])); // subs w0, w16, w0
    assert!(contains_instruction(&code.text, [0x47, 0x00, 0x00, 0x54])); // b.vc +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_i32_multiplication_with_overflow_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::I32Binary {
                operator: IntegerBinaryOperator::Multiply,
                destination: I32Location::Return,
                left: i32_const(21),
                right: i32_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x11, 0x7e, 0x20, 0x9b])); // smull x17, w16, w0
    assert!(contains_instruction(&code.text, [0x30, 0x7e, 0x40, 0x93])); // sxtw x16, w17
    assert!(contains_instruction(&code.text, [0x3f, 0x02, 0x10, 0xeb])); // cmp x17, x16
    assert!(contains_instruction(&code.text, [0x40, 0x00, 0x00, 0x54])); // b.eq +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_i32_shift_left_with_count_traps() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::I32Binary {
                operator: IntegerBinaryOperator::ShiftLeft,
                destination: I32Location::Return,
                left: i32_const(5),
                right: i32_const(3),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x4a, 0x00, 0x00, 0x54])); // b.ge +8
    assert!(contains_instruction(&code.text, [0x4b, 0x00, 0x00, 0x54])); // b.lt +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x00, 0x22, 0xc0, 0x1a])); // lslv w0, w16, w0
}

#[test]
fn generates_i32_shift_right_with_count_traps() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::I32Binary {
                operator: IntegerBinaryOperator::ShiftRight,
                destination: I32Location::Return,
                left: i32_const(8),
                right: i32_const(1),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x4a, 0x00, 0x00, 0x54])); // b.ge +8
    assert!(contains_instruction(&code.text, [0x4b, 0x00, 0x00, 0x54])); // b.lt +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x00, 0x2a, 0xc0, 0x1a])); // asrv w0, w16, w0
}

#[test]
fn generates_i32_division_with_safety_traps() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::I32Binary {
                operator: IntegerBinaryOperator::Divide,
                destination: I32Location::Return,
                left: i32_const(84),
                right: i32_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x00, 0x0e, 0xc0, 0x1a])); // sdiv w0, w16, w0
}

#[test]
fn generates_i32_remainder_with_safety_traps() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::I32Binary {
                operator: IntegerBinaryOperator::Remainder,
                destination: I32Location::Return,
                left: i32_const(85),
                right: i32_const(43),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x11, 0x0e, 0xc0, 0x1a])); // sdiv w17, w16, w0
    assert!(contains_instruction(&code.text, [0x20, 0xc2, 0x00, 0x1b])); // msub w0, w17, w0, w16
}

#[test]
fn generates_u8_addition_with_range_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::U8,
        instructions: vec![
            Instruction::U8Binary {
                operator: IntegerBinaryOperator::Add,
                destination: U8Location::Return,
                left: u8_const(40),
                right: u8_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x02, 0x00, 0x0b])); // add w0, w16, w0
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_u8_subtraction_with_range_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::U8,
        instructions: vec![
            Instruction::U8Binary {
                operator: IntegerBinaryOperator::Subtract,
                destination: U8Location::Return,
                left: u8_const(40),
                right: u8_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x02, 0x00, 0x4b])); // sub w0, w16, w0
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_u8_multiplication_with_range_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::U8,
        instructions: vec![
            Instruction::U8Binary {
                operator: IntegerBinaryOperator::Multiply,
                destination: U8Location::Return,
                left: u8_const(21),
                right: u8_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x7e, 0x00, 0x1b])); // mul w0, w16, w0
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_u8_division_with_zero_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::U8,
        instructions: vec![
            Instruction::U8Binary {
                operator: IntegerBinaryOperator::Divide,
                destination: U8Location::Return,
                left: u8_const(84),
                right: u8_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0x6b])); // cmp w0, wzr
    assert!(contains_instruction(&code.text, [0x41, 0x00, 0x00, 0x54])); // b.ne +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x00, 0x0a, 0xc0, 0x1a])); // udiv w0, w16, w0
}

#[test]
fn generates_u8_remainder_with_zero_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::U8,
        instructions: vec![
            Instruction::U8Binary {
                operator: IntegerBinaryOperator::Remainder,
                destination: U8Location::Return,
                left: u8_const(85),
                right: u8_const(43),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0x6b])); // cmp w0, wzr
    assert!(contains_instruction(&code.text, [0x41, 0x00, 0x00, 0x54])); // b.ne +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x11, 0x0a, 0xc0, 0x1a])); // udiv w17, w16, w0
}

#[test]
fn generates_u8_shift_left_with_count_and_range_traps() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::U8,
        instructions: vec![
            Instruction::U8Binary {
                operator: IntegerBinaryOperator::ShiftLeft,
                destination: U8Location::Return,
                left: u8_const(5),
                right: u8_const(3),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x22, 0xc0, 0x1a])); // lslv w0, w16, w0
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_u8_shift_right_with_count_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::U8,
        instructions: vec![
            Instruction::U8Binary {
                operator: IntegerBinaryOperator::ShiftRight,
                destination: U8Location::Return,
                left: u8_const(8),
                right: u8_const(1),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x26, 0xc0, 0x1a])); // lsrv w0, w16, w0
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_usize_addition_with_overflow_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Usize,
        instructions: vec![
            Instruction::UsizeBinary {
                operator: IntegerBinaryOperator::Add,
                destination: UsizeLocation::Return,
                left: usize_const(40),
                right: usize_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x02, 0x00, 0xab])); // adds x0, x16, x0
    assert!(contains_instruction(&code.text, [0x43, 0x00, 0x00, 0x54])); // b.cc +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_usize_subtraction_with_underflow_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Usize,
        instructions: vec![
            Instruction::UsizeBinary {
                operator: IntegerBinaryOperator::Subtract,
                destination: UsizeLocation::Return,
                left: usize_const(40),
                right: usize_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x02, 0x00, 0xeb])); // subs x0, x16, x0
    assert!(contains_instruction(&code.text, [0x42, 0x00, 0x00, 0x54])); // b.cs +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_usize_multiplication_with_overflow_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Usize,
        instructions: vec![
            Instruction::UsizeBinary {
                operator: IntegerBinaryOperator::Multiply,
                destination: UsizeLocation::Return,
                left: usize_const(21),
                right: usize_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x11, 0x7e, 0xc0, 0x9b])); // umulh x17, x16, x0
    assert!(contains_instruction(&code.text, [0x3f, 0x02, 0x1f, 0xeb])); // cmp x17, xzr
    assert!(contains_instruction(&code.text, [0x40, 0x00, 0x00, 0x54])); // b.eq +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x00, 0x7e, 0x00, 0x9b])); // mul x0, x16, x0
}

#[test]
fn generates_usize_division_with_zero_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Usize,
        instructions: vec![
            Instruction::UsizeBinary {
                operator: IntegerBinaryOperator::Divide,
                destination: UsizeLocation::Return,
                left: usize_const(84),
                right: usize_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
    assert!(contains_instruction(&code.text, [0x41, 0x00, 0x00, 0x54])); // b.ne +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x00, 0x0a, 0xc0, 0x9a])); // udiv x0, x16, x0
}

#[test]
fn generates_usize_remainder_with_zero_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Usize,
        instructions: vec![
            Instruction::UsizeBinary {
                operator: IntegerBinaryOperator::Remainder,
                destination: UsizeLocation::Return,
                left: usize_const(85),
                right: usize_const(43),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
    assert!(contains_instruction(&code.text, [0x41, 0x00, 0x00, 0x54])); // b.ne +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x11, 0x0a, 0xc0, 0x9a])); // udiv x17, x16, x0
    assert!(contains_instruction(&code.text, [0x20, 0xc2, 0x00, 0x9b])); // msub x0, x17, x0, x16
}

#[test]
fn generates_usize_shift_left_with_count_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Usize,
        instructions: vec![
            Instruction::UsizeBinary {
                operator: IntegerBinaryOperator::ShiftLeft,
                destination: UsizeLocation::Return,
                left: usize_const(5),
                right: usize_const(3),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x11, 0xeb])); // cmp x0, x17
    assert!(contains_instruction(&code.text, [0x43, 0x00, 0x00, 0x54])); // b.cc +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x00, 0x22, 0xc0, 0x9a])); // lslv x0, x16, x0
}

#[test]
fn generates_usize_shift_right_with_count_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Usize,
        instructions: vec![
            Instruction::UsizeBinary {
                operator: IntegerBinaryOperator::ShiftRight,
                destination: UsizeLocation::Return,
                left: usize_const(8),
                right: usize_const(1),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x11, 0xeb])); // cmp x0, x17
    assert!(contains_instruction(&code.text, [0x43, 0x00, 0x00, 0x54])); // b.cc +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x00, 0x26, 0xc0, 0x9a])); // lsrv x0, x16, x0
}
