use super::*;

#[test]
fn lowers_direct_payload_enum_value_argument() {
    let aggregate_type = Type::DirectAggregate {
        layout: ValueLayout::new(8, 4),
        words: 1,
    };
    let function = lower_named_function_with_signatures(
        r#"enum Result {
    ok(value: i32)
    failed
}

func accept(result: Result): i32 {
    return 1
}

func make_ok(): Result {
    return Result.ok(20)
}

func main(): i32 {
    let local = Result.ok(10)
    let returned = make_ok()
    return accept(move local) + accept(move returned)
}
"#,
        "main",
        function_signatures(vec![
            ("accept", Type::I32, vec![aggregate_type.clone()]),
            ("make_ok", aggregate_type.clone(), vec![]),
        ]),
    )
    .unwrap();

    assert_eq!(
        function.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(8, 4),
            },
            Instruction::StoreAggregateU8 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: u8_const(0),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 4,
                value: i32_const(10),
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: ValueLayout::new(8, 4),
            },
            Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("make_ok"),
                arguments: vec![],
                layout: ValueLayout::new(8, 4),
            },
            Instruction::CallI32 {
                destination: I32Location::Local(0),
                target: CallTarget::same_file("accept"),
                arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(0),
                    layout: ValueLayout::new(8, 4),
                    words: 1,
                })],
            },
            Instruction::CallI32 {
                destination: I32Location::Local(1),
                target: CallTarget::same_file("accept"),
                arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(1),
                    layout: ValueLayout::new(8, 4),
                    words: 1,
                })],
            },
            Instruction::AddI32 {
                destination: I32Location::Return,
                left: i32_local(0),
                right: i32_local(1),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_payload_enum_copy_aggregate_payload_binding() {
    let ir = lower_text(
        r#"copy struct Detail {
    code: i32
    bonus: i32
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    let result = Result.ok(Detail { code: 42, bonus: 1 })
    if result is Result.ok(value) {
        return value.code
    }

    return 0
}
"#,
    );

    let main = &ir.functions[0];
    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::If {
                    then_instructions,
                    ..
                } if then_instructions.contains(&Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(8, 4),
                }) && then_instructions.contains(&Instruction::CopyAggregateRange {
                    destination: AggregateLocation::Slot(1),
                    destination_offset: 0,
                    source: AggregateLocation::Slot(0),
                    source_offset: 4,
                    layout: ValueLayout::new(8, 4),
                }) && then_instructions.contains(&Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::Slot(1),
                    offset: 0,
                })
            )
        }),
        "{main:?}"
    );
}

#[test]
fn lowers_owned_direct_drop_payload_binding_with_conditional_target_cleanup() {
    let ir = lower_text(
        r#"struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    let result = Result.ok(Payload { code: 42 })
    if move result is Result.ok(value) {
        let code = value.code
    }
    return 0
}
"#,
    );

    let main = &ir.functions[0];
    assert!(
        main.instructions.contains(&Instruction::SetBool {
            destination: BoolLocation::Local(0),
            value: BoolValue::Const(true),
        }),
        "{main:?}"
    );

    let pattern_branch = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::If {
                then_instructions, ..
            } if then_instructions.contains(&Instruction::SetBool {
                destination: BoolLocation::Local(0),
                value: BoolValue::Const(false),
            }) =>
            {
                Some(then_instructions)
            }
            _ => None,
        })
        .expect("expected move-binding pattern branch");
    assert!(pattern_branch.contains(&Instruction::CopyAggregateRange {
        destination: AggregateLocation::Slot(2),
        destination_offset: 0,
        source: AggregateLocation::Slot(1),
        source_offset: 4,
        layout: ValueLayout::new(4, 4),
    }));
    assert!(pattern_branch.contains(&Instruction::CallVoid {
        target: CallTarget::same_file("Payload.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(2),
        })],
    }));

    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(0)),
                    then_instructions,
                    else_instructions,
                } if else_instructions.is_empty()
                    && then_instructions.iter().any(|instruction| matches!(
                        instruction,
                        Instruction::LoadAggregateU8 {
                            source: AggregateLocation::Slot(1),
                            offset: 0,
                            ..
                        }
                    ))
            )
        }),
        "{main:?}"
    );
}

#[test]
fn lowers_payload_enum_slice_payload_binding() {
    let score = lower_named_function(
        r#"enum Result {
    ok(value: &[u8])
    failed
}

func main(): void {
    return
}

func score(bytes: &[u8]): usize {
    let result = Result.ok(bytes)
    if result is Result.ok(value) {
        return value.len()
    }

    return 0
}
"#,
        "score",
    );

    assert!(
        score.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::If {
                    then_instructions,
                    ..
                } if then_instructions.contains(&Instruction::LoadAggregateUsize {
                    destination: UsizeLocation::Local(1),
                    source: AggregateLocation::Slot(0),
                    offset: 8,
                }) && then_instructions.contains(&Instruction::LoadAggregateUsize {
                    destination: UsizeLocation::Local(2),
                    source: AggregateLocation::Slot(0),
                    offset: 16,
                }) && then_instructions.contains(&Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::SliceLen(SliceLocation::Local(1)),
                })
            )
        }),
        "{score:?}"
    );
}

#[test]
fn lowers_scope_end_drop_for_active_payload_enum_payload() {
    let ir = lower_text(
        r#"struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    let result = Result.ok(Payload { code: 42 })
    return 0
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: CallTarget::same_file("main"),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(8, 4),
                    },
                    Instruction::StoreAggregateU8 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: u8_const(0),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 4,
                        value: i32_const(42),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(0),
                    },
                    Instruction::LoadAggregateU8 {
                        destination: U8Location::Local(1),
                        source: AggregateLocation::Slot(0),
                        offset: 0,
                    },
                    Instruction::If {
                        condition: BoolValue::I32Comparison {
                            operator: I32ComparisonOperator::Equal,
                            left: I32Value::U8ZeroExtend(Box::new(U8Value::Location(
                                U8Location::Local(1),
                            ))),
                            right: I32Value::U8ZeroExtend(Box::new(u8_const(0))),
                        },
                        then_instructions: vec![Instruction::CallVoid {
                            target: CallTarget::same_file("Payload.drop"),
                            arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                                source: BorrowSource::AggregateSlotField {
                                    slot_index: 0,
                                    offset: 4,
                                },
                            })],
                        }],
                        else_instructions: Vec::new(),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "Payload.drop".to_string(),
                target: CallTarget::same_file("Payload.drop"),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_scope_end_drop_for_multi_field_active_payload_enum_payload() {
    let ir = lower_text(
        r#"struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(first: Payload, second: Payload)
    failed
}

func main(): i32 {
    let result = Result.ok(Payload { code: 10 }, Payload { code: 20 })
    return 0
}
"#,
    );
    let main = &ir.functions[0];

    let drop_then_instructions = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::If {
                then_instructions, ..
            } if !then_instructions.is_empty()
                && then_instructions.iter().all(|then_instruction| {
                    matches!(
                        then_instruction,
                        Instruction::CallVoid {
                            target,
                            ..
                        } if target == &CallTarget::same_file("Payload.drop")
                    )
                }) =>
            {
                Some(then_instructions)
            }
            _ => None,
        })
        .expect("expected active payload drop branch");

    assert_eq!(
        drop_then_instructions,
        &vec![
            Instruction::CallVoid {
                target: CallTarget::same_file("Payload.drop"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlotField {
                        slot_index: 0,
                        offset: 8,
                    },
                })],
            },
            Instruction::CallVoid {
                target: CallTarget::same_file("Payload.drop"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlotField {
                        slot_index: 0,
                        offset: 4,
                    },
                })],
            },
        ]
    );
}

#[test]
fn lowers_scope_end_drop_for_inactive_payload_enum_payload() {
    let ir = lower_text(
        r#"struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    let result = Result.failed
    return 42
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: CallTarget::same_file("main"),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(8, 4),
                    },
                    Instruction::StoreAggregateU8 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: u8_const(1),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(42),
                    },
                    Instruction::LoadAggregateU8 {
                        destination: U8Location::Local(1),
                        source: AggregateLocation::Slot(0),
                        offset: 0,
                    },
                    Instruction::If {
                        condition: BoolValue::I32Comparison {
                            operator: I32ComparisonOperator::Equal,
                            left: I32Value::U8ZeroExtend(Box::new(U8Value::Location(
                                U8Location::Local(1),
                            ))),
                            right: I32Value::U8ZeroExtend(Box::new(u8_const(0))),
                        },
                        then_instructions: vec![Instruction::CallVoid {
                            target: CallTarget::same_file("Payload.drop"),
                            arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                                source: BorrowSource::AggregateSlotField {
                                    slot_index: 0,
                                    offset: 4,
                                },
                            })],
                        }],
                        else_instructions: Vec::new(),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "Payload.drop".to_string(),
                target: CallTarget::same_file("Payload.drop"),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_wildcard_only_payloadless_match_statement_without_branch() {
    let ir = lower_text(
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.no
    match choice {
        _ {
            return 7
        }
    }
}
"#,
    );

    let instructions = &ir.functions[0].instructions;
    assert!(
        instructions
            .iter()
            .all(|instruction| !matches!(instruction, Instruction::If { .. })),
        "{instructions:?}"
    );
    assert!(
        instructions.contains(&set_return_i32(7)),
        "{instructions:?}"
    );
}

#[test]
fn lowers_wildcard_only_payloadless_match_expression_without_branch() {
    let ir = lower_text(
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    let result = match choice {
        _ {
            7
        }
    }
    return result
}
"#,
    );

    let instructions = &ir.functions[0].instructions;
    assert!(
        instructions
            .iter()
            .all(|instruction| !matches!(instruction, Instruction::If { .. })),
        "{instructions:?}"
    );
    assert!(
        instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::SetI32 {
                destination: I32Location::Local(_),
                value: I32Value::Const(7)
            }
        )),
        "{instructions:?}"
    );
}
