use super::*;

#[test]
fn generates_str_len_value_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "size".to_string(),
            target: crate::ir::CallTarget::same_file("size".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::StrLen(StrLocation::Parameter(0)),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0xe0, 0x03, 0x01, 0xaa])); // mov x0, x1
}

#[test]
fn generates_slice_len_value_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "size".to_string(),
            target: crate::ir::CallTarget::same_file("size".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::SliceLen(SliceLocation::Parameter(0)),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0xe0, 0x03, 0x01, 0xaa])); // mov x0, x1
}

#[test]
fn generates_slice_index_byte_load_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: U8Value::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: UsizeValue::Const(1),
                    },
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x20, 0x6a, 0x70, 0x38])); // ldrb w0, [x17, x16]
}

#[test]
fn generates_slice_index_usize_load_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: Box::new(UsizeValue::Const(1)),
                    },
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(
        &code.text,
        encoded_lsl_x_imm(XReg::X16, XReg::X16, 3),
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_adds_x(XReg::X17, XReg::X17, XReg::X16),
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_imm(XReg::X0, XReg::X17, 0),
    ));
}

#[test]
fn generates_slice_index_i32_load_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: UsizeValue::Const(1),
                    },
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(
        &code.text,
        encoded_lsl_x_imm(XReg::X16, XReg::X16, 2),
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_w_reg(WReg::W0, XReg::X17, XReg::X16),
    ));
}

#[test]
fn generates_slice_index_bool_load_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: UsizeValue::Const(1),
                    },
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(
        &code.text,
        encoded_ldrb_w_reg(WReg::W0, XReg::X17, XReg::X16),
    ));
}

#[test]
fn generates_stack_passed_slice_index_byte_load_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: U8Value::SliceIndex {
                        source: SliceLocation::Parameter(8),
                        index: UsizeValue::Const(1),
                    },
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0xf1, 0x07, 0x40, 0xf9])); // ldr x17, [sp, #8]
    assert!(contains_instruction(&code.text, [0xf1, 0x03, 0x40, 0xf9])); // ldr x17, [sp, #0]
    assert!(contains_instruction(&code.text, [0x20, 0x6a, 0x70, 0x38])); // ldrb w0, [x17, x16]
}

#[test]
fn generates_u8_to_i32_zero_extend_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::U8ZeroExtend(Box::new(U8Value::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: UsizeValue::Const(1),
                    })),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x20, 0x6a, 0x70, 0x38])); // ldrb w0, [x17, x16]
}

#[test]
fn generates_u8_comparison_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "check".to_string(),
            target: crate::ir::CallTarget::same_file("check".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: I32Value::U8ZeroExtend(Box::new(U8Value::SliceIndex {
                            source: SliceLocation::Parameter(0),
                            index: UsizeValue::Const(0),
                        })),
                        right: I32Value::U8ZeroExtend(Box::new(U8Value::Const(0x7F))),
                    },
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x30, 0x6a, 0x70, 0x38])); // ldrb w16, [x17, x16]
    assert!(contains_instruction(&code.text, [0x20, 0x00, 0x80, 0x52])); // mov w0, #1
}

#[test]
fn generated_str_literal_argument_uses_two_abi_words() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::TailCall {
                target: CallTarget::same_file("consume"),
                arguments: vec![
                    ScalarArgument::Str(StrValue::StaticBytes(b"Nocter".to_vec())),
                    ScalarArgument::I32(I32Value::Const(42)),
                ],
            }],
        },
        Function {
            name: "consume".to_string(),
            target: crate::ir::CallTarget::same_file("consume".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_param(2),
                },
                Instruction::Return,
            ],
        },
    ]);
    let code = generate_arm64_darwin_entry(&module).unwrap();
    assert_eq!(code.read_only_data, b"Nocter");
    let image = crate::target::macho::write_arm64_macos_executable_with_data(
        &code.text,
        &code.read_only_data,
    );
    let executable = write_temp_executable("codegen-str-argument-runs", &image.bytes);

    let output = std::process::Command::new(&executable).output().unwrap();
    let _ = std::fs::remove_file(executable);

    assert_eq!(output.status.code(), Some(42));
}
