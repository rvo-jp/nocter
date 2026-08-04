use super::*;

#[test]
fn aggregate_call_passes_destination_slot_in_x8() {
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
                Instruction::CallAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                },
                set_return_i32(0),
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
                    offset: 16,
                    value: UsizeValue::Const(7),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0xe8, 0x03, 0x00, 0x91])); // add x8, sp, #0
    assert!(contains_instruction(&code.text, [0x10, 0x09, 0x00, 0xf9])); // str x16, [x8, #16]
}

#[test]
fn aggregate_return_call_restores_saved_x8_destination() {
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
                Instruction::CallAggregate {
                    destination: AggregateLocation::Return,
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
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
                    offset: 16,
                    value: UsizeValue::Const(7),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(!contains_instruction(&code.text, [0xe8, 0x03, 0x00, 0x91])); // add x8, sp, #0
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X8, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X8, 0)
    ));
    assert!(contains_instruction(&code.text, [0x10, 0x09, 0x00, 0xf9])); // str x16, [x8, #16]
}

#[test]
fn direct_aggregate_call_result_stores_partial_final_word_to_slot() {
    let layout = ValueLayout::new(9, 1);
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
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::CallDirectAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    layout,
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        },
        Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::DirectAggregate { layout, words: 2 },
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
                Instruction::CopyAggregate {
                    destination: AggregateLocation::DirectReturn,
                    source: AggregateLocation::Slot(0),
                    layout,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X0, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_sp(WReg::W1, 8)
    ));
}

#[test]
fn fallible_aggregate_call_passes_destination_slot_and_checks_status() {
    let aggregate = Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    };
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
            return_type: Type::Fallible(Box::new(aggregate.clone())),
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::CallOutcomeAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Propagate,
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Return,
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::ReturnOutcomeSuccess,
            ],
        },
        Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::Fallible(Box::new(aggregate)),
            instructions: vec![
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 0,
                    value: UsizeValue::Const(7),
                },
                Instruction::ReturnOutcomeSuccess,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0xe8, 0x23, 0x00, 0x91])); // add x8, sp, #8
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X8, 0)
    ));
    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
    assert!(contains_instruction(&code.text, [0x10, 0x01, 0x00, 0xf9])); // str x16, [x8, #0]
}

#[test]
fn fallible_direct_aggregate_call_stores_success_payload_words() {
    let aggregate = Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    };
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
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CallOutcomeDirectAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    layout: ValueLayout::new(16, 8),
                    failure_mode: OutcomeFailureMode::Propagate,
                },
                set_return_i32(0),
                Instruction::ReturnOutcomeSuccess,
            ],
        },
        Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::Fallible(Box::new(aggregate)),
            instructions: vec![
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::DirectReturn,
                    offset: 0,
                    value: UsizeValue::Const(7),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::DirectReturn,
                    offset: 8,
                    value: UsizeValue::Const(9),
                },
                Instruction::ReturnOutcomeSuccess,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
    assert!(contains_instruction(&code.text, [0xe1, 0x03, 0x00, 0xf9])); // str x1, [sp, #0]
    assert!(contains_instruction(&code.text, [0xe2, 0x07, 0x00, 0xf9])); // str x2, [sp, #8]
}

#[test]
fn fallible_direct_aggregate_call_stores_partial_final_payload_word() {
    let layout = ValueLayout::new(9, 1);
    let aggregate = Type::DirectAggregate { layout, words: 2 };
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
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::CallOutcomeDirectAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    layout,
                    failure_mode: OutcomeFailureMode::Propagate,
                },
                set_return_i32(0),
                Instruction::ReturnOutcomeSuccess,
            ],
        },
        Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::Fallible(Box::new(aggregate)),
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
                Instruction::CopyAggregate {
                    destination: AggregateLocation::DirectReturn,
                    source: AggregateLocation::Slot(0),
                    layout,
                },
                Instruction::ReturnOutcomeSuccess,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X1, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_sp(WReg::W2, 8)
    ));
}

#[test]
fn fallible_direct_aggregate_call_forwards_partial_payload_to_direct_return() {
    let layout = ValueLayout::new(9, 1);
    let aggregate = Type::DirectAggregate { layout, words: 2 };
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
            return_type: aggregate.clone(),
            instructions: vec![
                Instruction::CallOutcomeDirectAggregate {
                    destination: AggregateLocation::DirectReturn,
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    layout,
                    failure_mode: OutcomeFailureMode::Trap,
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::Fallible(Box::new(aggregate)),
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
                Instruction::CopyAggregate {
                    destination: AggregateLocation::DirectReturn,
                    source: AggregateLocation::Slot(0),
                    layout,
                },
                Instruction::ReturnOutcomeSuccess,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
    assert!(contains_instruction(
        &code.text,
        encoded_mov_x(XReg::X0, XReg::X1)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_mov_x(XReg::X1, XReg::X2)
    ));
}
