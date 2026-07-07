use super::*;

use crate::ast::{
    BinaryOperator, Expr, ImplMember, Item, JsonAstNode, Stmt, TypeExpr, UnaryOperator, Visibility,
};
use crate::lexer::lex;
use crate::source::SourceMap;

fn parse_text(text: &str) -> ParseOutput {
    let (_, output) = parse_text_with_sources(text);
    output
}

fn parse_text_with_sources(text: &str) -> (SourceMap, ParseOutput) {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let lexed = lex(&sources, source);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    let output = parse(&sources, source, &lexed.tokens);
    (sources, output)
}

fn find_json_node<'a>(node: &'a JsonAstNode, kind: &str) -> Option<&'a JsonAstNode> {
    if node.kind == kind {
        return Some(node);
    }

    node.items
        .iter()
        .find_map(|child| find_json_node(child, kind))
}

#[test]
fn parses_hello_program() {
    let output = parse_text(
        r#"use std/prelude

from std/io import print

program(): i32 {
    print("Hello") catch error {
        return 1
    }

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    assert_eq!(ast.items.len(), 3);
    assert!(matches!(ast.items[0], Item::Use(_)));
    assert!(matches!(ast.items[1], Item::FromImport(_)));
    assert!(matches!(ast.items[2], Item::Program(_)));
}

#[test]
fn parses_import_aliases() {
    let output = parse_text(
        r#"import std/io as io
from std/io import File as StdFile, stdout
pub from std/string import String as StdString

program(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Import(import) = &ast.items[0] else {
        panic!("expected namespace import");
    };
    let Item::FromImport(from_import) = &ast.items[1] else {
        panic!("expected from import");
    };
    let Item::FromImport(reexport) = &ast.items[2] else {
        panic!("expected public re-export");
    };

    assert_eq!(import.path.value, "std/io");
    assert_eq!(import.alias.name, "io");
    assert_eq!(from_import.names[0].name, "File");
    assert_eq!(from_import.names[0].local_name(), "StdFile");
    assert_eq!(from_import.names[1].name, "stdout");
    assert_eq!(from_import.names[1].local_name(), "stdout");
    assert_eq!(reexport.visibility, Visibility::Public);
    assert_eq!(reexport.names[0].local_name(), "StdString");
}

#[test]
fn parses_impl_trait_methods_and_generic_bounds() {
    let output = parse_text(
        r#"pub struct Counter {
    value: i32
}

impl Counter {
    pub func zero(): i32 {
        return 0
    }

    pub method (counter: &+Self).add(value: i32): void {
        return
    }
}

pub trait Writer {
    method (writer: &+Self).write(text: str): void!
}

impl Writer for Counter {
    method (counter: &+Self).write(text: str): void! {
        return
    }
}

func print<W: Writer>(writer: &+W): void! {
    return
}

program(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();

    let Item::Impl(inherent_impl) = &ast.items[1] else {
        panic!("expected inherent impl");
    };
    assert!(inherent_impl.trait_ty.is_none());
    assert!(matches!(
        &inherent_impl.target_ty,
        TypeExpr::Reference(reference) if reference.name == "Counter"
    ));
    assert!(matches!(
        &inherent_impl.members[0],
        ImplMember::Function(function) if function.name == "zero"
    ));
    let ImplMember::Method(method) = &inherent_impl.members[1] else {
        panic!("expected method");
    };
    assert_eq!(method.name, "add");
    assert!(method.body.is_some());
    assert!(matches!(&method.receiver.ty, TypeExpr::Borrow(_)));

    let Item::Trait(trait_) = &ast.items[2] else {
        panic!("expected trait");
    };
    assert_eq!(trait_.visibility, Visibility::Public);
    assert_eq!(trait_.name, "Writer");
    assert_eq!(trait_.methods.len(), 1);
    assert_eq!(trait_.methods[0].name, "write");
    assert!(trait_.methods[0].body.is_none());

    let Item::Impl(trait_impl) = &ast.items[3] else {
        panic!("expected trait impl");
    };
    assert!(trait_impl.trait_ty.is_some());
    assert!(matches!(
        &trait_impl.target_ty,
        TypeExpr::Reference(reference) if reference.name == "Counter"
    ));

    let Item::Function(function) = &ast.items[4] else {
        panic!("expected generic function");
    };
    assert_eq!(function.generics.parameters.len(), 1);
    assert_eq!(function.generics.parameters[0].name, "W");
    assert!(matches!(
        &function.generics.parameters[0].bound,
        Some(TypeExpr::Reference(reference)) if reference.name == "Writer"
    ));
}

#[test]
fn parses_function_with_fallible_return_type() {
    let output = parse_text(
        r#"use std/prelude

from std/io import print

program(): i32 {
    run() catch error {
        return 1
    }

    return 0
}

func run(): void! {
    print("Hello")?
    return
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Some(Item::Function(function)) = ast.items.last() else {
        panic!("expected final item to be a function");
    };
    assert!(matches!(function.return_type, TypeExpr::Fallible(_)));
}

#[test]
fn parses_grouped_optional_fallible_return_type() {
    let output = parse_text(
        r#"func env(name: str): (str?)! {
    return none
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Some(Item::Function(function)) = ast.items.first() else {
        panic!("expected function item");
    };
    let TypeExpr::Fallible(fallible) = &function.return_type else {
        panic!("expected fallible return type");
    };
    assert!(matches!(fallible.success.as_ref(), TypeExpr::Optional(_)));
}

#[test]
fn parses_optional_default_expression() {
    let output = parse_text(
        r#"use std/prelude

program(): i32 {
    let user = (env("USER") catch error {
        return 1
    }) ?? "unknown"

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[1] else {
        panic!("expected program item");
    };
    let Stmt::Binding(binding) = &program.body.statements[0] else {
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
fn parses_force_unwrap_expression() {
    let output = parse_text(
        r#"program(): i32 {
    return answer()!
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::Return(statement) = &program.body.statements[0] else {
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
fn ast_json_includes_expression_operator_spans() {
    let (sources, output) = parse_text_with_sources(
        r#"program(): i32 {
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
fn parses_builtin_view_and_array_types() {
    let output = parse_text(
        r#"pub func checksum(bytes: [u8], output: [+u8], header: [u8; 4]): str {
    return "ok"
}

program(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function declaration");
    };

    assert!(matches!(
        &function.parameters.parameters[0].ty,
        TypeExpr::View(view) if !view.is_readwrite
    ));
    assert!(matches!(
        &function.parameters.parameters[1].ty,
        TypeExpr::View(view) if view.is_readwrite
    ));
    assert!(matches!(
        &function.parameters.parameters[2].ty,
        TypeExpr::Array(array) if array.length.value == "4"
    ));
    assert!(
        matches!(&function.return_type, TypeExpr::Reference(reference) if reference.name == "str")
    );
}

#[test]
fn parses_array_literal_expression() {
    let output = parse_text(
        r#"program(): i32 {
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
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::Binding(binding) = &program.body.statements[0] else {
        panic!("expected binding statement");
    };
    let Expr::ArrayLiteral(array) = &binding.initializer else {
        panic!("expected array literal");
    };
    assert_eq!(array.elements.len(), 4);
}

#[test]
fn parses_struct_literal_expression() {
    let output = parse_text(
        r#"struct Point {
    x: i32
    label: str
}

program(): i32 {
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
    let Item::Program(program) = &ast.items[1] else {
        panic!("expected program item");
    };
    let Stmt::Binding(binding) = &program.body.statements[0] else {
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
fn parses_index_expression() {
    let output = parse_text(
        r#"program(): i32 {
    let byte = header[0]
    let next = matrix[0][1]
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::Binding(first) = &program.body.statements[0] else {
        panic!("expected binding statement");
    };
    assert!(matches!(first.initializer, Expr::Index(_)));

    let Stmt::Binding(second) = &program.body.statements[1] else {
        panic!("expected binding statement");
    };
    let Expr::Index(outer) = &second.initializer else {
        panic!("expected outer index expression");
    };
    assert!(matches!(outer.object.as_ref(), Expr::Index(_)));
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

#[test]
fn parses_comparison_expressions() {
    let output = parse_text(
        r#"program(): i32 {
    let nonempty = count > 0
    let same = bytes[0] == 0
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::Binding(first) = &program.body.statements[0] else {
        panic!("expected binding statement");
    };
    assert!(matches!(first.initializer, Expr::Binary(_)));

    let Stmt::Binding(second) = &program.body.statements[1] else {
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
        r#"program(): i32 {
    let value = 1 + 2 * 3 - 4 / 2 % 2
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::Binding(statement) = &program.body.statements[0] else {
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
        r#"program(): i32 {
    let value = byte as u64 + 1
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::Binding(statement) = &program.body.statements[0] else {
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
        r#"program(): i32 {
    let outside = value + 1 << count * 2 < limit
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::Binding(statement) = &program.body.statements[0] else {
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
        r#"program(): i32 {
    let condition = count > 0 && ready || fallback
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::Binding(statement) = &program.body.statements[0] else {
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
        r#"program(): i32 {
    let condition = !ready && fallback
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::Binding(statement) = &program.body.statements[0] else {
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
        r#"program(): i32 {
    let smaller = -count < 0
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Program(program) = &ast.items[0] else {
        panic!("expected program item");
    };
    let Stmt::Binding(statement) = &program.body.statements[0] else {
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
fn parses_relative_import_paths() {
    let output = parse_text(
        r#"from ./config import Config
from ../shared/path import Path

program(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::FromImport(config) = &ast.items[0] else {
        panic!("expected first item to be a relative import");
    };
    let Item::FromImport(path) = &ast.items[1] else {
        panic!("expected second item to be a relative import");
    };

    assert_eq!(config.path.value, "./config");
    assert_eq!(config.visibility, Visibility::Private);
    assert_eq!(path.path.value, "../shared/path");
    assert_eq!(path.visibility, Visibility::Private);
}

#[test]
fn parses_public_reexports() {
    let output = parse_text(
        r#"pub from std/string import String

program(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::FromImport(import) = &ast.items[0] else {
        panic!("expected first item to be a public re-export");
    };

    assert_eq!(import.visibility, Visibility::Public);
    assert_eq!(import.names.len(), 1);
}

#[test]
fn parses_top_level_type_and_primitive_declarations() {
    let output = parse_text(
        r#"pub type Bytes = [u8]

pub copy struct Layout {
    size: usize
    align: usize
}

pub enum IOError {
    not_found(path: str)
    denied
}

pub(nocter) primitive addr<T>(pointer: *T): usize

pub func write(file: &+File, text: str): void! {
    return
}

program(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    assert_eq!(ast.items.len(), 6);

    let Item::TypeAlias(alias) = &ast.items[0] else {
        panic!("expected type alias");
    };
    assert_eq!(alias.visibility, Visibility::Public);
    assert!(matches!(&alias.target, TypeExpr::View(_)));

    let Item::Struct(struct_) = &ast.items[1] else {
        panic!("expected struct declaration");
    };
    assert!(struct_.is_copy);
    assert_eq!(struct_.fields.len(), 2);

    let Item::Enum(enum_) = &ast.items[2] else {
        panic!("expected enum declaration");
    };
    assert_eq!(enum_.variants.len(), 2);
    assert_eq!(enum_.variants[0].payload.len(), 1);

    let Item::Primitive(primitive) = &ast.items[3] else {
        panic!("expected primitive declaration");
    };
    assert_eq!(primitive.visibility, Visibility::Nocter);
    assert_eq!(primitive.generics.parameters.len(), 1);
    assert!(matches!(
        &primitive.parameters.parameters[0].ty,
        TypeExpr::Pointer(_)
    ));

    let Item::Function(function) = &ast.items[4] else {
        panic!("expected function declaration");
    };
    assert!(matches!(
        &function.parameters.parameters[0].ty,
        TypeExpr::Borrow(borrow) if borrow.is_readwrite
    ));
    assert!(matches!(&function.return_type, TypeExpr::Fallible(_)));
}

#[test]
fn diagnoses_unknown_top_level_item() {
    let output = parse_text(
        r#"module app/main

program(): i32 {
    return 0
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1);
    assert!(output.diagnostics[0].message.contains("top-level item"));
}
