use super::*;

#[test]
fn aggregate_scalar_field_stores_use_field_width() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::Aggregate {
                layout: ValueLayout::new(32, 8),
            },
            instructions: vec![
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Return,
                    offset: 0,
                    value: U8Value::Const(7),
                },
                Instruction::StoreAggregateBool {
                    destination: AggregateLocation::Return,
                    offset: 1,
                    value: BoolValue::Const(true),
                },
                Instruction::StoreAggregateU16 {
                    destination: AggregateLocation::Return,
                    offset: 2,
                    value: 0xaabb,
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Return,
                    offset: 4,
                    value: I32Value::Const(42),
                },
                Instruction::StoreAggregateU32 {
                    destination: AggregateLocation::Return,
                    offset: 12,
                    value: 0xaabb_ccdd,
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 16,
                    value: UsizeValue::Const(11),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_imm(WReg::W16, XReg::X8, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_imm(WReg::W16, XReg::X8, 1)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_strh_w_imm(WReg::W16, XReg::X8, 2)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_w_imm(WReg::W16, XReg::X8, 4)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_w_imm(WReg::W16, XReg::X8, 12)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X16, XReg::X8, 16)
    ));
}

#[test]
fn aggregate_scalar_field_stores_to_slot_use_frame_offsets() {
    let layout = ValueLayout::new(8, 4);
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout,
            },
            Instruction::StoreAggregateU8 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: U8Value::Const(7),
            },
            Instruction::StoreAggregateBool {
                destination: AggregateLocation::Slot(0),
                offset: 1,
                value: BoolValue::Const(true),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 4,
                value: I32Value::Const(42),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_sp(WReg::W16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_sp(WReg::W16, 1)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_w_sp(WReg::W16, 4)
    ));
}

#[test]
fn aggregate_scalar_field_loads_from_slot_use_frame_offsets() {
    let layout = ValueLayout::new(16, 8);
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout,
            },
            Instruction::LoadAggregateU8 {
                destination: U8Location::Return,
                source: AggregateLocation::Slot(0),
                offset: 0,
            },
            Instruction::LoadAggregateBool {
                destination: BoolLocation::Return,
                source: AggregateLocation::Slot(0),
                offset: 1,
            },
            Instruction::LoadAggregateI32 {
                destination: I32Location::Return,
                source: AggregateLocation::Slot(0),
                offset: 4,
            },
            Instruction::LoadAggregateUsize {
                destination: UsizeLocation::Return,
                source: AggregateLocation::Slot(0),
                offset: 8,
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldrb_w_sp(WReg::W0, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldrb_w_sp(WReg::W0, 1)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_w_sp(WReg::W0, 4)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X0, 8)
    ));
}

#[test]
fn direct_aggregate_parameter_i32_field_load_extracts_register_word() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "code".to_string(),
            target: crate::ir::CallTarget::same_file("code".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::DirectParameter { start_index: 0 },
                    offset: 4,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X16, XReg::X16, 32)
    ));
}

#[test]
fn direct_aggregate_parameter_i32_field_load_after_call_reads_spilled_word() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "code".to_string(),
            target: crate::ir::CallTarget::same_file("code".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallVoid {
                    target: CallTarget::same_file("effect"),
                    arguments: vec![],
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::DirectParameter { start_index: 0 },
                    offset: 0,
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "effect".to_string(),
            target: crate::ir::CallTarget::same_file("effect".to_string()),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_mov_w(WReg::W0, WReg::W16)
    ));
}

#[test]
fn direct_aggregate_parameter_i32_field_load_reads_shifted_second_word() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "code".to_string(),
            target: crate::ir::CallTarget::same_file("code".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::DirectParameter { start_index: 2 },
                    offset: 12,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_mov_x(XReg::X16, XReg::X3)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X16, XReg::X16, 32)
    ));
}

#[test]
fn direct_aggregate_parameter_u8_field_load_masks_selected_byte() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "read".to_string(),
            target: crate::ir::CallTarget::same_file("read".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::LoadAggregateU8 {
                    destination: U8Location::Return,
                    source: AggregateLocation::DirectParameter { start_index: 0 },
                    offset: 2,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X16, XReg::X16, 16)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsl_x_imm(XReg::X16, XReg::X16, 56)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X16, XReg::X16, 56)
    ));
}

#[test]
fn direct_aggregate_parameter_u8_field_load_reads_shifted_start_register() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "read".to_string(),
            target: crate::ir::CallTarget::same_file("read".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::LoadAggregateU8 {
                    destination: U8Location::Return,
                    source: AggregateLocation::DirectParameter { start_index: 2 },
                    offset: 9,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_mov_x(XReg::X16, XReg::X3)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X16, XReg::X16, 8)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsl_x_imm(XReg::X16, XReg::X16, 56)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X16, XReg::X16, 56)
    ));
}

#[test]
fn direct_aggregate_parameter_u8_field_load_reads_stack_word() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "read".to_string(),
            target: crate::ir::CallTarget::same_file("read".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::LoadAggregateU8 {
                    destination: U8Location::Return,
                    source: AggregateLocation::DirectParameter { start_index: 8 },
                    offset: 4,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X16, XReg::X16, 32)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsl_x_imm(XReg::X16, XReg::X16, 56)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X16, XReg::X16, 56)
    ));
}

#[test]
fn direct_aggregate_parameter_usize_field_load_reads_stack_word() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "read".to_string(),
            target: crate::ir::CallTarget::same_file("read".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::LoadAggregateUsize {
                    destination: UsizeLocation::Return,
                    source: AggregateLocation::DirectParameter { start_index: 8 },
                    offset: 8,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X0, 8)
    ));
}
