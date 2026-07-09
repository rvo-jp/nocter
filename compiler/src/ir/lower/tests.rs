use super::*;
use crate::analysis::{CompileUnit, analyze_compile_unit_with_entry};
use crate::diagnostics::Diagnostic;
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::ir::{
    BoolComparisonOperator, BoolLocation, BoolLogicalOperator, BoolValue, Function,
    I32ComparisonOperator, I32Location, I32Value, Instruction, IrModule, Type,
};
use crate::source::SourceMap;
use crate::target::DEFAULT_TARGET;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn lowers_entry_returning_i32_literal() {
    let ir = lower_text(
        r#"func main(): i32 {
    return 42
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            return_type: Type::I32,
            instructions: vec![set_return_i32(42), Instruction::Return],
        }])
    );
}

#[test]
fn lowers_entry_i32_let_binding_then_return() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = 42
    return value
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
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
        }])
    );
}

#[test]
fn lowers_entry_i32_let_initializer_normal_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = answer()
    return value
}

func answer(): i32 {
    return 42
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    call_i32(I32Location::Local(0), "answer", vec![]),
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "answer".to_string(),
                return_type: Type::I32,
                instructions: vec![set_return_i32(42), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_let_initializer_normal_call_with_arguments() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = add(20, 22)
    return value
}

func add(a: i32, b: i32): i32 {
    return a + b
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    call_i32(
                        I32Location::Local(0),
                        "add",
                        vec![i32_const(20), i32_const(22)]
                    ),
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "add".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_param(0),
                        right: i32_param(1),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_i32_let_initializer_normal_call_with_non_reordered_parameter_arguments() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func add(a: i32, b: i32): i32 {
    return a + b
}

func wrapper(a: i32, b: i32): i32 {
    let value = add(a, b)
    return value
}
"#,
        "wrapper",
        context::FunctionSignatures::new(HashMap::from([("add".to_string(), Type::I32)])),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "wrapper".to_string(),
            return_type: Type::I32,
            instructions: vec![
                call_i32(
                    I32Location::Local(0),
                    "add",
                    vec![i32_param(0), i32_param(1)]
                ),
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
fn lowers_entry_i32_return_expression_normal_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    return answer() + 1
}

func answer(): i32 {
    return 41
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    call_i32(I32Location::Local(0), "answer", vec![]),
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_local(0),
                        right: i32_const(1),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "answer".to_string(),
                return_type: Type::I32,
                instructions: vec![set_return_i32(41), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_let_initializer_normal_call_addition() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = answer() + 1
    return value
}

func answer(): i32 {
    return 41
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    call_i32(I32Location::Local(0), "answer", vec![]),
                    Instruction::AddI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(0),
                        right: i32_const(1),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "answer".to_string(),
                return_type: Type::I32,
                instructions: vec![set_return_i32(41), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_return_expression_local_plus_normal_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    let base = 5
    return base + answer()
}

func answer(): i32 {
    return 37
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(5),
                    },
                    call_i32(I32Location::Local(1), "answer", vec![]),
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_local(0),
                        right: i32_local(1),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "answer".to_string(),
                return_type: Type::I32,
                instructions: vec![set_return_i32(37), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_nested_return_addition_with_one_normal_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    return (answer() + 1) + 2
}

func answer(): i32 {
    return 39
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    call_i32(I32Location::Local(1), "answer", vec![]),
                    Instruction::AddI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(1),
                        right: i32_const(1),
                    },
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_local(0),
                        right: i32_const(2),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "answer".to_string(),
                return_type: Type::I32,
                instructions: vec![set_return_i32(39), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_annotated_let_binding_then_return() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value: i32 = 42
    return value
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(42),
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
fn lowers_entry_i32_local_addition_binding_then_return() {
    let ir = lower_text(
        r#"func main(): i32 {
    let base = 40
    let result = base + 2
    return result
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(40),
                },
                Instruction::AddI32 {
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
        }])
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
            return_type: Type::I32,
            instructions: vec![Instruction::If {
                condition: BoolValue::Const(false),
                then_instructions: vec![set_return_i32(1), Instruction::Return],
                else_instructions: vec![set_return_i32(2), Instruction::Return],
            }],
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
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Local(0),
                    value: BoolValue::Const(true),
                },
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(0)),
                    then_instructions: vec![set_return_i32(0), Instruction::Return],
                    else_instructions: vec![set_return_i32(1), Instruction::Return],
                },
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
                    then_instructions: vec![
                        Instruction::SetI32 {
                            destination: I32Location::Return,
                            value: i32_local(0),
                        },
                        Instruction::Return,
                    ],
                    else_instructions: vec![set_return_i32(1), Instruction::Return],
                },
            ],
        }])
    );
}

#[test]
fn lowers_entry_bool_not_binding() {
    let ir = lower_text(
        r#"func main(): i32 {
    let blocked = false
    let enabled = !blocked
    if enabled {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::SetBool {
                destination: BoolLocation::Local(0),
                value: BoolValue::Const(false),
            },
            Instruction::SetBool {
                destination: BoolLocation::Local(1),
                value: BoolValue::Not(Box::new(BoolValue::Location(BoolLocation::Local(0)))),
            },
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(1)),
                then_instructions: vec![set_return_i32(0), Instruction::Return],
                else_instructions: vec![set_return_i32(1), Instruction::Return],
            },
        ]
    );
}

#[test]
fn lowers_entry_terminal_if_with_bool_and_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    if ready && !blocked {
        return 0
    } else {
        return 1
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
            Instruction::If {
                condition: BoolValue::Logical {
                    operator: BoolLogicalOperator::And,
                    left: Box::new(BoolValue::Location(BoolLocation::Local(0))),
                    right: Box::new(BoolValue::Not(Box::new(BoolValue::Location(
                        BoolLocation::Local(1),
                    )))),
                },
                then_instructions: vec![set_return_i32(0), Instruction::Return],
                else_instructions: vec![set_return_i32(1), Instruction::Return],
            },
        ]
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
                then_instructions: vec![set_return_i32(1), Instruction::Return],
                else_instructions: vec![set_return_i32(0), Instruction::Return],
            },
        ]
    );
}

#[test]
fn reports_unsupported_bool_equality_over_unary_operand() {
    let diagnostics = lower_text_diagnostics(
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    let same = !ready == blocked
    if same {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8008");
    assert_eq!(
        diagnostics[0].message,
        "IR v0 can only lower bool equality/inequality operands that are bool literals or bool locals"
    );
}

#[test]
fn reports_unsupported_bool_equality_over_logical_operand() {
    let diagnostics = lower_text_diagnostics(
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    let same = (ready && !blocked) == ready
    if same {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8008");
    assert_eq!(
        diagnostics[0].message,
        "IR v0 can only lower bool equality/inequality operands that are bool literals or bool locals"
    );
}

#[test]
fn reports_unsupported_bool_equality_in_terminal_if_condition() {
    let diagnostics = lower_text_diagnostics(
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    if !ready == blocked {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8002");
    assert_eq!(
        diagnostics[0].message,
        "IR v0 can only lower bool equality/inequality operands that are bool literals or bool locals"
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
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(42),
                },
                Instruction::If {
                    condition: BoolValue::Const(true),
                    then_instructions: vec![
                        Instruction::SetI32 {
                            destination: I32Location::Return,
                            value: i32_local(0),
                        },
                        Instruction::Return,
                    ],
                    else_instructions: vec![set_return_i32(0), Instruction::Return],
                },
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
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(42),
                },
                Instruction::If {
                    condition: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: i32_local(0),
                        right: i32_const(42),
                    },
                    then_instructions: vec![set_return_i32(0), Instruction::Return],
                    else_instructions: vec![set_return_i32(1), Instruction::Return],
                },
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
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(41),
                },
                Instruction::If {
                    condition: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Less,
                        left: i32_local(0),
                        right: i32_const(42),
                    },
                    then_instructions: vec![set_return_i32(0), Instruction::Return],
                    else_instructions: vec![set_return_i32(1), Instruction::Return],
                },
            ],
        }])
    );
}

#[test]
fn lowers_configured_entry_name() {
    let ir = lower_text_with_entry(
        r#"func start(): i32 {
    return 9
}

func main(): i32 {
    return 0
}
"#,
        "start",
    );

    assert_eq!(ir.functions[0].name, "start");
    assert_eq!(
        ir.functions[0].instructions,
        vec![set_return_i32(9), Instruction::Return]
    );
}

#[test]
fn lowers_entry_returning_negative_i32_literal() {
    let ir = lower_text(
        r#"func main(): i32 {
    return -42
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![set_return_i32(-42), Instruction::Return]
    );
}

#[test]
fn lowers_fallible_entry_returning_i32_literal() {
    let ir = lower_text(
        r#"func main(): i32! {
    return 7
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![set_return_i32(7), Instruction::Return],
        }])
    );
}

#[test]
fn lowers_fallible_entry_return_make_error() {
    let ir = lower_text(
        r#"primitive make_error(code: str, message: str): error

func main(): i32! {
    return make_error("app.failed", "failed")
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::WriteStaticStderr(b"failed\n".to_vec()),
                set_return_i32(1),
                Instruction::Return,
            ],
        }])
    );
}

#[test]
fn lowers_fallible_entry_return_error_message_without_duplicate_newline() {
    let ir = lower_text(
        r#"primitive make_error(code: str, message: str): error

func main(): i32! {
    return make_error("app.failed", "failed\n")
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::WriteStaticStderr(b"failed\n".to_vec()),
            set_return_i32(1),
            Instruction::Return,
        ]
    );
}

#[test]
fn reports_unsupported_fail_payload() {
    let diagnostics = lower_text_diagnostics(
        r#"primitive make_error(code: str, message: str): error

func main(): i32! {
    return make_error("app.failed", dynamic())
}

func dynamic(): str {
    return "failed"
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8004");
}

#[test]
fn lowers_void_entry_with_empty_body() {
    let ir = lower_text(
        r#"func main(): void {
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        }])
    );
}

#[test]
fn lowers_entry_returning_same_file_function_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    return answer()
}

func answer(): i32 {
    return 7
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                return_type: Type::I32,
                instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_returning_i32_function_call_with_arguments() {
    let ir = lower_text(
        r#"func main(): i32 {
    return add(20, 22)
}

func add(a: i32, b: i32): i32 {
    return a + b
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![tail_call("add", vec![i32_const(20), i32_const(22)])],
            },
            Function {
                name: "add".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_param(0),
                        right: i32_param(1),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_same_file_function_with_i32_let_binding() {
    let ir = lower_text(
        r#"func main(): i32 {
    return answer()
}

func answer(): i32 {
    let value = 7
    return value
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(7),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_same_file_function_with_i32_local_addition() {
    let ir = lower_text(
        r#"func main(): i32 {
    return answer()
}

func answer(): i32 {
    let base = 40
    let result = base + 2
    return result
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(40),
                    },
                    Instruction::AddI32 {
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
            },
        ])
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
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                return_type: Type::I32,
                instructions: vec![Instruction::If {
                    condition: BoolValue::Const(true),
                    then_instructions: vec![set_return_i32(7), Instruction::Return],
                    else_instructions: vec![set_return_i32(9), Instruction::Return],
                }],
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
                return_type: Type::I32,
                instructions: vec![tail_call("differs", vec![i32_const(40), i32_const(2)])],
            },
            Function {
                name: "differs".to_string(),
                return_type: Type::I32,
                instructions: vec![Instruction::If {
                    condition: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::NotEqual,
                        left: i32_param(0),
                        right: i32_param(1),
                    },
                    then_instructions: vec![set_return_i32(1), Instruction::Return],
                    else_instructions: vec![set_return_i32(0), Instruction::Return],
                }],
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
                return_type: Type::I32,
                instructions: vec![tail_call("at_least", vec![i32_const(42), i32_const(40)])],
            },
            Function {
                name: "at_least".to_string(),
                return_type: Type::I32,
                instructions: vec![Instruction::If {
                    condition: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::GreaterEqual,
                        left: i32_param(0),
                        right: i32_param(1),
                    },
                    then_instructions: vec![set_return_i32(1), Instruction::Return],
                    else_instructions: vec![set_return_i32(0), Instruction::Return],
                }],
            },
        ])
    );
}

#[test]
fn lowers_bool_returning_function() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func enabled(): bool {
    let ready = true
    let blocked = false
    return ready && !blocked
}
"#,
        "enabled",
    );

    assert_eq!(
        function,
        Function {
            name: "enabled".to_string(),
            return_type: Type::Bool,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Local(0),
                    value: BoolValue::Const(true),
                },
                Instruction::SetBool {
                    destination: BoolLocation::Local(1),
                    value: BoolValue::Const(false),
                },
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::Logical {
                        operator: BoolLogicalOperator::And,
                        left: Box::new(BoolValue::Location(BoolLocation::Local(0))),
                        right: Box::new(BoolValue::Not(Box::new(BoolValue::Location(
                            BoolLocation::Local(1),
                        )))),
                    },
                },
                Instruction::Return,
            ],
        }
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
            return_type: Type::Bool,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Local(0),
                    value: BoolValue::Const(true),
                },
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(0)),
                    then_instructions: vec![
                        Instruction::SetBool {
                            destination: BoolLocation::Return,
                            value: BoolValue::Const(true),
                        },
                        Instruction::Return,
                    ],
                    else_instructions: vec![
                        Instruction::SetBool {
                            destination: BoolLocation::Return,
                            value: BoolValue::Const(false),
                        },
                        Instruction::Return,
                    ],
                },
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
            return_type: Type::Bool,
            instructions: vec![tail_call("enabled", vec![])],
        }
    );
}

#[test]
fn rejects_tail_call_return_type_mismatch_during_lowering() {
    let diagnostics = lower_named_function_diagnostics_with_signatures(
        r#"func main(): i32 {
    return 0
}

func enabled(): i32 {
    return 1
}

func mirrors_enabled(): i32 {
    return enabled()
}
"#,
        "mirrors_enabled",
        context::FunctionSignatures::new(HashMap::from([("enabled".to_string(), Type::Bool)])),
    );

    assert_eq!(diagnostics[0].code, "E8006");
    assert_eq!(
        diagnostics[0].message,
        "IR v0 cannot lower tail call from function `mirrors_enabled` returning `i32` to function `enabled` returning `bool`"
    );
}

#[test]
fn reports_unsupported_nested_i32_tail_call_argument() {
    let diagnostics = lower_text_diagnostics(
        r#"func main(): i32 {
    return add(answer(), 1)
}

func answer(): i32 {
    return 41
}

func add(a: i32, b: i32): i32 {
    return a + b
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8006");
    assert_eq!(
        diagnostics[0].message,
        "IR v0 can only lower function calls in direct tail return position"
    );
}

#[test]
fn lowers_entry_i32_let_initializer_nested_normal_call_argument() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = outer(inner())
    return value
}

func inner(): i32 {
    return 41
}

func outer(value: i32): i32 {
    return value + 1
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    call_i32(I32Location::Local(0), "inner", vec![]),
                    call_i32(I32Location::Local(0), "outer", vec![i32_local(0)]),
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "inner".to_string(),
                return_type: Type::I32,
                instructions: vec![set_return_i32(41), Instruction::Return],
            },
            Function {
                name: "outer".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_param(0),
                        right: i32_const(1),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_let_initializer_multiple_nested_normal_call_arguments() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = add(left(), right())
    return value
}

func left(): i32 {
    return 20
}

func right(): i32 {
    return 22
}

func add(a: i32, b: i32): i32 {
    return a + b
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_i32(I32Location::Local(0), "left", vec![]),
            call_i32(I32Location::Local(1), "right", vec![]),
            call_i32(
                I32Location::Local(0),
                "add",
                vec![i32_local(0), i32_local(1)]
            ),
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_entry_i32_return_addition_with_nested_normal_call_argument() {
    let ir = lower_text(
        r#"func main(): i32 {
    return outer(inner()) + 1
}

func inner(): i32 {
    return 40
}

func outer(value: i32): i32 {
    return value + 1
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_i32(I32Location::Local(1), "inner", vec![]),
            call_i32(I32Location::Local(0), "outer", vec![i32_local(1)]),
            Instruction::AddI32 {
                destination: I32Location::Return,
                left: i32_local(0),
                right: i32_const(1),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_entry_i32_return_expression_with_multiple_normal_calls() {
    let ir = lower_text(
        r#"func main(): i32 {
    return left() + right()
}

func left(): i32 {
    return 20
}

func right(): i32 {
    return 22
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    call_i32(I32Location::Local(0), "left", vec![]),
                    call_i32(I32Location::Local(1), "right", vec![]),
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_local(0),
                        right: i32_local(1),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "left".to_string(),
                return_type: Type::I32,
                instructions: vec![set_return_i32(20), Instruction::Return],
            },
            Function {
                name: "right".to_string(),
                return_type: Type::I32,
                instructions: vec![set_return_i32(22), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_let_initializer_with_multiple_normal_calls() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = left() + right()
    return value
}

func left(): i32 {
    return 20
}

func right(): i32 {
    return 22
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_i32(I32Location::Local(0), "left", vec![]),
            call_i32(I32Location::Local(1), "right", vec![]),
            Instruction::AddI32 {
                destination: I32Location::Local(0),
                left: i32_local(0),
                right: i32_local(1),
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
fn lowers_entry_i32_multiple_normal_calls_without_colliding_with_local() {
    let ir = lower_text(
        r#"func main(): i32 {
    let base = 1
    return (left() + right()) + base
}

func left(): i32 {
    return 20
}

func right(): i32 {
    return 21
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(1),
            },
            call_i32(I32Location::Local(2), "left", vec![]),
            call_i32(I32Location::Local(3), "right", vec![]),
            Instruction::AddI32 {
                destination: I32Location::Local(1),
                left: i32_local(2),
                right: i32_local(3),
            },
            Instruction::AddI32 {
                destination: I32Location::Return,
                left: i32_local(1),
                right: i32_local(0),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_reordered_normal_call_arguments() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func first(a: i32, b: i32): i32 {
    return a
}

func wrapper(a: i32, b: i32): i32 {
    let value = first(b, a)
    return value
}
"#,
        "wrapper",
        context::FunctionSignatures::new(HashMap::from([("first".to_string(), Type::I32)])),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "wrapper".to_string(),
            return_type: Type::I32,
            instructions: vec![
                call_i32(
                    I32Location::Local(0),
                    "first",
                    vec![i32_param(1), i32_param(0)]
                ),
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
fn lowers_reordered_tail_call_arguments() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func first(a: i32, b: i32): i32 {
    return a
}

func wrapper(a: i32, b: i32): i32 {
    return first(b, a)
}
"#,
        "wrapper",
        context::FunctionSignatures::new(HashMap::new()),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "wrapper".to_string(),
            return_type: Type::I32,
            instructions: vec![tail_call("first", vec![i32_param(1), i32_param(0)])],
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
                return_type: Type::I32,
                instructions: vec![
                    call_bool(BoolLocation::Local(0), "ready", vec![]),
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![set_return_i32(0), Instruction::Return],
                        else_instructions: vec![set_return_i32(1), Instruction::Return],
                    },
                ],
            },
            Function {
                name: "ready".to_string(),
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
fn lowers_bool_let_initializer_not_normal_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    let disabled = !ready()
    if disabled {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return false
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_bool(BoolLocation::Local(0), "ready", vec![]),
            Instruction::SetBool {
                destination: BoolLocation::Local(0),
                value: BoolValue::Not(Box::new(BoolValue::Location(BoolLocation::Local(0)))),
            },
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![set_return_i32(42), Instruction::Return],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ]
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
                then_instructions: vec![set_return_i32(42), Instruction::Return],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ]
    );
}

#[test]
fn lowers_entry_i32_if_condition_not_normal_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    if !ready() {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return false
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_bool(BoolLocation::Local(0), "ready", vec![]),
            Instruction::If {
                condition: BoolValue::Not(Box::new(BoolValue::Location(BoolLocation::Local(0)))),
                then_instructions: vec![set_return_i32(42), Instruction::Return],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            },
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
            return_type: Type::Bool,
            instructions: vec![
                call_bool(BoolLocation::Local(0), "ready", vec![]),
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(0)),
                    then_instructions: vec![
                        Instruction::SetBool {
                            destination: BoolLocation::Return,
                            value: BoolValue::Const(false),
                        },
                        Instruction::Return,
                    ],
                    else_instructions: vec![
                        Instruction::SetBool {
                            destination: BoolLocation::Return,
                            value: BoolValue::Const(true),
                        },
                        Instruction::Return,
                    ],
                },
            ],
        }
    );
}

#[test]
fn reports_unsupported_short_circuit_call_in_condition() {
    let diagnostics = lower_text_diagnostics(
        r#"func main(): i32 {
    if ready() && other() {
        return 0
    } else {
        return 1
    }
}

func ready(): bool {
    return true
}

func other(): bool {
    return true
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8006");
    assert_eq!(
        diagnostics[0].message,
        "IR v0 can only lower function calls in direct tail return position"
    );
}

#[test]
fn reports_unsupported_bool_non_tail_call() {
    let diagnostics = lower_named_function_diagnostics_with_signatures(
        r#"func main(): i32 {
    return 0
}

func ready(): bool {
    return true
}

func enabled(): bool {
    return ready() && true
}
"#,
        "enabled",
        context::FunctionSignatures::new(HashMap::new()),
    );

    assert_eq!(diagnostics[0].code, "E8006");
    assert_eq!(
        diagnostics[0].message,
        "IR v0 can only lower function calls in direct tail return position"
    );
}

#[test]
fn reports_unsupported_entry_body() {
    let diagnostics = lower_text_diagnostics(
        r#"func main(): i32 {
    use_value(1)
    return 1
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8002");
}

#[test]
fn rejects_nested_negative_integer_literal() {
    let diagnostics = lower_text_diagnostics(
        r#"func main(): i32 {
    return -(-42)
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8003");
}

fn lower_text(text: &str) -> IrModule {
    lower_text_with_entry(text, crate::entry::DEFAULT_ENTRY_NAME)
}

fn lower_text_with_entry(text: &str, entry_name: &str) -> IrModule {
    let diagnostics = lower_text_diagnostics_with_entry(text, entry_name);
    match diagnostics.as_slice() {
        [] => {
            let analysis = analyze_text_with_entry(text, entry_name);
            lower_executable_with_entry(&analysis, entry_name).unwrap()
        }
        diagnostics => panic!("unexpected diagnostics: {diagnostics:?}"),
    }
}

fn lower_named_function(text: &str, function_name: &str) -> Function {
    lower_named_function_with_signatures(
        text,
        function_name,
        context::FunctionSignatures::new(HashMap::new()),
    )
    .unwrap()
}

fn lower_named_function_with_signatures(
    text: &str,
    function_name: &str,
    function_signatures: context::FunctionSignatures,
) -> Result<Function, Vec<Diagnostic>> {
    let analysis = analyze_text_with_entry(text, crate::entry::DEFAULT_ENTRY_NAME);
    let root = analysis.root_file().unwrap();
    let Some(crate::ast::Item::Function(function)) = root.ast.items.iter().find(|item| {
        matches!(item, crate::ast::Item::Function(function) if function.name == function_name)
    }) else {
        panic!("missing function `{function_name}`");
    };

    functions::lower_function(function, function_signatures)
}

fn lower_named_function_diagnostics_with_signatures(
    text: &str,
    function_name: &str,
    function_signatures: context::FunctionSignatures,
) -> Vec<Diagnostic> {
    match lower_named_function_with_signatures(text, function_name, function_signatures) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics,
    }
}

fn set_return_i32(value: i32) -> Instruction {
    Instruction::SetI32 {
        destination: I32Location::Return,
        value: i32_const(value),
    }
}

fn tail_call(function: &str, arguments: Vec<I32Value>) -> Instruction {
    Instruction::TailCall {
        function: function.to_string(),
        arguments,
    }
}

fn call_i32(destination: I32Location, function: &str, arguments: Vec<I32Value>) -> Instruction {
    Instruction::CallI32 {
        destination,
        function: function.to_string(),
        arguments,
    }
}

fn call_bool(destination: BoolLocation, function: &str, arguments: Vec<I32Value>) -> Instruction {
    Instruction::CallBool {
        destination,
        function: function.to_string(),
        arguments,
    }
}

fn i32_const(value: i32) -> I32Value {
    I32Value::Const(value)
}

fn i32_param(index: usize) -> I32Value {
    I32Value::Location(I32Location::Parameter(index))
}

fn i32_local(index: usize) -> I32Value {
    I32Value::Location(I32Location::Local(index))
}

fn lower_text_diagnostics(text: &str) -> Vec<Diagnostic> {
    lower_text_diagnostics_with_entry(text, crate::entry::DEFAULT_ENTRY_NAME)
}

fn lower_text_diagnostics_with_entry(text: &str, entry_name: &str) -> Vec<Diagnostic> {
    let analysis = analyze_text_with_entry(text, entry_name);
    match lower_executable_with_entry(&analysis, entry_name) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics,
    }
}

fn analyze_text_with_entry(text: &str, entry_name: &str) -> crate::analysis::CompileUnitAnalysis {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let temp_root = make_temp_project();
    let nocter_home = make_nocter_home(&temp_root);
    let unit: CompileUnit = load_compile_unit(
        &mut sources,
        source,
        &FrontendOptions {
            nocter_home: Some(nocter_home),
            target: DEFAULT_TARGET.to_string(),
        },
    )
    .unwrap();
    let analysis = analyze_compile_unit_with_entry(&sources, &unit, entry_name);
    let diagnostics = analysis.diagnostics();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    analysis
}

fn make_temp_project() -> PathBuf {
    let unique = format!(
        "nocter-ir-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let root = std::env::temp_dir().join(unique);
    fs::create_dir_all(&root).unwrap();
    root
}

fn make_nocter_home(root: &Path) -> PathBuf {
    let home = root.join(".nocter");
    fs::create_dir_all(home.join("std")).unwrap();
    fs::create_dir_all(home.join("targets/arm64-darwin/std")).unwrap();
    fs::write(home.join("std/prelude.nct"), "").unwrap();
    home
}
