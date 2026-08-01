use super::support::{
    assert_rejects_discard_name, assert_rejects_self_name, find_json_node, parse_text,
    parse_text_with_sources,
};
use crate::ast::{AssignmentOperator, Expr, Item, Stmt};

#[test]
fn parses_lexical_region_statement_and_json_shape() {
    let (sources, output) = parse_text_with_sources(
        r#"func main(arena: usize): void {
    region temp using arena {
        let value = temp
    }
    return
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::Region(region) = &function.body.statements[0] else {
        panic!("expected region statement");
    };
    assert_eq!(region.name, "temp");
    assert!(matches!(region.allocator, Expr::Identifier(_)));
    assert_eq!(region.body.statements.len(), 1);

    let json = ast.to_json(&sources);
    let region = find_json_node(&json, "region_statement").expect("expected region JSON node");
    assert_eq!(region.value.as_deref(), Some("temp"));
    assert_eq!(region.items[0].kind, "region_binding");
    assert_eq!(region.items[1].kind, "identifier");
    assert_eq!(region.items[2].kind, "body");
}

#[test]
fn rejects_optional_let_else_binding() {
    let output = parse_text(
        r#"func main(): i32 {
    let home = lookup("HOME") else {
        return 1
    }

    return 0
}
"#,
    );

    assert!(output.ast.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("`let ... else` and `var ... else` were removed")
    }));
}

#[test]
fn rejects_optional_var_else_binding() {
    let output = parse_text(
        r#"func main(): i32 {
    var text = maybe_text else {
        return 1
    }

    return 0
}
"#,
    );

    assert!(output.ast.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("`let ... else` and `var ... else` were removed")
    }));
}

#[test]
fn rejects_discard_name_for_statement_bindings() {
    for source in [
        r#"func main(): i32 {
    let _ = 1
    return 0
}
"#,
        r#"func main(): void {
    drop _
    return
}
"#,
        r#"func main(): i32 {
    for _ in 0..<1 {
        return 0
    }
    return 0
}
"#,
    ] {
        assert_rejects_discard_name(source);
    }
}

#[test]
fn rejects_self_name_for_statement_bindings() {
    for source in [
        r#"func main(): i32 {
    let Self = 1
    return 0
}
"#,
        r#"func main(): void {
    drop Self
    return
}
"#,
        r#"func main(): i32 {
    for Self in 0..<1 {
        return 0
    }
    return 0
}
"#,
        r#"func main(choice: Choice): i32 {
    return match choice {
        Choice.some(Self) { 1 }
        _ { 0 }
    }
}
"#,
    ] {
        assert_rejects_self_name(source);
    }
}

#[test]
fn parses_block_use_statements_at_block_start() {
    let output = parse_text(
        r#"func greet(): void {
    use std/io.print
    print("hello")
    return
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };

    assert!(matches!(function.body.statements[0], Stmt::FromImport(_)));
    assert!(matches!(function.body.statements[1], Stmt::Expression(_)));
    assert!(matches!(function.body.statements[2], Stmt::Return(_)));
}

#[test]
fn parses_block_use_at_nested_block_start() {
    let output = parse_text(
        r#"func process(debug: bool): void {
    if debug {
        use std/io.print
        print("debug")
    }
    return
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::If(statement) = &function.body.statements[0] else {
        panic!("expected if statement");
    };

    assert!(matches!(
        statement.then_block.statements[0],
        Stmt::FromImport(_)
    ));
}

#[test]
fn rejects_block_use_after_other_statement() {
    let output = parse_text(
        r#"func greet(): void {
    print("hello")
    use std/io.print
    return
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains("block `use` declarations must appear before other statements")
    );
}

#[test]
fn rejects_public_block_use() {
    let output = parse_text(
        r#"func greet(): void {
    pub use std/io.print
    return
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains("block `use` declarations cannot be public")
    );
}

#[test]
fn parses_assignment_and_compound_assignment_statements() {
    let output = parse_text(
        r#"func main(): i32 {
    var count = 0
    count = 1
    count += 2
    stats.lines += 1
    return count
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };

    let Stmt::Assignment(assign) = &function.body.statements[1] else {
        panic!("expected assignment statement");
    };
    assert_eq!(assign.operator, AssignmentOperator::Assign);
    assert!(matches!(assign.target, Expr::Identifier(_)));
    assert!(matches!(assign.value, Expr::IntegerLiteral(_)));

    let Stmt::Assignment(add_assign) = &function.body.statements[2] else {
        panic!("expected compound assignment statement");
    };
    assert_eq!(add_assign.operator, AssignmentOperator::AddAssign);
    assert!(matches!(add_assign.target, Expr::Identifier(_)));

    let Stmt::Assignment(field_assign) = &function.body.statements[3] else {
        panic!("expected field compound assignment statement");
    };
    assert_eq!(field_assign.operator, AssignmentOperator::AddAssign);
    assert!(matches!(field_assign.target, Expr::Member(_)));
}

#[test]
fn parses_parenthesized_assignment_target() {
    let output = parse_text(
        r#"func main(): i32 {
    var count = 0
    (count) = 1
    (stats.lines) += 1
    return count
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };

    let Stmt::Assignment(assign) = &function.body.statements[1] else {
        panic!("expected assignment statement");
    };
    assert_eq!(assign.operator, AssignmentOperator::Assign);
    assert!(matches!(assign.target, Expr::Group(_)));

    let Stmt::Assignment(field_assign) = &function.body.statements[2] else {
        panic!("expected field assignment statement");
    };
    assert_eq!(field_assign.operator, AssignmentOperator::AddAssign);
    assert!(matches!(field_assign.target, Expr::Group(_)));
}

#[test]
fn parses_drop_statement_without_reserving_drop_identifier() {
    let output = parse_text(
        r#"func main(): void {
    var file = open()
    drop file
    drop(file)
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };

    let Stmt::Drop(drop_) = &function.body.statements[1] else {
        panic!("expected drop statement");
    };
    assert_eq!(drop_.name, "file");

    assert!(matches!(
        function.body.result.as_deref(),
        Some(Expr::Call(_))
    ));
}

#[test]
fn diagnoses_deferred_drop_field_target() {
    let output = parse_text(
        r#"func main(): void {
    drop file.handle
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(output.diagnostics[0].message.contains("drop object.field"));
}

#[test]
fn diagnoses_deferred_drop_index_target() {
    let output = parse_text(
        r#"func main(): void {
    drop files[0]
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(output.diagnostics[0].message.contains("drop array[index]"));
}

#[test]
fn diagnoses_deferred_drop_call_target() {
    let output = parse_text(
        r#"func main(): void {
    drop make_file()
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(output.diagnostics[0].message.contains("drop make_value()"));
}

#[test]
fn rejects_non_place_assignment_target() {
    let output = parse_text(
        r#"func main(): i32 {
    a + b = c
    return 0
}
"#,
    );

    assert!(output.ast.is_none());
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "expected assignment target")
    );
}

#[test]
fn parses_index_assignment_targets() {
    let output = parse_text(
        r#"func main(): i32 {
    bytes[0] = 1
    rows[0].count += 1
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };

    let Stmt::Assignment(first) = &function.body.statements[0] else {
        panic!("expected assignment statement");
    };
    assert_eq!(first.operator, AssignmentOperator::Assign);
    assert!(matches!(first.target, Expr::Index(_)));

    let Stmt::Assignment(second) = &function.body.statements[1] else {
        panic!("expected assignment statement");
    };
    assert_eq!(second.operator, AssignmentOperator::AddAssign);
    let Expr::Member(member) = &second.target else {
        panic!("expected member assignment target");
    };
    assert!(matches!(member.object.as_ref(), Expr::Index(_)));
}

#[test]
fn rejects_removed_if_let_and_if_var_statements() {
    let output = parse_text(
        r#"func main(): i32 {
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

    assert!(output.ast.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("`if let` and `if var` were removed")
    }));
}

#[test]
fn parses_else_if_chains_as_nested_else_blocks() {
    let output = parse_text(
        r#"enum Status {
    ready(value: i32)
    fallback(value: i32)
}

func main(status: Status): i32 {
    if ready {
        return 0
    } else if status is Status.ready(value) {
        return value
    } else if status is Status.fallback(fallback) {
        return fallback
    } else {
        return 3
    }
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[1] else {
        panic!("expected function item");
    };
    let Stmt::If(first) = &function.body.statements[0] else {
        panic!("expected if statement");
    };
    let first_else = first.else_block.as_ref().expect("expected else block");
    let Stmt::IfIs(second) = &first_else.statements[0] else {
        panic!("expected nested if-is statement");
    };
    let second_else = second
        .else_block
        .as_ref()
        .expect("expected second else block");
    let Stmt::IfIs(third) = &second_else.statements[0] else {
        panic!("expected nested if-is statement");
    };

    assert_eq!(second.variant_name, "ready");
    assert_eq!(third.variant_name, "fallback");
    assert!(third.else_block.is_some());
}

#[test]
fn parses_while_statement() {
    let output = parse_text(
        r#"func main(): i32 {
    while ready {
        tick()
    }

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::While(first) = &function.body.statements[0] else {
        panic!("expected while statement");
    };

    assert!(matches!(first.condition, Expr::Identifier(_)));
}

#[test]
fn rejects_removed_while_let_and_while_var_statements() {
    let output = parse_text(
        r#"func main(): i32 {
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

    assert!(output.ast.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("`while let` and `while var` were removed")
    }));
}

#[test]
fn parses_break_and_continue_statements() {
    let output = parse_text(
        r#"func main(): i32 {
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
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::While(first) = &function.body.statements[0] else {
        panic!("expected while statement");
    };
    let Stmt::While(second) = &function.body.statements[1] else {
        panic!("expected while statement");
    };

    assert!(matches!(first.body.statements[0], Stmt::Break(_)));
    assert!(matches!(second.body.statements[0], Stmt::Continue(_)));
}

#[test]
fn parses_loop_statement() {
    let output = parse_text(
        r#"func main(): i32 {
    loop {
        continue
    }

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::Loop(statement) = &function.body.statements[0] else {
        panic!("expected loop statement");
    };

    assert!(matches!(statement.body.statements[0], Stmt::Continue(_)));
}

#[test]
fn parses_error_return_statement() {
    let output = parse_text(
        r#"func main(): i32 {
    return 0
}

func run(error: error): i32! {
    return error
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[1] else {
        panic!("expected function item");
    };
    assert!(matches!(function.body.statements[0], Stmt::Return(_)));
}

#[test]
fn parses_switch_statement() {
    let output = parse_text(
        r#"enum AppError {
    missing_path
    open_failed(path: &str)
}

func main(): i32 {
    return 0
}

func code(error: AppError): i32 {
    match error {
        AppError.missing_path {
            return 1
        }

        AppError.open_failed(path) {
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
        panic!("expected match statement");
    };

    assert_eq!(statement.arms.len(), 2);
    assert!(statement.arms[0].payload.is_none());
    assert_eq!(
        statement.arms[1]
            .payload
            .as_ref()
            .and_then(|payload| payload.binding_name()),
        Some("path")
    );
    assert!(statement.wildcard_arm.is_none());
}

#[test]
fn parses_switch_wildcard_arm() {
    let output = parse_text(
        r#"enum AppError {
    missing_path
}

func main(): i32 {
    return 0
}

func code(error: AppError): i32 {
    match error {
        AppError.missing_path {
            return 1
        }

        _ {
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
        panic!("expected match statement");
    };

    assert_eq!(statement.arms.len(), 1);
    assert!(statement.wildcard_arm.is_some());
}

#[test]
fn parses_if_is_statement() {
    let output = parse_text(
        r#"enum AppError {
    missing_path
    open_failed(path: &str)
}

func main(): i32 {
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
            .and_then(|payload| payload.binding_name()),
        Some("path")
    );
    let Some(else_block) = &statement.else_block else {
        panic!("expected else block");
    };
    assert!(matches!(else_block.statements[0], Stmt::IfIs(_)));
}

#[test]
fn rejects_unqualified_if_is_patterns() {
    let output = parse_text(
        r#"struct Test {
    value: i32
}

func main(maybe: Test?, fallible: Test!): i32 {
    if maybe is none {
        return 0
    }

    if maybe is Test(value) {
        return value.value
    }

    if fallible is Error(error) {
        return 1
    }

    return 3
}
"#,
    );

    assert!(output.ast.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("expected enum pattern `Enum.variant` after `is`")
    }));
}

#[test]
fn rejects_if_is_wildcard_pattern() {
    let output = parse_text(
        r#"enum AppError {
    missing_path
}

func main(error: AppError): i32 {
    if error is _ {
        return 1
    }

    return 0
}
"#,
    );

    assert!(output.ast.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("`if is` requires an enum variant pattern")
    }));
}

#[test]
fn parses_payload_discard_patterns() {
    let output = parse_text(
        r#"enum AppError {
    missing_path
    open_failed(path: &str)
}

func main(): i32 {
    return 0
}

func code(error: AppError): i32 {
    match error {
        AppError.open_failed(_) {
            return 1
        }

        _ {
            return 0
        }
    }

    if error is AppError.open_failed(_) {
        return 2
    }

    return 3
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[2] else {
        panic!("expected function item");
    };
    let Stmt::Switch(statement) = &function.body.statements[0] else {
        panic!("expected match statement");
    };
    assert!(statement.arms[0].payload.as_ref().is_some_and(|payload| {
        payload.binding_name().is_none() && payload.span().start < payload.span().end
    }));
    let Stmt::IfIs(statement) = &function.body.statements[1] else {
        panic!("expected if-is statement");
    };
    assert!(statement.payload.as_ref().is_some_and(|payload| {
        payload.binding_name().is_none() && payload.span().start < payload.span().end
    }));
}

#[test]
fn rejects_switch_arm_after_wildcard() {
    let output = parse_text(
        r#"enum AppError {
    missing_path
}

func main(): i32 {
    return 0
}

func code(error: AppError): i32 {
    match error {
        _ {
            return 0
        }

        AppError.missing_path {
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
fn rejects_duplicate_switch_wildcard_arm() {
    let output = parse_text(
        r#"enum AppError {
    missing_path
}

func main(): i32 {
    return 0
}

func code(error: AppError): i32 {
    match error {
        _ {
            return 0
        }

        _ {
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
fn rejects_match_else_arm_syntax() {
    let output = parse_text(
        r#"enum AppError {
    missing_path
}

func main(): i32 {
    return 0
}

func code(error: AppError): i32 {
    match error {
        AppError.missing_path {
            return 1
        }

        else {
            return 0
        }
    }
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1);
    assert!(output.diagnostics[0].message.contains("_ { ... }"));
}

#[test]
fn parses_range_for_statement() {
    let output = parse_text(
        r#"func main(): i32 {
    for i in 0..<4 {
        use_value(i)
    }

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::ForRange(statement) = &function.body.statements[0] else {
        panic!("expected range for statement");
    };

    assert_eq!(statement.name, "i");
    assert!(matches!(statement.start, Expr::IntegerLiteral(_)));
    assert!(matches!(statement.end, Expr::IntegerLiteral(_)));
    assert!(statement.body.statements.is_empty());
    assert!(matches!(
        statement.body.result.as_deref(),
        Some(Expr::Call(_))
    ));
}

#[test]
fn rejects_non_range_for_statement() {
    let output = parse_text(
        r#"func main(): void {
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
        r#"func main(): void {
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
        r#"func main(): i32 {
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
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::If(statement) = &function.body.statements[0] else {
        panic!("expected if statement");
    };
    assert!(matches!(statement.condition, Expr::BoolLiteral(_)));
    assert!(statement.else_block.is_some());
}
