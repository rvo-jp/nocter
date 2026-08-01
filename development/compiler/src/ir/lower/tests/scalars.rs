use super::*;

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
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(42), Instruction::Return],
        }])
    );
}

#[test]
fn lowers_entry_returning_usize_literal() {
    let ir = lower_text(
        r#"func main(): usize {
    return 42
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Usize,
            instructions: vec![set_return_usize(42), Instruction::Return],
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
            target: crate::ir::CallTarget::same_file("main".to_string()),
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
fn lowers_usize_arithmetic_and_shift_returns() {
    let text = r#"func main(): i32 {
    return 0
}

func add(left: usize, right: usize): usize {
    return left + right
}

func subtract(left: usize, right: usize): usize {
    return left - right
}

func multiply(left: usize, right: usize): usize {
    return left * right
}

func divide(left: usize, right: usize): usize {
    return left / right
}

func remainder(left: usize, right: usize): usize {
    return left % right
}

func shift_left(left: usize, right: usize): usize {
    return left << right
}

func shift_right(left: usize, right: usize): usize {
    return left >> right
}
"#;

    for (name, instruction) in [
        (
            "add",
            Instruction::AddUsize {
                destination: UsizeLocation::Return,
                left: usize_param(0),
                right: usize_param(1),
            },
        ),
        (
            "subtract",
            Instruction::SubtractUsize {
                destination: UsizeLocation::Return,
                left: usize_param(0),
                right: usize_param(1),
            },
        ),
        (
            "multiply",
            Instruction::MultiplyUsize {
                destination: UsizeLocation::Return,
                left: usize_param(0),
                right: usize_param(1),
            },
        ),
        (
            "divide",
            Instruction::DivideUsize {
                destination: UsizeLocation::Return,
                left: usize_param(0),
                right: usize_param(1),
            },
        ),
        (
            "remainder",
            Instruction::RemainderUsize {
                destination: UsizeLocation::Return,
                left: usize_param(0),
                right: usize_param(1),
            },
        ),
        (
            "shift_left",
            Instruction::ShiftLeftUsize {
                destination: UsizeLocation::Return,
                left: usize_param(0),
                right: usize_param(1),
            },
        ),
        (
            "shift_right",
            Instruction::ShiftRightUsize {
                destination: UsizeLocation::Return,
                left: usize_param(0),
                right: usize_param(1),
            },
        ),
    ] {
        assert_eq!(
            lower_named_function(text, name),
            Function {
                name: name.to_string(),
                target: crate::ir::CallTarget::same_file(name),
                return_type: Type::Usize,
                instructions: vec![instruction, Instruction::Return],
            }
        );
    }
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
            target: crate::ir::CallTarget::same_file("main".to_string()),
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
fn lowers_usize_compound_remainder_assignment() {
    let ir = lower_text(
        r#"func main(): usize {
    var total: usize = 42
    total %= 5
    return total
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::SetUsize {
                destination: UsizeLocation::Local(0),
                value: usize_const(42),
            },
            Instruction::RemainderUsize {
                destination: UsizeLocation::Local(0),
                left: usize_local(0),
                right: usize_const(5),
            },
            Instruction::SetUsize {
                destination: UsizeLocation::Return,
                value: usize_local(0),
            },
            Instruction::Return,
        ]
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
fn lowers_i32_unary_negate_local_return() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = 7
    return -value
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(7),
            },
            Instruction::SubtractI32 {
                destination: I32Location::Return,
                left: i32_const(0),
                right: i32_local(0),
            },
            Instruction::Return
        ]
    );
}

#[test]
fn lowers_nested_negative_integer_literal() {
    let ir = lower_text(
        r#"func main(): i32 {
    return -(-42)
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::SubtractI32 {
                destination: I32Location::Return,
                left: i32_const(0),
                right: i32_const(-42),
            },
            Instruction::Return
        ]
    );
}
