use super::support::{find_json_node, parse_text, parse_text_with_sources};
use crate::ast::{
    BinaryOperator, Expr, InterpolatedStringPart, Item, Stmt, TypeExpr, UnaryOperator,
};

#[test]
fn parses_optional_default_expression() {
    let output = parse_text(
        r#"use std/prelude

func main(): i32 {
    let user = (env("USER") catch error {
        return 1
    }) ?? "unknown"

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[1] else {
        panic!("expected function item");
    };
    let Stmt::Binding(binding) = &function.body.statements[0] else {
        panic!("expected binding statement");
    };
    let Expr::OptionalDefault(expression) = &binding.initializer else {
        panic!("expected optional default expression");
    };
    assert_eq!(expression.operator_span.len(), 2);
    assert!(expression.span.start < expression.operator_span.start);
    assert!(expression.operator_span.end < expression.span.end);
}

#[test]
fn parses_pattern_conditional_expression() {
    let output = parse_text(
        r#"enum AppError {
    missing_path
    open_failed(path: &str)
}

func code(error: AppError): i32 {
    return error ?{
        AppError.missing_path : 1
        AppError.open_failed(path) : path.len()
        : 0
    }
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[1] else {
        panic!("expected function item");
    };
    let Stmt::Return(statement) = &function.body.statements[0] else {
        panic!("expected return statement");
    };
    let Some(Expr::PatternConditional(expression)) = &statement.expression else {
        panic!("expected pattern conditional expression");
    };
    assert_eq!(expression.arms.len(), 2);
    assert_eq!(expression.arms[0].enum_name, "AppError");
    assert_eq!(expression.arms[0].variant_name, "missing_path");
    assert!(expression.arms[0].payload.is_none());
    assert_eq!(expression.arms[1].variant_name, "open_failed");
    assert_eq!(
        expression.arms[1]
            .payload
            .as_ref()
            .map(|payload| payload.name.as_str()),
        Some("path")
    );
    assert_eq!(expression.question_span.len(), 1);
}

#[test]
fn parses_force_unwrap_expression() {
    let output = parse_text(
        r#"func main(): i32 {
    return answer()!
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::Return(statement) = &function.body.statements[0] else {
        panic!("expected return statement");
    };
    let Some(Expr::Force(expression)) = &statement.expression else {
        panic!("expected force unwrap expression");
    };
    assert_eq!(expression.operator_span.len(), 1);
    assert!(expression.span.start < expression.operator_span.start);
    assert_eq!(expression.span.end, expression.operator_span.end);
}

#[test]
fn parses_borrow_expressions() {
    let output = parse_text(
        r#"func main(): i32 {
    let readonly = &value
    let readwrite = &+value
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::Binding(readonly) = &function.body.statements[0] else {
        panic!("expected binding statement");
    };
    let Expr::Borrow(readonly_borrow) = &readonly.initializer else {
        panic!("expected readonly borrow expression");
    };
    assert!(!readonly_borrow.is_readwrite);
    assert!(matches!(
        readonly_borrow.expression.as_ref(),
        Expr::Identifier(_)
    ));

    let Stmt::Binding(readwrite) = &function.body.statements[1] else {
        panic!("expected binding statement");
    };
    let Expr::Borrow(readwrite_borrow) = &readwrite.initializer else {
        panic!("expected readwrite borrow expression");
    };
    assert!(readwrite_borrow.is_readwrite);
    assert!(matches!(
        readwrite_borrow.expression.as_ref(),
        Expr::Identifier(_)
    ));
}

#[test]
fn diagnoses_named_arguments_as_deferred() {
    let output = parse_text(
        r#"func main(): i32 {
    return copy(source: "hello")
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains("named arguments are not part of v0")
    );
}

#[test]
fn ast_json_includes_expression_operator_spans() {
    let (sources, output) = parse_text_with_sources(
        r#"func main(): i32 {
    let value = maybe() ?? 0
    let handled = answer() catch error {
        return 1
    }
    return handled!
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let json = output.ast.unwrap().to_json(&sources);
    let optional_default = find_json_node(&json, "optional_default_expression")
        .expect("expected optional default expression");
    let optional_default_span = optional_default.operator_span.as_ref().unwrap();
    assert_eq!(
        optional_default_span.end_byte - optional_default_span.start_byte,
        2
    );

    let catch = find_json_node(&json, "fallible_catch_expression")
        .expect("expected fallible catch expression");
    let catch_span = catch.operator_span.as_ref().unwrap();
    assert_eq!(catch_span.end_byte - catch_span.start_byte, "catch".len());

    let force =
        find_json_node(&json, "force_unwrap_expression").expect("expected force unwrap expression");
    let force_span = force.operator_span.as_ref().unwrap();
    assert_eq!(force_span.end_byte - force_span.start_byte, 1);
}

#[test]
fn parses_array_literal_expression() {
    let output = parse_text(
        r#"func main(): i32 {
    let header = [
        0x7F,
        0x45,
        0x4C,
        0x46,
    ]
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::Binding(binding) = &function.body.statements[0] else {
        panic!("expected binding statement");
    };
    let Expr::ArrayLiteral(array) = &binding.initializer else {
        panic!("expected array literal");
    };
    assert_eq!(array.elements.len(), 4);
}

#[test]
fn parses_multi_line_string_literal_expression() {
    let output = parse_text(
        r#"func main(): &str {
    return """
        alpha
        beta
        """
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::Return(statement) = &function.body.statements[0] else {
        panic!("expected return statement");
    };
    let Some(Expr::StringLiteral(literal)) = &statement.expression else {
        panic!("expected string literal");
    };
    assert_eq!(
        literal.value,
        "\"\"\"\n        alpha\n        beta\n        \"\"\""
    );
}

#[test]
fn parses_interpolated_string_expression() {
    let (sources, output) = parse_text_with_sources(
        r#"func main(name: &str): i32 {
    let text = "hello ${name} ${1 + 2}"
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::Binding(binding) = &function.body.statements[0] else {
        panic!("expected binding statement");
    };
    let Expr::InterpolatedString(expression) = &binding.initializer else {
        panic!("expected interpolated string expression");
    };
    assert_eq!(expression.parts.len(), 4);
    let InterpolatedStringPart::Expression(name_part) = &expression.parts[1] else {
        panic!("expected interpolation expression");
    };
    let Expr::Identifier(identifier) = name_part.expression.as_ref() else {
        panic!("expected identifier interpolation");
    };
    assert_eq!(identifier.name, "name");
    let InterpolatedStringPart::Expression(sum_part) = &expression.parts[3] else {
        panic!("expected interpolation expression");
    };
    let Expr::Binary(binary) = sum_part.expression.as_ref() else {
        panic!("expected binary interpolation");
    };
    assert_eq!(binary.operator, BinaryOperator::Add);

    let json = ast.to_json(&sources);
    assert!(find_json_node(&json, "interpolated_string").is_some());
    assert!(find_json_node(&json, "string_interpolation").is_some());
}

#[test]
fn parses_struct_literal_expression() {
    let output = parse_text(
        r#"struct Point {
    x: i32
    label: &str
}

func main(): i32 {
    let point = Point{
        x: 1,
        label: "home",
    }
    return point.x
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[1] else {
        panic!("expected function item");
    };
    let Stmt::Binding(binding) = &function.body.statements[0] else {
        panic!("expected binding statement");
    };
    let Expr::StructLiteral(literal) = &binding.initializer else {
        panic!("expected struct literal");
    };
    assert_eq!(literal.fields.len(), 2);
    assert_eq!(literal.fields[0].name, "x");
    assert_eq!(literal.fields[1].name, "label");
}

#[test]
fn parses_generic_struct_literal_expression() {
    let output = parse_text(
        r#"struct Box<T> {
    value: T
}

func main(): i32 {
    let box = Box<i32>{
        value: 1,
    }
    return box.value
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[1] else {
        panic!("expected function item");
    };
    let Stmt::Binding(binding) = &function.body.statements[0] else {
        panic!("expected binding statement");
    };
    let Expr::StructLiteral(literal) = &binding.initializer else {
        panic!("expected struct literal");
    };
    let TypeExpr::Generic(ty) = &literal.ty else {
        panic!("expected generic struct literal type");
    };
    assert_eq!(ty.name, "Box");
    assert_eq!(ty.arguments.len(), 1);
    assert_eq!(literal.fields.len(), 1);
}

#[test]
fn parses_nested_generic_struct_literal_expression() {
    let output = parse_text(
        r#"struct Box<T> {
    value: T
}

func main(): i32 {
    let nested = Box<Box<i32>>{
        value: Box<i32>{
            value: 1,
        },
    }
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[1] else {
        panic!("expected function item");
    };
    let Stmt::Binding(binding) = &function.body.statements[0] else {
        panic!("expected binding statement");
    };
    let Expr::StructLiteral(literal) = &binding.initializer else {
        panic!("expected struct literal");
    };
    let TypeExpr::Generic(outer) = &literal.ty else {
        panic!("expected generic struct literal type");
    };
    assert_eq!(outer.name, "Box");
    assert_eq!(outer.arguments.len(), 1);
    assert!(matches!(outer.arguments[0], TypeExpr::Generic(_)));
    assert_eq!(literal.fields.len(), 1);
}

#[test]
fn parses_index_expression() {
    let output = parse_text(
        r#"func main(): i32 {
    let byte = header[0]
    let next = matrix[0][1]
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::Binding(first) = &function.body.statements[0] else {
        panic!("expected binding statement");
    };
    assert!(matches!(first.initializer, Expr::Index(_)));

    let Stmt::Binding(second) = &function.body.statements[1] else {
        panic!("expected binding statement");
    };
    let Expr::Index(outer) = &second.initializer else {
        panic!("expected outer index expression");
    };
    assert!(matches!(outer.object.as_ref(), Expr::Index(_)));
}

#[test]
fn parses_comparison_expressions() {
    let output = parse_text(
        r#"func main(): i32 {
    let nonempty = count > 0
    let same = bytes[0] == 0
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::Binding(first) = &function.body.statements[0] else {
        panic!("expected binding statement");
    };
    assert!(matches!(first.initializer, Expr::Binary(_)));

    let Stmt::Binding(second) = &function.body.statements[1] else {
        panic!("expected binding statement");
    };
    let Expr::Binary(binary) = &second.initializer else {
        panic!("expected binary expression");
    };
    assert_eq!(binary.operator, BinaryOperator::Equal);
}

#[test]
fn parses_arithmetic_expression_precedence() {
    let output = parse_text(
        r#"func main(): i32 {
    let value = 1 + 2 * 3 - 4 / 2 % 2
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::Binding(statement) = &function.body.statements[0] else {
        panic!("expected binding statement");
    };
    let Expr::Binary(subtract_expression) = &statement.initializer else {
        panic!("expected top-level subtraction expression");
    };
    assert_eq!(subtract_expression.operator, BinaryOperator::Subtract);

    let Expr::Binary(add_expression) = subtract_expression.left.as_ref() else {
        panic!("expected addition on the left side of subtraction");
    };
    assert_eq!(add_expression.operator, BinaryOperator::Add);

    let Expr::Binary(multiply_expression) = add_expression.right.as_ref() else {
        panic!("expected multiplication on the right side of addition");
    };
    assert_eq!(multiply_expression.operator, BinaryOperator::Multiply);

    let Expr::Binary(remainder_expression) = subtract_expression.right.as_ref() else {
        panic!("expected remainder on the right side of subtraction");
    };
    assert_eq!(remainder_expression.operator, BinaryOperator::Remainder);

    let Expr::Binary(divide_expression) = remainder_expression.left.as_ref() else {
        panic!("expected division on the left side of remainder");
    };
    assert_eq!(divide_expression.operator, BinaryOperator::Divide);
}

#[test]
fn parses_type_conversion_expression_precedence() {
    let output = parse_text(
        r#"func main(): i32 {
    let value = byte as u64 + 1
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::Binding(statement) = &function.body.statements[0] else {
        panic!("expected binding statement");
    };
    let Expr::Binary(add_expression) = &statement.initializer else {
        panic!("expected top-level addition expression");
    };
    assert_eq!(add_expression.operator, BinaryOperator::Add);
    assert!(matches!(
        add_expression.left.as_ref(),
        Expr::TypeConversion(_)
    ));
}

#[test]
fn parses_shift_expression_precedence() {
    let output = parse_text(
        r#"func main(): i32 {
    let outside = value + 1 << count * 2 < limit
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::Binding(statement) = &function.body.statements[0] else {
        panic!("expected binding statement");
    };
    let Expr::Binary(ordering_expression) = &statement.initializer else {
        panic!("expected top-level ordering expression");
    };
    assert_eq!(ordering_expression.operator, BinaryOperator::Less);

    let Expr::Binary(shift_expression) = ordering_expression.left.as_ref() else {
        panic!("expected shift expression on the left side of ordering expression");
    };
    assert_eq!(shift_expression.operator, BinaryOperator::ShiftLeft);

    let Expr::Binary(add_expression) = shift_expression.left.as_ref() else {
        panic!("expected addition on the left side of shift expression");
    };
    assert_eq!(add_expression.operator, BinaryOperator::Add);

    let Expr::Binary(multiply_expression) = shift_expression.right.as_ref() else {
        panic!("expected multiplication on the right side of shift expression");
    };
    assert_eq!(multiply_expression.operator, BinaryOperator::Multiply);
}

#[test]
fn parses_logical_expression_precedence() {
    let output = parse_text(
        r#"func main(): i32 {
    let condition = count > 0 && ready || fallback
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::Binding(statement) = &function.body.statements[0] else {
        panic!("expected binding statement");
    };
    let Expr::Binary(or_expression) = &statement.initializer else {
        panic!("expected top-level logical or expression");
    };
    assert_eq!(or_expression.operator, BinaryOperator::LogicalOr);

    let Expr::Binary(and_expression) = or_expression.left.as_ref() else {
        panic!("expected logical and on the left side of logical or");
    };
    assert_eq!(and_expression.operator, BinaryOperator::LogicalAnd);

    let Expr::Binary(ordering_expression) = and_expression.left.as_ref() else {
        panic!("expected ordering expression on the left side of logical and");
    };
    assert_eq!(ordering_expression.operator, BinaryOperator::Greater);
}

#[test]
fn parses_logical_not_expression_precedence() {
    let output = parse_text(
        r#"func main(): i32 {
    let condition = !ready && fallback
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::Binding(statement) = &function.body.statements[0] else {
        panic!("expected binding statement");
    };
    let Expr::Binary(and_expression) = &statement.initializer else {
        panic!("expected logical and expression");
    };
    assert_eq!(and_expression.operator, BinaryOperator::LogicalAnd);

    let Expr::Unary(not_expression) = and_expression.left.as_ref() else {
        panic!("expected logical not on the left side of logical and");
    };
    assert_eq!(not_expression.operator, UnaryOperator::LogicalNot);
}

#[test]
fn parses_numeric_negate_expression_precedence() {
    let output = parse_text(
        r#"func main(): i32 {
    let smaller = -count < 0
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::Binding(statement) = &function.body.statements[0] else {
        panic!("expected binding statement");
    };
    let Expr::Binary(ordering_expression) = &statement.initializer else {
        panic!("expected ordering expression");
    };
    assert_eq!(ordering_expression.operator, BinaryOperator::Less);

    let Expr::Unary(negate_expression) = ordering_expression.left.as_ref() else {
        panic!("expected numeric negation on the left side of ordering expression");
    };
    assert_eq!(negate_expression.operator, UnaryOperator::Negate);
}

#[test]
fn parses_move_expression() {
    let output = parse_text(
        r#"func main(): i32 {
    let next = move value
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function item");
    };
    let Stmt::Binding(statement) = &function.body.statements[0] else {
        panic!("expected binding statement");
    };
    let Expr::Unary(move_expression) = &statement.initializer else {
        panic!("expected move expression");
    };
    assert_eq!(move_expression.operator, UnaryOperator::Move);
    assert!(matches!(
        move_expression.operand.as_ref(),
        Expr::Identifier(identifier) if identifier.name == "value"
    ));
}
