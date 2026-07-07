use super::support::parse_text;
use crate::ast::{Item, TypeExpr};

#[test]
fn parses_function_with_fallible_return_type() {
    let output = parse_text(
        r#"use std/prelude

from std/io import print

func main(): i32 {
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
fn parses_compact_optional_fallible_return_type() {
    let output = parse_text(
        r#"func env(name: str): str?! {
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
fn parses_builtin_view_and_array_types() {
    let output = parse_text(
        r#"pub func checksum(bytes: [u8], output: [+u8], header: [u8; 4]): str {
    return "ok"
}

func main(): i32 {
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
