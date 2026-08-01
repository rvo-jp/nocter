use super::*;

#[test]
fn pointer_u8_store_uses_byte_store() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::StoreU8ToPointer {
                pointer: UsizeValue::Const(4096),
                offset: UsizeValue::Const(4),
                value: U8Value::Const(0),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_imm(WReg::W2, XReg::X0, 0)
    ));
}

#[test]
fn pointer_i32_store_uses_word_store() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::StoreI32ToPointer {
                pointer: UsizeValue::Const(4096),
                offset: UsizeValue::Const(4),
                value: I32Value::Const(42),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_w_imm(WReg::W2, XReg::X0, 0)
    ));
}

#[test]
fn pointer_usize_store_uses_word_store() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::StoreUsizeToPointer {
                pointer: UsizeValue::Const(4096),
                offset: UsizeValue::Const(8),
                value: UsizeValue::Const(42),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X2, XReg::X0, 0)
    ));
}

#[test]
fn pointer_bool_store_uses_byte_store() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::StoreBoolToPointer {
                pointer: UsizeValue::Const(4096),
                offset: UsizeValue::Const(1),
                value: BoolValue::Const(true),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_imm(WReg::W2, XReg::X0, 0)
    ));
}

#[test]
fn pointer_str_store_uses_two_word_stores() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::StoreStrToPointer {
                pointer: UsizeValue::Const(4096),
                offset: UsizeValue::Const(16),
                value: StrValue::StaticBytes(b"arg".to_vec()),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X2, XReg::X0, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X3, XReg::X0, 8)
    ));
}

#[test]
fn pointer_aggregate_copy_stores_slot_bytes() {
    let layout = ValueLayout { size: 4, align: 4 };
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout,
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: I32Value::Const(42),
            },
            Instruction::CopyAggregateToPointer {
                pointer: UsizeValue::Const(4096),
                offset: UsizeValue::Const(4),
                source: AggregateLocation::Slot(0),
                layout,
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_w_imm(WReg::W16, XReg::X9, 0)
    ));
}

#[test]
fn slice_u8_index_store_uses_byte_store() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "fill".to_string(),
            target: crate::ir::CallTarget::same_file("fill".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::StoreU8ToSliceIndex {
                    destination: SliceLocation::Parameter(0),
                    index: UsizeValue::Const(0),
                    value: U8Value::Const(7),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_imm(WReg::W2, XReg::X0, 0)
    ));
}

#[test]
fn slice_i32_index_store_uses_word_store() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "fill".to_string(),
            target: crate::ir::CallTarget::same_file("fill".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::StoreI32ToSliceIndex {
                    destination: SliceLocation::Parameter(0),
                    index: UsizeValue::Const(1),
                    value: I32Value::Const(42),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    let value_offset = instruction_offset(&code.text, &encoded_mov_u32_to_w(WReg::W2, 42))
        .expect("expected i32 store value materialization");
    let scale_offset = instruction_offset(&code.text, &encoded_lsl_x_imm(XReg::X16, XReg::X16, 2))
        .expect("expected i32 slice index scaling");
    assert!(
        value_offset < scale_offset,
        "slice index stores must materialize the assigned value before computing the destination address"
    );
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(
        &code.text,
        encoded_lsl_x_imm(XReg::X16, XReg::X16, 2),
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_w_imm(WReg::W2, XReg::X0, 0)
    ));
}

#[test]
fn slice_i32_index_borrow_argument_uses_element_address() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "forward".to_string(),
            target: crate::ir::CallTarget::same_file("forward".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::CallVoid {
                    target: CallTarget::same_file("touch"),
                    arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::SliceIndex {
                            source: SliceLocation::Parameter(0),
                            index: SliceElementIndex::Const(1),
                            element: SliceElementAddressKind::I32,
                        },
                    })],
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "touch".to_string(),
            target: crate::ir::CallTarget::same_file("touch".to_string()),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
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
        encoded_adds_x(XReg::X16, XReg::X8, XReg::X16),
    ));
}

#[test]
fn slice_usize_index_store_uses_word_store() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "fill".to_string(),
            target: crate::ir::CallTarget::same_file("fill".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::StoreUsizeToSliceIndex {
                    destination: SliceLocation::Parameter(0),
                    index: UsizeValue::Const(1),
                    value: UsizeValue::Const(42),
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
        encoded_str_x_imm(XReg::X2, XReg::X0, 0)
    ));
}

#[test]
fn slice_bool_index_store_uses_byte_store() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "fill".to_string(),
            target: crate::ir::CallTarget::same_file("fill".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::StoreBoolToSliceIndex {
                    destination: SliceLocation::Parameter(0),
                    index: UsizeValue::Const(0),
                    value: BoolValue::Const(true),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_imm(WReg::W2, XReg::X0, 0)
    ));
}

#[test]
fn slice_str_index_store_uses_two_word_stores() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "fill".to_string(),
            target: crate::ir::CallTarget::same_file("fill".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::StoreStrToSliceIndex {
                    destination: SliceLocation::Parameter(0),
                    index: UsizeValue::Const(1),
                    value: StrValue::StaticBytes(b"arg".to_vec()),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(
        &code.text,
        encoded_lsl_x_imm(XReg::X16, XReg::X16, 4),
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X2, XReg::X0, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X3, XReg::X0, 8)
    ));
}
