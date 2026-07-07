use super::support::parse_text;
use crate::ast::{BindingKind, Expr, Item, Stmt};

#[test]
fn parses_optional_let_else_binding() {
    let output = parse_text(
        r#"program(): i32 {
    let home = lookup("HOME") else {
        return 1
    }

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::Binding(binding) = &program.body.statements[0] else {
        panic!("expected binding statement");
    };

    assert_eq!(binding.kind, BindingKind::Let);
    assert!(binding.else_block.is_some());
    assert!(matches!(binding.initializer, Expr::Call(_)));
}

#[test]
fn parses_optional_var_else_binding() {
    let output = parse_text(
        r#"program(): i32 {
    var text = maybe_text else {
        return 1
    }

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::Binding(binding) = &program.body.statements[0] else {
        panic!("expected binding statement");
    };

    assert_eq!(binding.kind, BindingKind::Var);
    assert!(binding.else_block.is_some());
}

#[test]
fn parses_optional_if_let_and_if_var_statements() {
    let output = parse_text(
        r#"program(): i32 {
    if let home = maybe_home {
        return 0
    } else {
        return 1
    }

    if var text = maybe_text {
        return 0
    }

    return 1
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::IfLet(first) = &program.body.statements[0] else {
        panic!("expected if let statement");
    };
    let Stmt::IfLet(second) = &program.body.statements[1] else {
        panic!("expected if var statement");
    };

    assert_eq!(first.kind, BindingKind::Let);
    assert_eq!(first.name, "home");
    assert!(first.else_block.is_some());
    assert_eq!(second.kind, BindingKind::Var);
    assert_eq!(second.name, "text");
    assert!(second.else_block.is_none());
}

#[test]
fn parses_else_if_chains_as_nested_else_blocks() {
    let output = parse_text(
        r#"program(): i32 {
    if ready {
        return 0
    } else if let value = maybe_value {
        return value
    } else if var fallback = maybe_fallback {
        return fallback
    } else {
        return 3
    }
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::If(first) = &program.body.statements[0] else {
        panic!("expected if statement");
    };
    let first_else = first.else_block.as_ref().expect("expected else block");
    let Stmt::IfLet(second) = &first_else.statements[0] else {
        panic!("expected nested if let statement");
    };
    let second_else = second
        .else_block
        .as_ref()
        .expect("expected second else block");
    let Stmt::IfLet(third) = &second_else.statements[0] else {
        panic!("expected nested if var statement");
    };

    assert_eq!(second.kind, BindingKind::Let);
    assert_eq!(second.name, "value");
    assert_eq!(third.kind, BindingKind::Var);
    assert_eq!(third.name, "fallback");
    assert!(third.else_block.is_some());
}

#[test]
fn parses_while_and_optional_while_statements() {
    let output = parse_text(
        r#"program(): i32 {
    while ready {
        tick()
    }

    while let value = next_value {
        use_value(value)
    }

    while var text = next_text {
        use_text(text)
    }

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::While(first) = &program.body.statements[0] else {
        panic!("expected while statement");
    };
    let Stmt::WhileLet(second) = &program.body.statements[1] else {
        panic!("expected while let statement");
    };
    let Stmt::WhileLet(third) = &program.body.statements[2] else {
        panic!("expected while var statement");
    };

    assert!(matches!(first.condition, Expr::Identifier(_)));
    assert_eq!(second.kind, BindingKind::Let);
    assert_eq!(second.name, "value");
    assert_eq!(third.kind, BindingKind::Var);
    assert_eq!(third.name, "text");
}

#[test]
fn parses_break_and_continue_statements() {
    let output = parse_text(
        r#"program(): i32 {
    while ready {
        break
    }

    while waiting {
        continue
    }

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::While(first) = &program.body.statements[0] else {
        panic!("expected while statement");
    };
    let Stmt::While(second) = &program.body.statements[1] else {
        panic!("expected while statement");
    };

    assert!(matches!(first.body.statements[0], Stmt::Break(_)));
    assert!(matches!(second.body.statements[0], Stmt::Continue(_)));
}

#[test]
fn parses_loop_statement() {
    let output = parse_text(
        r#"program(): i32 {
    loop {
        continue
    }

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::Loop(statement) = &program.body.statements[0] else {
        panic!("expected loop statement");
    };

    assert!(matches!(statement.body.statements[0], Stmt::Continue(_)));
}

#[test]
fn parses_fail_statement() {
    let output = parse_text(
        r#"program(): i32 {
    return 0
}

func run(error: error): i32! {
    fail error
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[1] else {
        panic!("expected function item");
    };
    assert!(matches!(function.body.statements[0], Stmt::Fail(_)));
}

#[test]
fn parses_switch_statement() {
    let output = parse_text(
        r#"enum AppError {
    missing_path
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    switch error {
        is AppError.missing_path {
            return 1
        }

        is AppError.open_failed(path) {
            return 2
        }
    }

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[2] else {
        panic!("expected function item");
    };
    let Stmt::Switch(statement) = &function.body.statements[0] else {
        panic!("expected switch statement");
    };

    assert_eq!(statement.arms.len(), 2);
    assert!(statement.arms[0].payload.is_none());
    assert_eq!(
        statement.arms[1]
            .payload
            .as_ref()
            .map(|payload| payload.name.as_str()),
        Some("path")
    );
    assert!(statement.else_arm.is_none());
}

#[test]
fn parses_switch_else_arm() {
    let output = parse_text(
        r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    switch error {
        is AppError.missing_path {
            return 1
        }

        else {
            return 0
        }
    }
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[2] else {
        panic!("expected function item");
    };
    let Stmt::Switch(statement) = &function.body.statements[0] else {
        panic!("expected switch statement");
    };

    assert_eq!(statement.arms.len(), 1);
    assert!(statement.else_arm.is_some());
}

#[test]
fn parses_if_is_statement() {
    let output = parse_text(
        r#"enum AppError {
    missing_path
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    if error is AppError.open_failed(path) {
        return 1
    } else if error is AppError.missing_path {
        return 2
    } else {
        return 0
    }
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[2] else {
        panic!("expected function item");
    };
    let Stmt::IfIs(statement) = &function.body.statements[0] else {
        panic!("expected if-is statement");
    };

    assert_eq!(statement.enum_name, "AppError");
    assert_eq!(statement.variant_name, "open_failed");
    assert_eq!(
        statement
            .payload
            .as_ref()
            .map(|payload| payload.name.as_str()),
        Some("path")
    );
    let Some(else_block) = &statement.else_block else {
        panic!("expected else block");
    };
    assert!(matches!(else_block.statements[0], Stmt::IfIs(_)));
}

#[test]
fn rejects_switch_arm_after_else() {
    let output = parse_text(
        r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    switch error {
        else {
            return 0
        }

        is AppError.missing_path {
            return 1
        }
    }
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1);
    assert!(output.diagnostics[0].message.contains("last"));
}

#[test]
fn rejects_duplicate_switch_else_arm() {
    let output = parse_text(
        r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    switch error {
        else {
            return 0
        }

        else {
            return 1
        }
    }
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1);
    assert!(output.diagnostics[0].message.contains("only one"));
}

#[test]
fn parses_range_for_statement() {
    let output = parse_text(
        r#"program(): i32 {
    for i in 0..<4 {
        use_value(i)
    }

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::ForRange(statement) = &program.body.statements[0] else {
        panic!("expected range for statement");
    };

    assert_eq!(statement.name, "i");
    assert!(matches!(statement.start, Expr::IntegerLiteral(_)));
    assert!(matches!(statement.end, Expr::IntegerLiteral(_)));
    assert_eq!(statement.body.statements.len(), 1);
}

#[test]
fn rejects_non_range_for_statement() {
    let output = parse_text(
        r#"program(): void {
    for item in items {
    }
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1);
    assert!(output.diagnostics[0].message.contains("..<"));
}

#[test]
fn rejects_loop_control_with_values() {
    let output = parse_text(
        r#"program(): void {
    while ready {
        break 1
    }
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1);
    assert!(
        output.diagnostics[0]
            .message
            .contains("does not take a value")
    );
}

#[test]
fn parses_if_else_statement_and_bool_literals() {
    let output = parse_text(
        r#"program(): i32 {
    if true {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::If(statement) = &program.body.statements[0] else {
        panic!("expected if statement");
    };
    assert!(matches!(statement.condition, Expr::BoolLiteral(_)));
    assert!(statement.else_block.is_some());
}
