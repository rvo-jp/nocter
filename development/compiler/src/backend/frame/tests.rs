use super::*;
use crate::ir::{
    AggregateArgument, AggregateArgumentSource, BoolComparisonOperator, BorrowArgument, CallTarget,
    DirectAggregateArgument, IntegerBinaryOperator, OutcomeFailureMode, ScalarArgument,
    SliceLocation, SliceValue, StrValue, Type,
};

#[test]
fn plans_current_ir_functions_as_frameless() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![Instruction::TailCall {
            target: CallTarget::same_file("answer"),
            arguments: vec![],
        }],
    };

    assert_eq!(
        plan_function_frame(&function).unwrap(),
        FunctionFrame::Frameless
    );
}

#[test]
fn stack_backed_scalar_local_requires_frame_without_calls() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![Instruction::SetI32 {
            destination: I32Location::Local(7),
            value: I32Value::Const(42),
        }],
    };

    let frame = plan_function_frame(&function).unwrap();

    let FunctionFrame::Framed(layout) = frame else {
        panic!("expected stack-backed local to require a frame");
    };
    assert_eq!(
        layout.scalar_spill_slot(7).map(|slot| slot.offset()),
        Some(56)
    );
}

#[test]
fn computes_aligned_frame_with_saved_x30_only() {
    let layout = FrameLayout::for_slot_counts(0, 0).unwrap();

    assert_eq!(layout.frame_size(), 16);
    assert_eq!(layout.saved_x30_offset(), 8);
    assert!(layout.scalar_spill_slots().is_empty());
    assert!(layout.argument_staging_slots().is_empty());
}

#[test]
fn computes_scalar_spill_slots_below_saved_x30() {
    let layout = FrameLayout::for_slot_counts(3, 0).unwrap();

    assert_eq!(layout.frame_size(), 32);
    assert_eq!(layout.saved_x30_offset(), 24);
    assert_eq!(
        layout.scalar_spill_slots(),
        &[
            ScalarSpillSlot {
                local_index: 0,
                offset: 0
            },
            ScalarSpillSlot {
                local_index: 1,
                offset: 8
            },
            ScalarSpillSlot {
                local_index: 2,
                offset: 16
            },
        ]
    );
}

#[test]
fn computes_argument_staging_slots_above_scalar_spills() {
    let layout = FrameLayout::for_slot_counts(2, 3).unwrap();

    assert_eq!(layout.frame_size(), 48);
    assert_eq!(layout.saved_x30_offset(), 40);
    assert_eq!(
        layout.argument_staging_slots(),
        &[
            ArgumentStagingSlot {
                abi_word_index: 0,
                offset: 16
            },
            ArgumentStagingSlot {
                abi_word_index: 1,
                offset: 24
            },
            ArgumentStagingSlot {
                abi_word_index: 2,
                offset: 32
            },
        ]
    );
}

#[test]
fn computes_parameter_spill_slots_below_scalar_and_argument_slots() {
    let layout = FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
        2,
        1,
        &[8, 0, 8],
        &[],
        false,
    )
    .unwrap();

    assert_eq!(layout.frame_size(), 48);
    assert_eq!(layout.saved_x30_offset(), 40);
    assert_eq!(
        layout.parameter_spill_slots(),
        &[
            ParameterSpillSlot {
                parameter_index: 0,
                offset: 0
            },
            ParameterSpillSlot {
                parameter_index: 8,
                offset: 8
            },
        ]
    );
    assert_eq!(
        layout.scalar_spill_slots(),
        &[
            ScalarSpillSlot {
                local_index: 0,
                offset: 16
            },
            ScalarSpillSlot {
                local_index: 1,
                offset: 24
            },
        ]
    );
    assert_eq!(
        layout.argument_staging_slots(),
        &[ArgumentStagingSlot {
            abi_word_index: 0,
            offset: 32
        }]
    );
}

#[test]
fn computes_aggregate_slots_above_argument_staging_with_alignment() {
    let layout = FrameLayout::for_slot_counts_with_aggregate_slots(
        1,
        1,
        &[
            AggregateSlotRequest::new(0, ValueLayout::new(24, 8)),
            AggregateSlotRequest::new(1, ValueLayout::new(16, 16)),
        ],
    )
    .unwrap();

    assert_eq!(layout.frame_size(), 80);
    assert_eq!(layout.saved_x30_offset(), 72);
    assert_eq!(
        layout.aggregate_slots(),
        &[
            AggregateSlot {
                slot_index: 0,
                offset: 16,
                size: 24,
                align: 8,
            },
            AggregateSlot {
                slot_index: 1,
                offset: 48,
                size: 16,
                align: 16,
            },
        ]
    );
    assert_eq!(layout.aggregate_slot(1).unwrap().offset(), 48);
}

#[test]
fn rejects_unsupported_aggregate_slot_alignment() {
    let error = FrameLayout::for_slot_counts_with_aggregate_slots(
        0,
        0,
        &[AggregateSlotRequest::new(0, ValueLayout::new(8, 32))],
    )
    .unwrap_err();

    assert_eq!(error[0].code, "E9005");
}

#[test]
fn plans_aggregate_slot_requests_from_ir_instructions() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Void,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(24, 8),
            },
            Instruction::Return,
        ],
    };

    let frame = plan_function_frame(&function).unwrap();
    let FunctionFrame::Framed(layout) = frame else {
        panic!("aggregate slot reservation should require a frame");
    };

    assert_eq!(layout.frame_size(), 32);
    assert_eq!(layout.saved_x30_offset(), 24);
    assert_eq!(
        layout.aggregate_slots(),
        &[AggregateSlot {
            slot_index: 0,
            offset: 0,
            size: 24,
            align: 8,
        }]
    );
}

#[test]
fn deduplicates_aggregate_slot_requests_from_nested_control_flow() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Fallible(Box::new(Type::Void)),
        instructions: vec![
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                }],
                else_instructions: vec![Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                }],
            },
            Instruction::CheckFailure {
                failure_mode: OutcomeFailureMode::Catch {
                    code: StrLocation::Local(0),
                    message: StrLocation::Local(2),
                    instructions: vec![Instruction::ReserveAggregateSlot {
                        slot_index: 1,
                        layout: ValueLayout::new(16, 16),
                    }],
                    recovers: false,
                },
            },
            Instruction::Return,
        ],
    };

    let frame = plan_function_frame(&function).unwrap();
    let FunctionFrame::Framed(layout) = frame else {
        panic!("aggregate slot reservation should require a frame");
    };

    assert_eq!(
        layout.aggregate_slots(),
        &[
            AggregateSlot {
                slot_index: 0,
                offset: 32,
                size: 24,
                align: 8,
            },
            AggregateSlot {
                slot_index: 1,
                offset: 64,
                size: 16,
                align: 16,
            },
        ]
    );
}

#[test]
fn plans_frame_slots_from_while_condition_and_body() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Void,
        instructions: vec![
            Instruction::While {
                condition_instructions: vec![Instruction::CallBool {
                    destination: BoolLocation::Local(2),
                    target: CallTarget::same_file("ready"),
                    arguments: vec![],
                }],
                condition: BoolValue::Location(BoolLocation::Local(2)),
                body_instructions: vec![Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(8, 8),
                }],
            },
            Instruction::Return,
        ],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(
            FrameLayout::for_slot_counts_with_aggregate_slots(
                3,
                0,
                &[AggregateSlotRequest::new(0, ValueLayout::new(8, 8))]
            )
            .unwrap()
        )
    );
}

#[test]
fn aggregate_call_requires_frame_and_counts_argument_slots() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Void,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(24, 8),
            },
            Instruction::CallAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make"),
                arguments: vec![ScalarArgument::I32(I32Value::Location(I32Location::Local(
                    1,
                )))],
            },
            Instruction::Return,
        ],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(
            FrameLayout::for_slot_counts_with_aggregate_slots(
                2,
                1,
                &[AggregateSlotRequest::new(0, ValueLayout::new(24, 8))]
            )
            .unwrap()
        )
    );
}

#[test]
fn aggregate_value_arguments_count_abi_staging_slots() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Void,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(24, 8),
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: ValueLayout::new(16, 8),
            },
            Instruction::CallVoid {
                target: CallTarget::same_file("consume"),
                arguments: vec![
                    ScalarArgument::AggregateIndirect(AggregateArgument {
                        source: AggregateArgumentSource::Slot(0),
                    }),
                    ScalarArgument::AggregateDirect(DirectAggregateArgument {
                        source: AggregateArgumentSource::Slot(1),
                        layout: ValueLayout::new(16, 8),
                        words: 2,
                    }),
                ],
            },
            Instruction::Return,
        ],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(
            FrameLayout::for_slot_counts_with_aggregate_slots(
                0,
                3,
                &[
                    AggregateSlotRequest::new(0, ValueLayout::new(24, 8)),
                    AggregateSlotRequest::new(1, ValueLayout::new(16, 8)),
                ]
            )
            .unwrap()
        )
    );
}

#[test]
fn aggregate_return_store_does_not_require_frame() {
    let function = Function {
        name: "make".to_string(),
        target: crate::ir::CallTarget::same_file("make".to_string()),
        return_type: Type::Void,
        instructions: vec![
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Return,
                offset: 8,
                value: UsizeValue::Const(3),
            },
            Instruction::Return,
        ],
    };

    assert_eq!(
        plan_function_frame(&function).unwrap(),
        FunctionFrame::Frameless
    );
}

#[test]
fn aggregate_slot_store_requires_frame_and_counts_value_locals() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Void,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(24, 8),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(0),
                offset: 8,
                value: UsizeValue::Location(UsizeLocation::Local(1)),
            },
            Instruction::Return,
        ],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(
            FrameLayout::for_slot_counts_with_aggregate_slots(
                2,
                0,
                &[AggregateSlotRequest::new(0, ValueLayout::new(24, 8))]
            )
            .unwrap()
        )
    );
}

#[test]
fn aggregate_slot_load_requires_frame_and_counts_destination_local() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Void,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(16, 8),
            },
            Instruction::LoadAggregateI32 {
                destination: I32Location::Local(2),
                source: AggregateLocation::Slot(0),
                offset: 4,
            },
            Instruction::Return,
        ],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(
            FrameLayout::for_slot_counts_with_aggregate_slots(
                3,
                0,
                &[AggregateSlotRequest::new(0, ValueLayout::new(16, 8))]
            )
            .unwrap()
        )
    );
}

#[test]
fn aggregate_copy_requires_frame_and_reserves_slots() {
    let function = Function {
        name: "forward".to_string(),
        target: crate::ir::CallTarget::same_file("forward".to_string()),
        return_type: Type::Aggregate {
            layout: ValueLayout::new(24, 8),
        },
        instructions: vec![
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(1),
                source: AggregateLocation::Slot(0),
                layout: ValueLayout::new(24, 8),
            },
            Instruction::Return,
        ],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(
            FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
                0,
                0,
                &[],
                &[
                    AggregateSlotRequest::new(1, ValueLayout::new(24, 8)),
                    AggregateSlotRequest::new(0, ValueLayout::new(24, 8)),
                ],
                true,
            )
            .unwrap()
        )
    );
}

#[test]
fn aggregate_range_copy_uses_explicit_slot_reservations() {
    let function = Function {
        name: "copy_header".to_string(),
        target: crate::ir::CallTarget::same_file("copy_header".to_string()),
        return_type: Type::Void,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(32, 8),
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: ValueLayout::new(16, 8),
            },
            Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(1),
                destination_offset: 0,
                source: AggregateLocation::Slot(0),
                source_offset: 8,
                layout: ValueLayout::new(16, 8),
            },
            Instruction::Return,
        ],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(
            FrameLayout::for_slot_counts_with_aggregate_slots(
                0,
                0,
                &[
                    AggregateSlotRequest::new(0, ValueLayout::new(32, 8)),
                    AggregateSlotRequest::new(1, ValueLayout::new(16, 8)),
                ]
            )
            .unwrap()
        )
    );
}

#[test]
fn aggregate_range_copy_from_direct_parameter_after_call_reserves_parameter_spill_slots() {
    let function = Function {
        name: "identity".to_string(),
        target: crate::ir::CallTarget::same_file("identity".to_string()),
        return_type: Type::DirectAggregate {
            layout: ValueLayout::new(9, 1),
            words: 2,
        },
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
                layout: ValueLayout::new(9, 1),
            },
            Instruction::Return,
        ],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(
            FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
                0,
                0,
                &[8, 9],
                &[],
                false
            )
            .unwrap()
        )
    );
}

#[test]
fn aggregate_range_copy_to_borrowed_parameter_after_call_reserves_parameter_spill_slot() {
    let function = Function {
        name: "set_header".to_string(),
        target: crate::ir::CallTarget::same_file("set_header".to_string()),
        return_type: Type::Void,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(16, 8),
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
                layout: ValueLayout::new(16, 8),
            },
            Instruction::Return,
        ],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(
            FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
                0,
                0,
                &[0],
                &[AggregateSlotRequest::new(0, ValueLayout::new(16, 8))],
                false
            )
            .unwrap()
        )
    );
}

#[test]
fn counts_scalar_slots_from_nested_i32_and_bool_locals() {
    let instructions = vec![Instruction::If {
        condition: BoolValue::BoolComparison {
            operator: BoolComparisonOperator::Equal,
            left: Box::new(BoolValue::Location(BoolLocation::Local(1))),
            right: Box::new(BoolValue::Const(true)),
        },
        then_instructions: vec![Instruction::I32Binary {
            operator: IntegerBinaryOperator::Add,
            destination: I32Location::Local(3),
            left: I32Value::Location(I32Location::Local(0)),
            right: I32Value::Const(1),
        }],
        else_instructions: vec![Instruction::SetBool {
            destination: BoolLocation::Local(2),
            value: BoolValue::Const(false),
        }],
    }];

    assert_eq!(scalar_spill_slot_count(&instructions), 4);
}

#[test]
fn call_i32_requires_frame_and_counts_destination_and_argument_locals() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![Instruction::CallI32 {
            destination: I32Location::Local(2),
            target: CallTarget::same_file("answer"),
            arguments: vec![ScalarArgument::I32(I32Value::Location(I32Location::Local(
                1,
            )))],
        }],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(FrameLayout::for_slot_counts(3, 1).unwrap())
    );
}

#[test]
fn call_with_scalar_parameter_borrow_reserves_parameter_spill_slot() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![Instruction::CallI32 {
            destination: I32Location::Return,
            target: CallTarget::same_file("inspect"),
            arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                source: BorrowSource::I32(I32Location::Parameter(8)),
            })],
        }],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(
            FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
                0,
                1,
                &[8],
                &[],
                false
            )
            .unwrap()
        )
    );
}

#[test]
fn store_to_borrowed_aggregate_parameter_after_call_reserves_parameter_spill_slot() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
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
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(
            FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
                0,
                0,
                &[0],
                &[],
                false
            )
            .unwrap()
        )
    );
}

#[test]
fn direct_aggregate_parameter_field_load_after_call_reserves_parameter_spill_slot() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
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
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(
            FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
                0,
                0,
                &[0],
                &[],
                false
            )
            .unwrap()
        )
    );
}

#[test]
fn normal_call_function_spills_parameter_values_used_later() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::CallI32 {
                destination: I32Location::Local(0),
                target: CallTarget::same_file("effect"),
                arguments: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: I32Value::Location(I32Location::Parameter(0)),
            },
            Instruction::Return,
        ],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(
            FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
                1,
                0,
                &[0],
                &[],
                false
            )
            .unwrap()
        )
    );
}

#[test]
fn call_bool_requires_frame_and_counts_destination_and_argument_locals() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![Instruction::CallBool {
            destination: BoolLocation::Local(2),
            target: CallTarget::same_file("ready"),
            arguments: vec![ScalarArgument::I32(I32Value::Location(I32Location::Local(
                1,
            )))],
        }],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(FrameLayout::for_slot_counts(3, 1).unwrap())
    );
}

#[test]
fn call_fallible_i32_requires_frame_and_counts_destination_and_argument_locals() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Fallible(Box::new(Type::I32)),
        instructions: vec![Instruction::CallOutcomeI32 {
            destination: I32Location::Local(2),
            target: CallTarget::same_file("answer"),
            arguments: vec![ScalarArgument::I32(I32Value::Location(I32Location::Local(
                1,
            )))],
            failure_mode: OutcomeFailureMode::Propagate,
        }],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(FrameLayout::for_slot_counts(3, 1).unwrap())
    );
}

#[test]
fn call_void_requires_frame_and_counts_argument_locals() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![Instruction::CallVoid {
            target: CallTarget::same_file("effect"),
            arguments: vec![ScalarArgument::I32(I32Value::Location(I32Location::Local(
                1,
            )))],
        }],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(FrameLayout::for_slot_counts(2, 1).unwrap())
    );
}

#[test]
fn call_fallible_void_requires_frame_and_counts_argument_locals() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Fallible(Box::new(Type::Void)),
        instructions: vec![Instruction::CallOutcomeVoid {
            target: CallTarget::same_file("effect"),
            arguments: vec![ScalarArgument::I32(I32Value::Location(I32Location::Local(
                1,
            )))],
            failure_mode: OutcomeFailureMode::Propagate,
        }],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(FrameLayout::for_slot_counts(2, 1).unwrap())
    );
}

#[test]
fn tail_call_with_arguments_requires_frame_and_argument_staging_slots() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![Instruction::TailCall {
            target: CallTarget::same_file("answer"),
            arguments: vec![
                ScalarArgument::I32(I32Value::Const(40)),
                ScalarArgument::I32(I32Value::Const(2)),
            ],
        }],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(FrameLayout::for_slot_counts(0, 2).unwrap())
    );
}

#[test]
fn tail_call_with_str_argument_counts_two_argument_staging_slots() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![Instruction::TailCall {
            target: CallTarget::same_file("answer"),
            arguments: vec![
                ScalarArgument::Str(StrValue::StaticBytes(b"Nocter".to_vec())),
                ScalarArgument::I32(I32Value::Const(42)),
            ],
        }],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(FrameLayout::for_slot_counts(0, 3).unwrap())
    );
}

#[test]
fn tail_call_with_slice_argument_counts_two_argument_staging_slots() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![Instruction::TailCall {
            target: CallTarget::same_file("answer"),
            arguments: vec![
                ScalarArgument::Slice(SliceValue::Location(SliceLocation::Parameter(0))),
                ScalarArgument::I32(I32Value::Const(42)),
            ],
        }],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(FrameLayout::for_slot_counts(0, 3).unwrap())
    );
}

#[test]
fn tail_call_with_local_argument_counts_argument_local() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![Instruction::TailCall {
            target: CallTarget::same_file("answer"),
            arguments: vec![ScalarArgument::I32(I32Value::Location(I32Location::Local(
                2,
            )))],
        }],
    };

    let frame = plan_function_frame(&function).unwrap();

    assert_eq!(
        frame,
        FunctionFrame::Framed(FrameLayout::for_slot_counts(3, 1).unwrap())
    );
}

#[test]
fn rejects_frame_when_w_spill_offset_is_not_encodable() {
    let error = FrameLayout::for_slot_counts(4097, 0).unwrap_err();

    assert_eq!(error[0].code, "E9005");
}
