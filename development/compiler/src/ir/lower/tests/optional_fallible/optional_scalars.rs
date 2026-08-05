use super::*;

#[test]
fn specializes_propagated_generic_optional_call_from_payload_context() {
    let ir = lower_text(
        r#"func maybe<T>(value: T): T? {
    return value
}

func forward<T>(value: T): T? {
    return maybe(value)?
}

func main(): i32 {
    return forward(42) otherwise { return 0 }
}
"#,
    );

    let forward = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("forward<i32>"))
        .expect("expected specialized forwarding function");

    assert_eq!(forward.return_type, Type::Optional(Box::new(Type::I32)));
    assert!(
        forward.instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::CallOutcomeI32 {
                target,
                failure_mode: OutcomeFailureMode::Propagate,
                ..
            } if target == &CallTarget::same_file("maybe<i32>")
        )),
        "{forward:?}"
    );
    assert!(
        ir.functions
            .iter()
            .all(|function| function.target != CallTarget::same_file("maybe<i32?>"))
    );
}

#[test]
fn lowers_fixed_array_optional_otherwise_binding_and_return() {
    let source = r#"func main(): i32 {
    let fallback: [i32; 2] = [1, 2]
    let values: [i32; 2] = maybe_pair() otherwise { fallback }
    return values[0] + values[1]
}

func choose(): [i32; 2] {
    return maybe_pair() otherwise { [20, 22] }
}

func maybe_pair(): [i32; 2]? {
    return none
}
"#;
    let layout = ValueLayout::new(8, 4);
    let pair_type = Type::DirectAggregate { layout, words: 1 };

    let main = lower_named_function_with_signatures(
        source,
        "main",
        function_signatures(vec![(
            "maybe_pair",
            Type::Optional(Box::new(pair_type.clone())),
            vec![],
        )]),
    )
    .unwrap();

    let Some(Instruction::CallOutcomeDirectAggregate {
        destination,
        target,
        arguments,
        layout: call_layout,
        failure_mode: OutcomeFailureMode::Recover { instructions },
    }) = main
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::CallOutcomeDirectAggregate { .. }))
    else {
        panic!("{main:?}");
    };
    assert_eq!(*destination, AggregateLocation::Slot(1));
    assert_eq!(*target, CallTarget::same_file("maybe_pair"));
    assert!(arguments.is_empty());
    assert_eq!(*call_layout, layout);
    assert!(instructions.contains(&Instruction::CopyAggregateRange {
        destination: AggregateLocation::Slot(1),
        destination_offset: 0,
        source: AggregateLocation::Slot(0),
        source_offset: 0,
        layout,
    }));

    let choose = lower_named_function_with_signatures(
        source,
        "choose",
        function_signatures(vec![(
            "maybe_pair",
            Type::Optional(Box::new(pair_type)),
            vec![],
        )]),
    )
    .unwrap();

    assert!(
        choose.instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::CallOutcomeDirectAggregate {
                destination: AggregateLocation::DirectReturn,
                target,
                arguments,
                layout: call_layout,
                failure_mode: OutcomeFailureMode::Handle { instructions },
            } if *target == CallTarget::same_file("maybe_pair")
                && arguments.is_empty()
                && *call_layout == layout
                && instructions.contains(&Instruction::Return)
        )),
        "{choose:?}"
    );
}

#[test]
fn lowers_optional_i32_function_none_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func maybe_answer(): i32? {
    return none
}
"#,
        "maybe_answer",
    );

    assert_eq!(
        function,
        Function {
            name: "maybe_answer".to_string(),
            target: crate::ir::CallTarget::same_file("maybe_answer".to_string()),
            return_type: Type::Optional(Box::new(Type::I32)),
            instructions: vec![Instruction::ReturnOptionalNone],
        }
    );
}

#[test]
fn lowers_optional_alias_i32_function_none_return() {
    let function = lower_named_function(
        r#"type MaybeI32 = i32?

func main(): i32 {
    return 0
}

func maybe_answer(): MaybeI32 {
    return none
}
"#,
        "maybe_answer",
    );

    assert_eq!(
        function,
        Function {
            name: "maybe_answer".to_string(),
            target: crate::ir::CallTarget::same_file("maybe_answer".to_string()),
            return_type: Type::Optional(Box::new(Type::I32)),
            instructions: vec![Instruction::ReturnOptionalNone],
        }
    );
}

#[test]
fn lowers_optional_i32_function_success_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func maybe_answer(): i32? {
    return 42
}
"#,
        "maybe_answer",
    );

    assert_eq!(
        function,
        Function {
            name: "maybe_answer".to_string(),
            target: crate::ir::CallTarget::same_file("maybe_answer".to_string()),
            return_type: Type::Optional(Box::new(Type::I32)),
            instructions: vec![set_return_i32(42), Instruction::ReturnOutcomeSuccess],
        }
    );
}

#[test]
fn lowers_optional_i32_terminal_if_none_branch() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func maybe_answer(flag: bool): i32? {
    if flag {
        return 42
    } else {
        return none
    }
}
"#,
        "maybe_answer",
    );

    assert_eq!(
        function,
        Function {
            name: "maybe_answer".to_string(),
            target: crate::ir::CallTarget::same_file("maybe_answer".to_string()),
            return_type: Type::Optional(Box::new(Type::I32)),
            instructions: vec![Instruction::If {
                condition: BoolValue::Location(BoolLocation::Parameter(0)),
                then_instructions: vec![set_return_i32(42), Instruction::ReturnOutcomeSuccess],
                else_instructions: vec![Instruction::ReturnOptionalNone],
            }],
        }
    );
}

#[test]
fn lowers_optional_i32_return_propagation() {
    let ir = lower_text(
        r#"func main(): i32 {
    return value()!
}

func value(): i32? {
    return maybe_answer()?
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );
    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "value")
        .unwrap();

    assert_eq!(
        function,
        &Function {
            name: "value".to_string(),
            target: crate::ir::CallTarget::same_file("value".to_string()),
            return_type: Type::Optional(Box::new(Type::I32)),
            instructions: vec![
                Instruction::CallOutcomeI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("maybe_answer"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Propagate,
                },
                Instruction::ReturnOutcomeSuccess,
            ],
        }
    );
}

#[test]
fn lowers_optional_i32_otherwise_return_call_binding() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = maybe_answer() otherwise { return 1 }

    return value
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallOutcomeI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("maybe_answer"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Handle {
                        instructions: vec![set_return_i32(1), Instruction::Return],
                    },
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_local(0),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_optional_i32_otherwise_break_call_binding_inside_loop() {
    let ir = lower_text(
        r#"func main(): i32 {
    var total = 0
    loop {
        let value = maybe_answer(total) otherwise { break }
        total += value
    }
    return total
}

func maybe_answer(total: i32): i32? {
    return none
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();

    assert_eq!(
        main.instructions,
        vec![
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            Instruction::While {
                condition_instructions: vec![],
                condition: BoolValue::Const(true),
                body_instructions: vec![
                    Instruction::CallOutcomeI32 {
                        destination: I32Location::Local(1),
                        target: CallTarget::same_file("maybe_answer"),
                        arguments: vec![ScalarArgument::I32(i32_local(0))],
                        failure_mode: OutcomeFailureMode::Handle {
                            instructions: vec![Instruction::Break],
                        },
                    },
                    Instruction::AddI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(0),
                        right: i32_local(1),
                    },
                ],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_optional_i32_otherwise_continue_call_binding_inside_range_for() {
    let ir = lower_text(
        r#"func main(): i32 {
    var total = 0
    for index in 0..<4 {
        let value = only_even(index) otherwise { continue }
        total += value
    }
    return total
}

func only_even(index: i32): i32? {
    return none
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();

    assert_eq!(
        main.instructions,
        vec![
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            Instruction::SetI32 {
                destination: I32Location::Local(1),
                value: i32_const(0),
            },
            Instruction::SetI32 {
                destination: I32Location::Local(2),
                value: i32_const(4),
            },
            Instruction::While {
                condition_instructions: vec![],
                condition: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::Less,
                    left: i32_local(1),
                    right: i32_local(2),
                },
                body_instructions: vec![
                    Instruction::CallOutcomeI32 {
                        destination: I32Location::Local(3),
                        target: CallTarget::same_file("only_even"),
                        arguments: vec![ScalarArgument::I32(i32_local(1))],
                        failure_mode: OutcomeFailureMode::Handle {
                            instructions: vec![
                                Instruction::AddI32 {
                                    destination: I32Location::Local(1),
                                    left: i32_local(1),
                                    right: i32_const(1),
                                },
                                Instruction::Continue,
                            ],
                        },
                    },
                    Instruction::AddI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(0),
                        right: i32_local(3),
                    },
                    Instruction::AddI32 {
                        destination: I32Location::Local(1),
                        left: i32_local(1),
                        right: i32_const(1),
                    },
                ],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_optional_i32_otherwise_never_call_binding() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = maybe_answer() otherwise { abort() }

    return value
}

func maybe_answer(): i32? {
    return 42
}

func abort(): never {
    abort()
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallOutcomeI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("maybe_answer"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Handle {
                        instructions: vec![Instruction::TailCall {
                            target: CallTarget::same_file("abort"),
                            arguments: vec![],
                        }],
                    },
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_local(0),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_optional_i32_otherwise_never_call_without_scope_cleanup() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 3 }
    let value = maybe_answer() otherwise { abort() }

    return value
}

func maybe_answer(): i32? {
    return 42
}

func abort(): never {
    abort()
}
"#,
    );

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(4, 4),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: i32_const(3),
                },
                Instruction::CallOutcomeI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("maybe_answer"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Handle {
                        instructions: vec![Instruction::TailCall {
                            target: CallTarget::same_file("abort"),
                            arguments: vec![],
                        }],
                    },
                },
                Instruction::SetI32 {
                    destination: I32Location::Local(1),
                    value: i32_local(0),
                },
                drop_call,
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_local(1),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_optional_i32_otherwise_return() {
    let ir = lower_text(
        r#"func main(): i32 {
    return maybe_answer() otherwise { 7 }
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallOutcomeI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("maybe_answer"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Handle {
                        instructions: vec![set_return_i32(7), Instruction::Return],
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_optional_i32_otherwise_return_with_scope_cleanup() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    return choose()
}

func choose(): i32 {
    var file = File { fd: 3 }
    return maybe_answer() otherwise { 7 }
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let choose = ir
        .functions
        .iter()
        .find(|function| function.name == "choose")
        .unwrap();
    assert_eq!(
        choose.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(3),
            },
            Instruction::CallOutcomeI32 {
                destination: I32Location::Local(0),
                target: CallTarget::same_file("maybe_answer"),
                arguments: vec![],
                failure_mode: OutcomeFailureMode::Handle {
                    instructions: vec![
                        Instruction::SetI32 {
                            destination: I32Location::Local(0),
                            value: i32_const(7),
                        },
                        drop_call.clone(),
                        Instruction::SetI32 {
                            destination: I32Location::Return,
                            value: i32_local(0),
                        },
                        Instruction::Return,
                    ],
                },
            },
            drop_call,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_optional_scalar_otherwise_returns() {
    let source = r#"func main(): i32 {
    return 0
}

func use_byte(): u8 {
    return maybe_byte() otherwise { 7 }
}

func maybe_byte(): u8? {
    return 42
}

func use_size(): usize {
    return maybe_size() otherwise { 7 }
}

func maybe_size(): usize? {
    return 42
}

func use_flag(): bool {
    return maybe_flag() otherwise { true }
}

func maybe_flag(): bool? {
    return false
}

func use_text(): &str {
    return maybe_text() otherwise { "fallback" }
}

func maybe_text(): &str? {
    return "text"
}

func use_bytes(bytes: &[u8]): &[u8] {
    return maybe_bytes(bytes) otherwise { bytes }
}

func maybe_bytes(bytes: &[u8]): &[u8]? {
    return bytes
}
"#;
    let signatures = function_signatures(vec![
        ("maybe_byte", Type::Optional(Box::new(Type::U8)), vec![]),
        ("maybe_size", Type::Optional(Box::new(Type::Usize)), vec![]),
        ("maybe_flag", Type::Optional(Box::new(Type::Bool)), vec![]),
        ("maybe_text", Type::Optional(Box::new(Type::Str)), vec![]),
        (
            "maybe_bytes",
            Type::Optional(Box::new(Type::Slice {
                is_readwrite: false,
            })),
            vec![Type::Slice {
                is_readwrite: false,
            }],
        ),
    ]);

    let use_byte =
        lower_named_function_with_signatures(source, "use_byte", signatures.clone()).unwrap();
    assert_eq!(
        use_byte.instructions,
        vec![
            Instruction::CallOutcomeU8 {
                destination: U8Location::Return,
                target: CallTarget::same_file("maybe_byte"),
                arguments: vec![],
                failure_mode: OutcomeFailureMode::Handle {
                    instructions: vec![
                        Instruction::SetU8 {
                            destination: U8Location::Return,
                            value: U8Value::Const(7),
                        },
                        Instruction::Return,
                    ],
                },
            },
            Instruction::Return,
        ]
    );

    let use_size =
        lower_named_function_with_signatures(source, "use_size", signatures.clone()).unwrap();
    assert_eq!(
        use_size.instructions,
        vec![
            Instruction::CallOutcomeUsize {
                destination: UsizeLocation::Return,
                target: CallTarget::same_file("maybe_size"),
                arguments: vec![],
                failure_mode: OutcomeFailureMode::Handle {
                    instructions: vec![
                        Instruction::SetUsize {
                            destination: UsizeLocation::Return,
                            value: UsizeValue::Const(7),
                        },
                        Instruction::Return,
                    ],
                },
            },
            Instruction::Return,
        ]
    );

    let use_flag =
        lower_named_function_with_signatures(source, "use_flag", signatures.clone()).unwrap();
    assert_eq!(
        use_flag.instructions,
        vec![
            Instruction::CallOutcomeBool {
                destination: BoolLocation::Return,
                target: CallTarget::same_file("maybe_flag"),
                arguments: vec![],
                failure_mode: OutcomeFailureMode::Handle {
                    instructions: vec![
                        Instruction::SetBool {
                            destination: BoolLocation::Return,
                            value: BoolValue::Const(true),
                        },
                        Instruction::Return,
                    ],
                },
            },
            Instruction::Return,
        ]
    );

    let use_text =
        lower_named_function_with_signatures(source, "use_text", signatures.clone()).unwrap();
    assert_eq!(
        use_text.instructions,
        vec![
            Instruction::CallOutcomeStr {
                destination: StrLocation::Return,
                target: CallTarget::same_file("maybe_text"),
                arguments: vec![],
                failure_mode: OutcomeFailureMode::Handle {
                    instructions: vec![
                        Instruction::SetStr {
                            destination: StrLocation::Return,
                            value: str_static_value(b"fallback"),
                        },
                        Instruction::Return,
                    ],
                },
            },
            Instruction::Return,
        ]
    );

    let use_bytes = lower_named_function_with_signatures(source, "use_bytes", signatures).unwrap();
    assert_eq!(
        use_bytes.instructions,
        vec![
            Instruction::CallOutcomeSlice {
                destination: SliceLocation::Return,
                target: CallTarget::same_file("maybe_bytes"),
                arguments: vec![ScalarArgument::Slice(SliceValue::Location(
                    SliceLocation::Parameter(0),
                ))],
                failure_mode: OutcomeFailureMode::Handle {
                    instructions: vec![
                        Instruction::SetSlice {
                            destination: SliceLocation::Return,
                            value: SliceValue::Location(SliceLocation::Parameter(0)),
                        },
                        Instruction::Return,
                    ],
                },
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_optional_i32_otherwise_call_binding() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = maybe_answer() otherwise { 7 }
    return value
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallOutcomeI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("maybe_answer"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Recover {
                        instructions: vec![Instruction::SetI32 {
                            destination: I32Location::Local(0),
                            value: I32Value::Const(7),
                        }],
                    },
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_local(0),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_optional_scalar_otherwise_call_bindings() {
    let source = r#"func main(): i32 {
    return 0
}

func use_byte(): i32 {
    let value: u8 = maybe_byte() otherwise { 7 }
    return value as i32
}

func maybe_byte(): u8? {
    return 42
}

func use_size(): usize {
    let value = maybe_size() otherwise { 7 }
    return value
}

func maybe_size(): usize? {
    return 42
}

func use_flag(): i32 {
    let value = maybe_flag() otherwise { true }
    if value {
        return 42
    } else {
        return 1
    }
}

func maybe_flag(): bool? {
    return false
}

func use_text(): usize {
    let value = maybe_text() otherwise { "fallback" }
    return value.len()
}

func maybe_text(): &str? {
    return "text"
}

func use_bytes(bytes: &[u8]): usize {
    let value: &[u8] = maybe_bytes(bytes) otherwise { bytes }
    return value.len()
}

func maybe_bytes(bytes: &[u8]): &[u8]? {
    return bytes
}
"#;
    let signatures = function_signatures(vec![
        ("maybe_byte", Type::Optional(Box::new(Type::U8)), vec![]),
        ("maybe_size", Type::Optional(Box::new(Type::Usize)), vec![]),
        ("maybe_flag", Type::Optional(Box::new(Type::Bool)), vec![]),
        ("maybe_text", Type::Optional(Box::new(Type::Str)), vec![]),
        (
            "maybe_bytes",
            Type::Optional(Box::new(Type::Slice {
                is_readwrite: false,
            })),
            vec![Type::Slice {
                is_readwrite: false,
            }],
        ),
    ]);

    let use_byte =
        lower_named_function_with_signatures(source, "use_byte", signatures.clone()).unwrap();
    assert_eq!(
        use_byte.instructions[0],
        Instruction::CallOutcomeU8 {
            destination: U8Location::Local(0),
            target: CallTarget::same_file("maybe_byte"),
            arguments: vec![],
            failure_mode: OutcomeFailureMode::Recover {
                instructions: vec![Instruction::SetU8 {
                    destination: U8Location::Local(0),
                    value: U8Value::Const(7),
                }],
            },
        }
    );

    let use_size =
        lower_named_function_with_signatures(source, "use_size", signatures.clone()).unwrap();
    assert_eq!(
        use_size.instructions[0],
        Instruction::CallOutcomeUsize {
            destination: UsizeLocation::Local(0),
            target: CallTarget::same_file("maybe_size"),
            arguments: vec![],
            failure_mode: OutcomeFailureMode::Recover {
                instructions: vec![Instruction::SetUsize {
                    destination: UsizeLocation::Local(0),
                    value: UsizeValue::Const(7),
                }],
            },
        }
    );

    let use_flag =
        lower_named_function_with_signatures(source, "use_flag", signatures.clone()).unwrap();
    assert_eq!(
        use_flag.instructions[0],
        Instruction::CallOutcomeBool {
            destination: BoolLocation::Local(0),
            target: CallTarget::same_file("maybe_flag"),
            arguments: vec![],
            failure_mode: OutcomeFailureMode::Recover {
                instructions: vec![Instruction::SetBool {
                    destination: BoolLocation::Local(0),
                    value: BoolValue::Const(true),
                }],
            },
        }
    );

    let use_text =
        lower_named_function_with_signatures(source, "use_text", signatures.clone()).unwrap();
    assert_eq!(
        use_text.instructions[0],
        Instruction::CallOutcomeStr {
            destination: StrLocation::Local(0),
            target: CallTarget::same_file("maybe_text"),
            arguments: vec![],
            failure_mode: OutcomeFailureMode::Recover {
                instructions: vec![Instruction::SetStr {
                    destination: StrLocation::Local(0),
                    value: str_static_value(b"fallback"),
                }],
            },
        }
    );

    let use_bytes = lower_named_function_with_signatures(source, "use_bytes", signatures).unwrap();
    assert_eq!(
        use_bytes.instructions[0],
        Instruction::CallOutcomeSlice {
            destination: SliceLocation::Local(0),
            target: CallTarget::same_file("maybe_bytes"),
            arguments: vec![ScalarArgument::Slice(SliceValue::Location(
                SliceLocation::Parameter(0),
            ))],
            failure_mode: OutcomeFailureMode::Recover {
                instructions: vec![Instruction::SetSlice {
                    destination: SliceLocation::Local(0),
                    value: SliceValue::Location(SliceLocation::Parameter(0)),
                }],
            },
        }
    );
}

#[test]
fn lowers_parenthesized_fallible_optional_none_return() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func value(): (i32?)! {
    return none
}
"#,
        "value",
        context::FunctionSignatures::new(HashMap::new()),
    )
    .unwrap();

    assert_eq!(
        function.return_type,
        Type::ComposedOutcome {
            outer: crate::outcomes::OutcomeLayer::Fallible,
            inner: crate::outcomes::OutcomeLayer::Optional,
            payload: Box::new(Type::I32),
        }
    );
    assert_eq!(function.instructions, vec![Instruction::ReturnOptionalNone]);
}
