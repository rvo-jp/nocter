use super::*;
use crate::ir::{AggregateIndex, AggregateRange};

#[test]
fn aggregate_range_copy_from_stack_passed_direct_parameter_reads_unaligned_bytes() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(8, 1),
            },
            Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(0),
                destination_offset: 0,
                source: AggregateLocation::DirectParameter { start_index: 8 },
                source_offset: 4,
                layout: ValueLayout::new(5, 1),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X17, 16)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X17, 24)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X17, XReg::X17, 32)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_sp(WReg::W17, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_sp(WReg::W17, 4)
    ));
}

#[test]
fn aggregate_range_copy_from_stack_passed_direct_parameter_after_call_uses_spills() {
    let layout = ValueLayout::new(9, 1);
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "identity".to_string(),
            target: crate::ir::CallTarget::same_file("identity".to_string()),
            return_type: Type::DirectAggregate { layout, words: 2 },
            instructions: vec![
                Instruction::CallVoid {
                    target: CallTarget::same_file("effect"),
                    arguments: vec![],
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::DirectReturn,
                    destination_offset: 0,
                    source: AggregateLocation::DirectParameter { start_index: 8 },
                    source_offset: 0,
                    layout,
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
        encoded_str_x_sp(XReg::X16, 8)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_mov_x(XReg::X0, XReg::X16)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X17, 8)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_bfi_x(XReg::X1, XReg::X16, 0, 8)
    ));
}

#[test]
fn aggregate_range_copy_to_borrowed_parameter_after_call_uses_spilled_parameter_pointer() {
    let layout = ValueLayout::new(16, 8);
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "set_header".to_string(),
            target: crate::ir::CallTarget::same_file("set_header".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: UsizeValue::Const(7),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: UsizeValue::Const(42),
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("effect"),
                    arguments: vec![],
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::Parameter(0),
                    destination_offset: 8,
                    source: AggregateLocation::Slot(0),
                    source_offset: 0,
                    layout,
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
        encoded_ldr_x_sp(XReg::X17, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X16, XReg::X17, 8)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X16, XReg::X17, 16)
    ));
}

#[test]
fn aggregate_copy_from_slot_to_return_copies_words_to_x8_destination() {
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
            return_type: Type::Aggregate {
                layout: ValueLayout::new(24, 8),
            },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::CallAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Return,
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::Aggregate {
                layout: ValueLayout::new(24, 8),
            },
            instructions: vec![
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 0,
                    value: UsizeValue::Const(7),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X8, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X8, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 8)
    ));
    assert!(contains_instruction(&code.text, [0x10, 0x01, 0x00, 0xf9])); // str x16, [x8, #0]
    assert!(contains_instruction(&code.text, [0x10, 0x09, 0x00, 0xf9])); // str x16, [x8, #16]
}

#[test]
fn aggregate_range_copy_to_direct_return_second_word_is_allowed() {
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
            return_type: Type::DirectAggregate {
                layout: ValueLayout::new(16, 8),
                words: 2,
            },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(1, 1),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: U8Value::Const(42),
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::DirectReturn,
                    destination_offset: 8,
                    source: AggregateLocation::Slot(0),
                    source_offset: 0,
                    layout: ValueLayout::new(1, 1),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldrb_w_sp(WReg::W16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_bfi_x(XReg::X1, XReg::X16, 0, 8)
    ));
}

#[test]
fn aggregate_range_copy_to_direct_return_rejects_range_past_second_word() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::DirectAggregate {
            layout: ValueLayout::new(16, 8),
            words: 2,
        },
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(9, 1),
            },
            Instruction::CopyAggregateRange {
                destination: AggregateLocation::DirectReturn,
                destination_offset: 8,
                source: AggregateLocation::Slot(0),
                source_offset: 0,
                layout: ValueLayout::new(9, 1),
            },
            Instruction::Return,
        ],
    }]);

    let diagnostics = generate_arm64_darwin_entry(&module).unwrap_err();

    assert!(
        diagnostics[0]
            .message
            .contains("direct aggregate return range exceeds two ABI words"),
        "{diagnostics:?}"
    );
}

#[test]
fn aggregate_copy_from_slot_to_slot_copies_words_between_stack_slots() {
    let layout = ValueLayout::new(24, 8);
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout,
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout,
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(1),
                offset: 0,
                value: UsizeValue::Const(7),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(1),
                offset: 8,
                value: UsizeValue::Const(8),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(1),
                offset: 16,
                value: UsizeValue::Const(9),
            },
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(0),
                source: AggregateLocation::Slot(1),
                layout,
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 24)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 32)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 8)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 40)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 16)
    ));
}

#[test]
fn projected_aggregate_copy_preserves_both_checked_index_addresses() {
    let element = ValueLayout::new(4, 4);
    let array = ValueLayout::new(8, 4);
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: CallTarget::same_file("main"),
        return_type: Type::I32,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: array,
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: array,
            },
            Instruction::CopyAggregateProjected {
                destination: AggregateRange {
                    location: AggregateLocation::Slot(0),
                    offset: 0,
                    index: Some(AggregateIndex {
                        value: UsizeValue::Const(0),
                        length: 2,
                        stride: 4,
                    }),
                },
                source: AggregateRange {
                    location: AggregateLocation::Slot(1),
                    offset: 0,
                    index: Some(AggregateIndex {
                        value: UsizeValue::Const(1),
                        length: 2,
                        stride: 4,
                    }),
                },
                layout: element,
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_mov_x(XReg::X14, XReg::X17)
    ));
}

#[test]
fn aggregate_borrow_argument_passes_slot_address() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("touch"),
                    arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlot(0),
                    })],
                },
                set_return_i32(0),
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

    assert!(contains_instruction(&code.text, [0xf0, 0x23, 0x00, 0x91])); // add x16, sp, #8
    assert!(contains_instruction(&code.text, [0xf0, 0x03, 0x00, 0xf9])); // str x16, [sp, #0]
    assert!(contains_instruction(&code.text, [0xe0, 0x03, 0x40, 0xf9])); // ldr x0, [sp, #0]
}

#[test]
fn direct_aggregate_argument_passes_slot_words() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: usize_const(40),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: usize_const(2),
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("consume"),
                    arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                        source: AggregateArgumentSource::Slot(0),
                        layout: ValueLayout::new(16, 8),
                        words: 2,
                    })],
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        },
        Function {
            name: "consume".to_string(),
            target: crate::ir::CallTarget::same_file("consume".to_string()),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 16)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 24)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 8)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X0, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X1, 8)
    ));
}

#[test]
fn direct_aggregate_argument_zero_extends_partial_final_word() {
    let layout = ValueLayout::new(9, 1);
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: usize_const(0x0102_0304_0506_0708),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: U8Value::Const(0xaa),
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("consume"),
                    arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                        source: AggregateArgumentSource::Slot(0),
                        layout,
                        words: 2,
                    })],
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        },
        Function {
            name: "consume".to_string(),
            target: crate::ir::CallTarget::same_file("consume".to_string()),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldrb_w_sp(WReg::W16, 24)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 8)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X1, 8)
    ));
}

#[test]
fn indirect_aggregate_parameter_copy_reads_from_parameter_pointer() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "length".to_string(),
            target: crate::ir::CallTarget::same_file("length".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    source: AggregateLocation::Parameter(0),
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::LoadAggregateUsize {
                    destination: UsizeLocation::Return,
                    source: AggregateLocation::Slot(0),
                    offset: 8,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_imm(XReg::X16, XReg::X0, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_imm(XReg::X16, XReg::X0, 8)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_imm(XReg::X16, XReg::X0, 16)
    ));
}

#[test]
fn borrowed_aggregate_parameter_field_read_loads_from_parameter_pointer() {
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
                    source: AggregateLocation::Parameter(0),
                    offset: 4,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldr_w_imm(WReg::W0, XReg::X0, 4)
    ));
}

#[test]
fn readwrite_borrowed_aggregate_parameter_field_write_stores_to_parameter_pointer() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "set_code".to_string(),
            target: crate::ir::CallTarget::same_file("set_code".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Parameter(0),
                    offset: 4,
                    value: I32Value::Const(99),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_w_imm(WReg::W16, XReg::X0, 4)
    ));
}

#[test]
fn borrowed_aggregate_parameter_field_write_after_call_uses_spilled_parameter_pointer() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "set_code".to_string(),
            target: crate::ir::CallTarget::same_file("set_code".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::CallVoid {
                    target: CallTarget::same_file("effect"),
                    arguments: vec![],
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Parameter(0),
                    offset: 4,
                    value: I32Value::Const(99),
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
        encoded_ldr_x_sp(XReg::X17, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_w_imm(WReg::W16, XReg::X17, 4)
    ));
}
