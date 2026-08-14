use super::*;

#[test]
fn lowers_entry_usize_terminal_if_return() {
    let ir = lower_text(
        r#"func main(): usize {
    if true {
        return 7
    } else {
        return 9
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::If {
                    condition: BoolValue::Const(true),
                    then_instructions: vec![set_return_usize(7)],
                    else_instructions: vec![set_return_usize(9)],
                },
                Instruction::Return,
            ],
        }])
    );
}

#[test]
fn lowers_entry_usize_let_binding_then_usize_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value: usize = 42
    if value == 42 {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Local(0),
                    value: usize_const(42),
                },
                Instruction::If {
                    condition: BoolValue::UsizeComparison {
                        operator: I32ComparisonOperator::Equal,
                        left: usize_local(0),
                        right: usize_const(42),
                    },
                    then_instructions: vec![set_return_i32(0), Instruction::Return],
                    else_instructions: vec![set_return_i32(1), Instruction::Return],
                },
            ],
        }])
    );
}

#[test]
fn lowers_entry_usize_arithmetic_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    let left: usize = 20
    let right: usize = 6
    if left + right * 2 == 32 {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Local(0),
                    value: usize_const(20),
                },
                Instruction::SetUsize {
                    destination: UsizeLocation::Local(1),
                    value: usize_const(6),
                },
                Instruction::MultiplyUsize {
                    destination: UsizeLocation::Local(3),
                    left: usize_local(1),
                    right: usize_const(2),
                },
                Instruction::AddUsize {
                    destination: UsizeLocation::Local(2),
                    left: usize_local(0),
                    right: usize_local(3),
                },
                Instruction::If {
                    condition: BoolValue::UsizeComparison {
                        operator: I32ComparisonOperator::Equal,
                        left: usize_local(2),
                        right: usize_const(32),
                    },
                    then_instructions: vec![set_return_i32(0), Instruction::Return],
                    else_instructions: vec![set_return_i32(1), Instruction::Return],
                },
            ],
        }])
    );
}

#[test]
fn lowers_bool_parameter_normal_call_in_terminal_if() {
    let ir = lower_text(
        r#"func main(): i32 {
    if choose(7, true, 42) {
        return 0
    } else {
        return 1
    }
}

func choose(code: i32, flag: bool, size: usize): bool {
    return flag
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    call_bool(
                        BoolLocation::Local(0),
                        "choose",
                        vec![
                            ScalarArgument::I32(i32_const(7)),
                            ScalarArgument::Bool(BoolValue::Const(true)),
                            ScalarArgument::Usize(usize_const(42)),
                        ],
                    ),
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![set_return_i32(0)],
                        else_instructions: vec![set_return_i32(1)],
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "choose".to_string(),
                target: crate::ir::CallTarget::same_file("choose".to_string()),
                return_type: Type::Bool,
                instructions: vec![
                    Instruction::SetBool {
                        destination: BoolLocation::Return,
                        value: bool_param(1),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_assignment_to_nonterminal_if_branch_local_scalar() {
    let ir = lower_text(
        r#"func main(): i32 {
    if true {
        var value = 1
        value = 2
    }
    return 0
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
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(1),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(2),
                    },
                ],
                else_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_nonterminal_loop_before_return() {
    let ir = lower_text(
        r#"func main(): i32 {
    var value = 0
    loop {
        value = 42
        break
    }
    return value
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
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(42),
                    },
                    Instruction::Break,
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
fn lowers_nonterminal_i32_range_for_before_return() {
    let ir = lower_text(
        r#"func main(): i32 {
    var total = 0
    for value in 0..<4 {
        total = total + value
    }
    return total
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
                    Instruction::AddI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(0),
                        right: i32_local(1),
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
fn lowers_continue_inside_nonterminal_range_for_with_increment() {
    let ir = lower_text(
        r#"func main(): i32 {
    for value in 0..<4 {
        continue
    }
    return 0
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
                value: i32_const(4),
            },
            Instruction::While {
                condition_instructions: vec![],
                condition: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::Less,
                    left: i32_local(0),
                    right: i32_local(1),
                },
                body_instructions: vec![Instruction::AddI32 {
                    destination: I32Location::Local(0),
                    left: i32_local(0),
                    right: i32_const(1),
                }],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_terminal_loop_return() {
    let ir = lower_text(
        r#"func main(): i32 {
    loop {
        return 42
    }
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
        vec![Instruction::While {
            condition_instructions: vec![],
            condition: BoolValue::Const(true),
            body_instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_const(42),
                },
                Instruction::Return,
            ],
        }],
    );
}

#[test]
fn lowers_assignment_to_nonterminal_while_body_local_scalar() {
    let ir = lower_text(
        r#"func main(): i32 {
    while ready() {
        var value = 1
        value = 2
    }
    return 0
}

func ready(): bool {
    return false
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
            Instruction::While {
                condition_instructions: vec![call_bool(BoolLocation::Local(0), "ready", vec![],)],
                condition: BoolValue::Location(BoolLocation::Local(0)),
                body_instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Local(1),
                        value: i32_const(1),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Local(1),
                        value: i32_const(2),
                    },
                ],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_local_binding_reusing_range_for_name_after_loop() {
    let ir = lower_text(
        r#"func main(): i32 {
    for value in 0..<2 {
    }
    let value = 5
    return value
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
                value: i32_const(2),
            },
            Instruction::While {
                condition_instructions: vec![],
                condition: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::Less,
                    left: i32_local(0),
                    right: i32_local(1),
                },
                body_instructions: vec![Instruction::AddI32 {
                    destination: I32Location::Local(0),
                    left: i32_local(0),
                    right: i32_const(1),
                }],
            },
            Instruction::SetI32 {
                destination: I32Location::Local(2),
                value: i32_const(5),
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(2),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_outer_scalar_assignment_inside_nonterminal_while_body() {
    let ir = lower_text(
        r#"func main(): i32 {
    var value = 1
    while false {
        value = 2
    }
    return value
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
                value: i32_const(1),
            },
            Instruction::While {
                condition_instructions: vec![],
                condition: BoolValue::Const(false),
                body_instructions: vec![Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(2),
                }],
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
fn lowers_scalar_outcome_edges_inside_mir_while_body() {
    let ir = lower_text(
        r#"func main(): i32 {
    var value = 0
    while value < 1 {
        value = answer()!
    }
    return value
}

func answer(): i32! {
    return 1
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
                condition: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::Less,
                    left: i32_local(0),
                    right: i32_const(1),
                },
                body_instructions: vec![Instruction::CallOutcomeI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Trap,
                }],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_propagated_outcome_edges_inside_mir_while_condition() {
    let ir = lower_text(
        r#"func main(): i32! {
    while (ready()?) {
    }
    return 0
}

func ready(): bool! {
    return false
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
            Instruction::While {
                condition_instructions: vec![Instruction::CallOutcomeBool {
                    destination: BoolLocation::Local(0),
                    target: CallTarget::same_file("ready"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Propagate,
                }],
                condition: BoolValue::Location(BoolLocation::Local(0)),
                body_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::ReturnOutcomeSuccess,
        ]
    );
}

#[test]
fn lowers_outer_scalar_assignment_inside_nonterminal_if_branch() {
    let ir = lower_text(
        r#"func main(): i32 {
    var value = 1
    if true {
        value = 2
    }
    return value
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
                value: i32_const(1),
            },
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(2),
                }],
                else_instructions: vec![],
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
fn lowers_branch_void_call_before_terminal_if_return() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = File { fd: 3 }
    if true {
        touch(&+file)
        return 0
    } else {
        return 1
    }
}

func touch(file: &+File): void {
    return
}
"#,
    );

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let touch_call = Instruction::CallVoid {
        target: CallTarget::same_file("touch"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
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
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    touch_call,
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_const(0),
                    },
                ],
                else_instructions: vec![Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_const(1),
                }],
            },
            drop_call,
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_void_terminal_if_function() {
    let ir = lower_text(
        r#"func main(): i32 {
    run(true)
    return 0
}

func run(flag: bool): void {
    if flag {
        return
    } else {
        return
    }
}
"#,
    );

    let run = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert_eq!(
        run.instructions,
        vec![Instruction::If {
            condition: BoolValue::Location(BoolLocation::Parameter(0)),
            then_instructions: vec![Instruction::Return],
            else_instructions: vec![Instruction::Return],
        }],
    );
}

#[test]
fn lowers_entry_terminal_if_with_bool_literal_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    if false {
        return 1
    } else {
        return 2
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::If {
                    condition: BoolValue::Const(false),
                    then_instructions: vec![set_return_i32(1)],
                    else_instructions: vec![set_return_i32(2)],
                },
                Instruction::Return,
            ],
        }])
    );
}

#[test]
fn lowers_entry_terminal_if_with_bool_local_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    let enabled = true
    if enabled {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Local(0),
                    value: BoolValue::Const(true),
                },
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(0)),
                    then_instructions: vec![set_return_i32(0)],
                    else_instructions: vec![set_return_i32(1)],
                },
                Instruction::Return,
            ],
        }])
    );
}

#[test]
fn lowers_entry_terminal_if_with_mixed_i32_and_bool_locals() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = 42
    let enabled = true
    if enabled {
        return value
    } else {
        return 1
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(42),
                },
                Instruction::SetBool {
                    destination: BoolLocation::Local(1),
                    value: BoolValue::Const(true),
                },
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(1)),
                    then_instructions: vec![Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },],
                    else_instructions: vec![set_return_i32(1)],
                },
                Instruction::Return,
            ],
        }])
    );
}

#[test]
fn lowers_entry_bool_equality_binding() {
    let ir = lower_text(
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    let same = ready == blocked
    if same {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::SetBool {
                destination: BoolLocation::Local(0),
                value: BoolValue::Const(true),
            },
            Instruction::SetBool {
                destination: BoolLocation::Local(1),
                value: BoolValue::Const(false),
            },
            Instruction::SetBool {
                destination: BoolLocation::Local(2),
                value: BoolValue::BoolComparison {
                    operator: BoolComparisonOperator::Equal,
                    left: Box::new(BoolValue::Location(BoolLocation::Local(0))),
                    right: Box::new(BoolValue::Location(BoolLocation::Local(1))),
                },
            },
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(2)),
                then_instructions: vec![set_return_i32(1)],
                else_instructions: vec![set_return_i32(0)],
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_entry_terminal_if_returning_outer_local() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = 42
    if true {
        return value
    } else {
        return 0
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(42),
                },
                Instruction::If {
                    condition: BoolValue::Const(true),
                    then_instructions: vec![Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },],
                    else_instructions: vec![set_return_i32(0)],
                },
                Instruction::Return,
            ],
        }])
    );
}

#[test]
fn lowers_entry_terminal_if_with_i32_equality_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = 42
    if value == 42 {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(42),
                },
                Instruction::SetBool {
                    destination: BoolLocation::Local(1),
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: i32_local(0),
                        right: i32_const(42),
                    },
                },
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(1)),
                    then_instructions: vec![set_return_i32(0)],
                    else_instructions: vec![set_return_i32(1)],
                },
                Instruction::Return,
            ],
        }])
    );
}

#[test]
fn lowers_entry_terminal_if_with_i32_less_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = 41
    if value < 42 {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(41),
                },
                Instruction::SetBool {
                    destination: BoolLocation::Local(1),
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Less,
                        left: i32_local(0),
                        right: i32_const(42),
                    },
                },
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(1)),
                    then_instructions: vec![set_return_i32(0)],
                    else_instructions: vec![set_return_i32(1)],
                },
                Instruction::Return,
            ],
        }])
    );
}

#[test]
fn lowers_void_entry_trailing_if_before_implicit_return() {
    let ir = lower_text(
        r#"func main(): void {
    if true {
        effect()
    }
}

func effect(): void {
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::Void,
                instructions: vec![
                    Instruction::If {
                        condition: BoolValue::Const(true),
                        then_instructions: vec![call_void("effect", vec![])],
                        else_instructions: vec![],
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
        ])
    );
}

#[test]
fn lowers_void_function_trailing_if_before_implicit_return() {
    let ir = lower_text(
        r#"func main(): void {
    run(true)
}

func run(flag: bool): void {
    if flag {
        effect()
    }
}

func effect(): void {
}
"#,
    );

    assert_eq!(
        ir.functions[1],
        Function {
            name: "run".to_string(),
            target: crate::ir::CallTarget::same_file("run".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Parameter(0)),
                    then_instructions: vec![call_void("effect", vec![])],
                    else_instructions: vec![],
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_same_file_function_with_terminal_if() {
    let ir = lower_text(
        r#"func main(): i32 {
    return answer()
}

func answer(): i32 {
    if true {
        return 7
    } else {
        return 9
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::If {
                        condition: BoolValue::Const(true),
                        then_instructions: vec![set_return_i32(7)],
                        else_instructions: vec![set_return_i32(9)],
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_same_file_function_with_i32_inequality_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    return differs(40, 2)
}

func differs(left: i32, right: i32): i32 {
    if left != right {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("differs", vec![i32_const(40), i32_const(2)])],
            },
            Function {
                name: "differs".to_string(),
                target: crate::ir::CallTarget::same_file("differs".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetBool {
                        destination: BoolLocation::Local(0),
                        value: BoolValue::I32Comparison {
                            operator: I32ComparisonOperator::NotEqual,
                            left: i32_param(0),
                            right: i32_param(1),
                        },
                    },
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![set_return_i32(1)],
                        else_instructions: vec![set_return_i32(0)],
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_same_file_function_with_i32_greater_equal_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    return at_least(42, 40)
}

func at_least(left: i32, right: i32): i32 {
    if left >= right {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("at_least", vec![i32_const(42), i32_const(40)])],
            },
            Function {
                name: "at_least".to_string(),
                target: crate::ir::CallTarget::same_file("at_least".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetBool {
                        destination: BoolLocation::Local(0),
                        value: BoolValue::I32Comparison {
                            operator: I32ComparisonOperator::GreaterEqual,
                            left: i32_param(0),
                            right: i32_param(1),
                        },
                    },
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![set_return_i32(1)],
                        else_instructions: vec![set_return_i32(0)],
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_bool_returning_function_with_terminal_if() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func enabled(): bool {
    let ready = true
    if ready {
        return true
    } else {
        return false
    }
}
"#,
        "enabled",
    );

    assert_eq!(
        function,
        Function {
            name: "enabled".to_string(),
            target: crate::ir::CallTarget::same_file("enabled".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Local(0),
                    value: BoolValue::Const(true),
                },
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(0)),
                    then_instructions: vec![Instruction::SetBool {
                        destination: BoolLocation::Return,
                        value: BoolValue::Const(true),
                    },],
                    else_instructions: vec![Instruction::SetBool {
                        destination: BoolLocation::Return,
                        value: BoolValue::Const(false),
                    },],
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_bool_returning_function_tail_call() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func enabled(): bool {
    return true
}

func mirrors_enabled(): bool {
    return enabled()
}
"#,
        "mirrors_enabled",
    );

    assert_eq!(
        function,
        Function {
            name: "mirrors_enabled".to_string(),
            target: crate::ir::CallTarget::same_file("mirrors_enabled".to_string()),
            return_type: Type::Bool,
            instructions: vec![tail_call("enabled", vec![])],
        }
    );
}

#[test]
fn lowers_entry_bool_let_initializer_normal_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = ready()
    if value {
        return 0
    } else {
        return 1
    }
}

func ready(): bool {
    return true
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    call_bool(BoolLocation::Local(0), "ready", vec![]),
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![set_return_i32(0)],
                        else_instructions: vec![set_return_i32(1)],
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "ready".to_string(),
                target: crate::ir::CallTarget::same_file("ready".to_string()),
                return_type: Type::Bool,
                instructions: vec![
                    Instruction::SetBool {
                        destination: BoolLocation::Return,
                        value: BoolValue::Const(true),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_bool_return_not_normal_call() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func ready(): bool {
    return false
}

func disabled(): bool {
    return !ready()
}
"#,
        "disabled",
    );

    assert_eq!(
        function,
        Function {
            name: "disabled".to_string(),
            target: crate::ir::CallTarget::same_file("disabled".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                call_bool(BoolLocation::Local(0), "ready", vec![]),
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::Not(Box::new(BoolValue::Location(BoolLocation::Local(0)))),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_bool_let_initializer_normal_call_comparison() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = ready() == true
    if value {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return true
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_bool(BoolLocation::Local(1), "ready", vec![]),
            Instruction::SetBool {
                destination: BoolLocation::Local(0),
                value: BoolValue::BoolComparison {
                    operator: BoolComparisonOperator::Equal,
                    left: Box::new(BoolValue::Location(BoolLocation::Local(1))),
                    right: Box::new(BoolValue::Const(true)),
                },
            },
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![set_return_i32(42)],
                else_instructions: vec![set_return_i32(7)],
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_bool_return_normal_call_comparison() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func left(): bool {
    return true
}

func right(): bool {
    return false
}

func differs(): bool {
    return left() != right()
}
"#,
        "differs",
        context::FunctionSignatures::new(HashMap::from([
            ("left".to_string(), Type::Bool),
            ("right".to_string(), Type::Bool),
        ])),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "differs".to_string(),
            target: crate::ir::CallTarget::same_file("differs".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                call_bool(BoolLocation::Local(0), "left", vec![]),
                call_bool(BoolLocation::Local(1), "right", vec![]),
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::BoolComparison {
                        operator: BoolComparisonOperator::NotEqual,
                        left: Box::new(BoolValue::Location(BoolLocation::Local(0))),
                        right: Box::new(BoolValue::Location(BoolLocation::Local(1))),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_entry_i32_if_condition_normal_call_comparison() {
    let ir = lower_text(
        r#"func main(): i32 {
    if left() == right() {
        return 42
    } else {
        return 7
    }
}

func left(): bool {
    return true
}

func right(): bool {
    return true
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_bool(BoolLocation::Local(1), "left", vec![]),
            call_bool(BoolLocation::Local(2), "right", vec![]),
            Instruction::SetBool {
                destination: BoolLocation::Local(0),
                value: BoolValue::BoolComparison {
                    operator: BoolComparisonOperator::Equal,
                    left: Box::new(BoolValue::Location(BoolLocation::Local(1))),
                    right: Box::new(BoolValue::Location(BoolLocation::Local(2))),
                },
            },
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![set_return_i32(42)],
                else_instructions: vec![set_return_i32(7)],
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_entry_i32_if_condition_i32_normal_call_comparison() {
    let ir = lower_text(
        r#"func main(): i32 {
    if answer() == 42 {
        return 42
    } else {
        return 7
    }
}

func answer(): i32 {
    return 42
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_i32(I32Location::Local(1), "answer", vec![]),
            Instruction::SetBool {
                destination: BoolLocation::Local(0),
                value: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::Equal,
                    left: i32_local(1),
                    right: i32_const(42),
                },
            },
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![set_return_i32(42)],
                else_instructions: vec![set_return_i32(7)],
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_bool_let_initializer_i32_normal_call_comparison() {
    let ir = lower_text(
        r#"func main(): i32 {
    let matched = answer() <= limit()
    if matched {
        return 42
    } else {
        return 7
    }
}

func answer(): i32 {
    return 40
}

func limit(): i32 {
    return 42
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_i32(I32Location::Local(1), "answer", vec![]),
            call_i32(I32Location::Local(2), "limit", vec![]),
            Instruction::SetBool {
                destination: BoolLocation::Local(0),
                value: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::LessEqual,
                    left: i32_local(1),
                    right: i32_local(2),
                },
            },
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![set_return_i32(42)],
                else_instructions: vec![set_return_i32(7)],
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_bool_return_i32_normal_call_comparison() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func left(): i32 {
    return 40
}

func right(): i32 {
    return 42
}

func less(): bool {
    return left() < right()
}
"#,
        "less",
        context::FunctionSignatures::new(HashMap::from([
            ("left".to_string(), Type::I32),
            ("right".to_string(), Type::I32),
        ])),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "less".to_string(),
            target: crate::ir::CallTarget::same_file("less".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                call_i32(I32Location::Local(0), "left", vec![]),
                call_i32(I32Location::Local(1), "right", vec![]),
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Less,
                        left: i32_local(0),
                        right: i32_local(1),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_entry_i32_if_condition_normal_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    if ready() {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return true
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_bool(BoolLocation::Local(0), "ready", vec![]),
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![set_return_i32(42)],
                else_instructions: vec![set_return_i32(7)],
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_bool_if_condition_normal_call() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func ready(): bool {
    return true
}

func choose(): bool {
    if ready() {
        return false
    } else {
        return true
    }
}
"#,
        "choose",
    );

    assert_eq!(
        function,
        Function {
            name: "choose".to_string(),
            target: crate::ir::CallTarget::same_file("choose".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                call_bool(BoolLocation::Local(0), "ready", vec![]),
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(0)),
                    then_instructions: vec![Instruction::SetBool {
                        destination: BoolLocation::Return,
                        value: BoolValue::Const(false),
                    }],
                    else_instructions: vec![Instruction::SetBool {
                        destination: BoolLocation::Return,
                        value: BoolValue::Const(true),
                    }],
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_conditional_break_and_continue_edges_inside_mir_while() {
    let ir = lower_text(
        r#"func main(): i32 {
    var value = 0
    while value < 4 {
        value = value + 1
        if value == 2 {
            continue
        }
        if value == 3 {
            break
        }
    }
    return value
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            Instruction::While {
                condition_instructions: vec![],
                condition: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::Less,
                    left: i32_local(0),
                    right: i32_const(4),
                },
                body_instructions: vec![
                    Instruction::AddI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(0),
                        right: i32_const(1),
                    },
                    Instruction::SetBool {
                        destination: BoolLocation::Local(1),
                        value: BoolValue::I32Comparison {
                            operator: I32ComparisonOperator::Equal,
                            left: i32_local(0),
                            right: i32_const(2),
                        },
                    },
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(1)),
                        then_instructions: vec![Instruction::Continue],
                        else_instructions: vec![],
                    },
                    Instruction::SetBool {
                        destination: BoolLocation::Local(2),
                        value: BoolValue::I32Comparison {
                            operator: I32ComparisonOperator::Equal,
                            left: i32_local(0),
                            right: i32_const(3),
                        },
                    },
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(2)),
                        then_instructions: vec![Instruction::Break],
                        else_instructions: vec![],
                    },
                ],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ]
    );
}
