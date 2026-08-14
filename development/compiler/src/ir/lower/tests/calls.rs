use super::*;

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
                target: crate::ir::CallTarget::same_file("main".to_string()),
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
                target: crate::ir::CallTarget::same_file("answer".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(42), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_associated_function_call_target() {
    let ir = lower_text(
        r#"struct Point {
    x: i32
}

func Point.origin(): i32 {
    return 42
}

func main(): i32 {
    return Point.origin()
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
                instructions: vec![tail_call("Point.origin", vec![])],
            },
            Function {
                name: "Point.origin".to_string(),
                target: crate::ir::CallTarget::same_file("Point.origin".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(42), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_let_initializer_associated_function_call_target() {
    let ir = lower_text(
        r#"struct Point {
    x: i32
}

func Point.origin(): i32 {
    return 42
}

func main(): i32 {
    let value = Point.origin()
    return value
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
                    call_i32(I32Location::Local(0), "Point.origin", vec![]),
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "Point.origin".to_string(),
                target: crate::ir::CallTarget::same_file("Point.origin".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(42), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_usize_returning_normal_call_in_let_initializer() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value: usize = size()
    if value >= 42 {
        return 0
    } else {
        return 1
    }
}

func size(): usize {
    return 42
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
                    call_usize(UsizeLocation::Local(0), "size", vec![]),
                    Instruction::SetBool {
                        destination: BoolLocation::Local(1),
                        value: BoolValue::UsizeComparison {
                            operator: I32ComparisonOperator::GreaterEqual,
                            left: usize_local(0),
                            right: usize_const(42),
                        },
                    },
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(1)),
                        then_instructions: vec![set_return_i32(0)],
                        else_instructions: vec![set_return_i32(1)],
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "size".to_string(),
                target: crate::ir::CallTarget::same_file("size".to_string()),
                return_type: Type::Usize,
                instructions: vec![set_return_usize(42), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_usize_parameter_normal_call_in_let_initializer() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value: usize = choose(7, 42)
    if value == 42 {
        return 0
    } else {
        return 1
    }
}

func choose(code: i32, value: usize): usize {
    return value
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
                    call_usize(
                        UsizeLocation::Local(0),
                        "choose",
                        vec![
                            ScalarArgument::I32(i32_const(7)),
                            ScalarArgument::Usize(usize_const(42)),
                        ],
                    ),
                    Instruction::SetBool {
                        destination: BoolLocation::Local(1),
                        value: BoolValue::UsizeComparison {
                            operator: I32ComparisonOperator::Equal,
                            left: usize_local(0),
                            right: usize_const(42),
                        },
                    },
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(1)),
                        then_instructions: vec![set_return_i32(0)],
                        else_instructions: vec![set_return_i32(1)],
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "choose".to_string(),
                target: crate::ir::CallTarget::same_file("choose".to_string()),
                return_type: Type::Usize,
                instructions: vec![
                    Instruction::SetUsize {
                        destination: UsizeLocation::Return,
                        value: usize_param(1),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_usize_parameter_tail_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value: usize = forward(42)
    if value == 42 {
        return 0
    } else {
        return 1
    }
}

func forward(value: usize): usize {
    return identity(value)
}

func identity(value: usize): usize {
    return value
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
                    call_usize(
                        UsizeLocation::Local(0),
                        "forward",
                        vec![ScalarArgument::Usize(usize_const(42))],
                    ),
                    Instruction::SetBool {
                        destination: BoolLocation::Local(1),
                        value: BoolValue::UsizeComparison {
                            operator: I32ComparisonOperator::Equal,
                            left: usize_local(0),
                            right: usize_const(42),
                        },
                    },
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(1)),
                        then_instructions: vec![set_return_i32(0)],
                        else_instructions: vec![set_return_i32(1)],
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "forward".to_string(),
                target: crate::ir::CallTarget::same_file("forward".to_string()),
                return_type: Type::Usize,
                instructions: vec![Instruction::TailCall {
                    target: CallTarget::same_file("identity"),
                    arguments: vec![ScalarArgument::Usize(usize_param(0))],
                }],
            },
            Function {
                name: "identity".to_string(),
                target: crate::ir::CallTarget::same_file("identity".to_string()),
                return_type: Type::Usize,
                instructions: vec![
                    Instruction::SetUsize {
                        destination: UsizeLocation::Return,
                        value: usize_param(0),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_usize_call_in_nested_arithmetic_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func score(base: usize): usize {
    return base + size() * 2
}

func size(): usize {
    return 20
}
"#,
        "score",
    );

    assert_eq!(
        function,
        Function {
            name: "score".to_string(),
            target: crate::ir::CallTarget::same_file("score".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                call_usize(UsizeLocation::Local(1), "size", vec![]),
                Instruction::MultiplyUsize {
                    destination: UsizeLocation::Local(0),
                    left: usize_local(1),
                    right: usize_const(2),
                },
                Instruction::AddUsize {
                    destination: UsizeLocation::Return,
                    left: usize_param(0),
                    right: usize_local(0),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_never_function_returning_target_trap_primitive() {
    let fixture = analyze_text_fixture_with_nocter_home_files(
        r#"use std/process.abort

func main(): i32 {
    return abort()
}
"#,
        &[std_process_file(), std_os_file()],
    );
    let analysis = &fixture.analysis;
    let process_source = analysis
        .files
        .iter()
        .find(|file| {
            !file.is_root
                && file.ast.items.iter().any(|item| {
                    matches!(item, crate::ast::Item::Function(function) if function.name == "abort")
                })
        })
        .map(|file| file.ast.span.source)
        .unwrap();

    let ir = lower_executable(analysis, &fixture.sources).unwrap();

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: CallTarget::same_file("main"),
                return_type: Type::I32,
                instructions: vec![Instruction::TailCall {
                    target: CallTarget::imported(process_source, "abort"),
                    arguments: vec![],
                }],
            },
            Function {
                name: "abort".to_string(),
                target: CallTarget::imported(process_source, "abort"),
                return_type: Type::Never,
                instructions: vec![Instruction::Trap],
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
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
                target: crate::ir::CallTarget::same_file("add".to_string()),
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
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
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
                target: crate::ir::CallTarget::same_file("answer".to_string()),
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    call_i32(I32Location::Local(1), "answer", vec![]),
                    Instruction::AddI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(1),
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
                target: crate::ir::CallTarget::same_file("answer".to_string()),
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
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
                target: crate::ir::CallTarget::same_file("answer".to_string()),
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
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
                target: crate::ir::CallTarget::same_file("answer".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(39), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_i32_compound_assignment_with_call_rhs() {
    let ir = lower_text(
        r#"func main(): i32 {
    var total = 40
    total += answer()
    return total
}

func answer(): i32 {
    return 2
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
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(40),
                    },
                    call_i32(I32Location::Local(1), "answer", vec![]),
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
                ],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(2), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_ignored_i32_call_expression_statement() {
    let ir = lower_text(
        r#"func main(): i32 {
    value()
    return 0
}

func value(): i32 {
    return 1
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
                    call_i32(I32Location::Local(0), "value", vec![]),
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_const(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "value".to_string(),
                target: crate::ir::CallTarget::same_file("value".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(1), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_subtract_and_multiply_with_normal_calls() {
    let ir = lower_text(
        r#"func main(): i32 {
    return answer() * 2 - offset()
}

func answer(): i32 {
    return 24
}

func offset(): i32 {
    return 6
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
                    call_i32(I32Location::Local(1), "answer", vec![]),
                    Instruction::MultiplyI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(1),
                        right: i32_const(2),
                    },
                    call_i32(I32Location::Local(2), "offset", vec![]),
                    Instruction::SubtractI32 {
                        destination: I32Location::Return,
                        left: i32_local(0),
                        right: i32_local(2),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(24), Instruction::Return],
            },
            Function {
                name: "offset".to_string(),
                target: crate::ir::CallTarget::same_file("offset".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(6), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_divide_and_remainder_with_normal_calls() {
    let ir = lower_text(
        r#"func main(): i32 {
    return total() / divisor() + dividend() % modulus()
}

func total(): i32 {
    return 84
}

func divisor(): i32 {
    return 2
}

func dividend(): i32 {
    return 85
}

func modulus(): i32 {
    return 43
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
                    call_i32(I32Location::Local(1), "total", vec![]),
                    call_i32(I32Location::Local(2), "divisor", vec![]),
                    Instruction::DivideI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(1),
                        right: i32_local(2),
                    },
                    call_i32(I32Location::Local(4), "dividend", vec![]),
                    call_i32(I32Location::Local(5), "modulus", vec![]),
                    Instruction::RemainderI32 {
                        destination: I32Location::Local(3),
                        left: i32_local(4),
                        right: i32_local(5),
                    },
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_local(0),
                        right: i32_local(3),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "total".to_string(),
                target: crate::ir::CallTarget::same_file("total".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(84), Instruction::Return],
            },
            Function {
                name: "divisor".to_string(),
                target: crate::ir::CallTarget::same_file("divisor".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(2), Instruction::Return],
            },
            Function {
                name: "dividend".to_string(),
                target: crate::ir::CallTarget::same_file("dividend".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(85), Instruction::Return],
            },
            Function {
                name: "modulus".to_string(),
                target: crate::ir::CallTarget::same_file("modulus".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(43), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_shifts_with_normal_calls() {
    let ir = lower_text(
        r#"func main(): i32 {
    return (value() << left_count()) + (shifted() >> right_count())
}

func value(): i32 {
    return 5
}

func left_count(): i32 {
    return 3
}

func shifted(): i32 {
    return 8
}

func right_count(): i32 {
    return 1
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
                    call_i32(I32Location::Local(1), "value", vec![]),
                    call_i32(I32Location::Local(2), "left_count", vec![]),
                    Instruction::ShiftLeftI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(1),
                        right: i32_local(2),
                    },
                    call_i32(I32Location::Local(4), "shifted", vec![]),
                    call_i32(I32Location::Local(5), "right_count", vec![]),
                    Instruction::ShiftRightI32 {
                        destination: I32Location::Local(3),
                        left: i32_local(4),
                        right: i32_local(5),
                    },
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_local(0),
                        right: i32_local(3),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "value".to_string(),
                target: crate::ir::CallTarget::same_file("value".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(5), Instruction::Return],
            },
            Function {
                name: "left_count".to_string(),
                target: crate::ir::CallTarget::same_file("left_count".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(3), Instruction::Return],
            },
            Function {
                name: "shifted".to_string(),
                target: crate::ir::CallTarget::same_file("shifted".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(8), Instruction::Return],
            },
            Function {
                name: "right_count".to_string(),
                target: crate::ir::CallTarget::same_file("right_count".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(1), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_void_entry_with_void_call_statement() {
    let ir = lower_text(
        r#"func main(): void {
    effect()
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
                instructions: vec![call_void("effect", vec![]), Instruction::Return],
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
fn lowers_entry_leading_void_call_statement_before_return() {
    let ir = lower_text(
        r#"func main(): i32 {
    effect()
    return 7
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
                return_type: Type::I32,
                instructions: vec![
                    call_void("effect", vec![]),
                    set_return_i32(7),
                    Instruction::Return
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
fn lowers_void_function_with_void_call_statement() {
    let ir = lower_text(
        r#"func main(): i32 {
    run()
    return 7
}

func run(): void {
    effect()
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
            instructions: vec![call_void("effect", vec![]), Instruction::Return],
        }
    );
}

#[test]
fn lowers_void_function_with_binding_before_implicit_return() {
    let ir = lower_text(
        r#"func main(): void {
    run()
}

func run(): void {
    let value = 1
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
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(1),
                },
                Instruction::Return,
            ],
        }
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("add", vec![i32_const(20), i32_const(22)])],
            },
            Function {
                name: "add".to_string(),
                target: crate::ir::CallTarget::same_file("add".to_string()),
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
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
fn lowers_entry_i32_nested_tail_call_argument() {
    let ir = lower_text(
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

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_i32(I32Location::Local(0), "answer", vec![]),
            tail_call("add", vec![i32_local(0), i32_const(1)]),
        ]
    );
}

#[test]
fn lowers_entry_i32_multiple_nested_tail_call_arguments() {
    let ir = lower_text(
        r#"func main(): i32 {
    return add(left(), right())
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
            tail_call("add", vec![i32_local(0), i32_local(1)]),
        ]
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    call_i32(I32Location::Local(1), "inner", vec![]),
                    call_i32(I32Location::Local(0), "outer", vec![i32_local(1)]),
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "inner".to_string(),
                target: crate::ir::CallTarget::same_file("inner".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(41), Instruction::Return],
            },
            Function {
                name: "outer".to_string(),
                target: crate::ir::CallTarget::same_file("outer".to_string()),
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
            call_i32(I32Location::Local(1), "left", vec![]),
            call_i32(I32Location::Local(2), "right", vec![]),
            call_i32(
                I32Location::Local(0),
                "add",
                vec![i32_local(1), i32_local(2)]
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
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
                target: crate::ir::CallTarget::same_file("left".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(20), Instruction::Return],
            },
            Function {
                name: "right".to_string(),
                target: crate::ir::CallTarget::same_file("right".to_string()),
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
            call_i32(I32Location::Local(1), "left", vec![]),
            call_i32(I32Location::Local(2), "right", vec![]),
            Instruction::AddI32 {
                destination: I32Location::Local(0),
                left: i32_local(1),
                right: i32_local(2),
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
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
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
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
            return_type: Type::I32,
            instructions: vec![tail_call("first", vec![i32_param(1), i32_param(0)])],
        }
    );
}
