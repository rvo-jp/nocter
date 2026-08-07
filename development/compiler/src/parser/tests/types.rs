use super::support::parse_text;
use crate::ast::{Item, TypeExpr};

#[test]
fn parses_function_with_fallible_return_type() {
    let output = parse_text(
        r#"use std/io.print

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
        r#"func env(name: &str): &str?! {
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
fn distinguishes_optional_borrow_from_borrow_of_optional() {
    let output = parse_text(
        r#"func optional_borrow(): &Item? {
    return none
}

func borrow_optional(value: Item?): &(Item?) {
    return &value
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(optional_borrow) = &ast.items[0] else {
        panic!("expected function declaration");
    };
    assert!(matches!(
        &optional_borrow.return_type,
        TypeExpr::Optional(optional)
            if matches!(optional.inner.as_ref(), TypeExpr::Borrow(_))
    ));

    let Item::Function(borrow_optional) = &ast.items[1] else {
        panic!("expected function declaration");
    };
    assert!(matches!(
        &borrow_optional.return_type,
        TypeExpr::Borrow(borrow)
            if matches!(borrow.inner.as_ref(), TypeExpr::Optional(_))
    ));
}

#[test]
fn parses_builtin_view_and_array_types() {
    let output = parse_text(
        r#"pub func checksum(bytes: &[u8], output: &+[u8], header: [u8; 4]): &str {
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
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite && matches!(borrow.inner.as_ref(), TypeExpr::View(view) if !view.is_readwrite)
    ));
    assert!(matches!(
        &function.parameters.parameters[1].ty,
        TypeExpr::Borrow(borrow)
            if borrow.is_readwrite && matches!(borrow.inner.as_ref(), TypeExpr::View(view) if !view.is_readwrite)
    ));
    assert!(matches!(
        &function.parameters.parameters[2].ty,
        TypeExpr::Array(array) if array.length.value == "4"
    ));
    assert!(matches!(
        &function.return_type,
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite && matches!(borrow.inner.as_ref(), TypeExpr::Reference(reference) if reference.name == "str")
    ));
}

#[test]
fn parses_builtin_callable_capability_types() {
    let output = parse_text(
        r#"func apply<
    Readonly: &func(i32): i32,
    Mutable: &+func(value: i32): i32,
    Once: func(source: &str): &str from source,
>(readonly: Readonly, mutable: Mutable, once: Once): void {
    return
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function declaration");
    };
    let bounds = function
        .generics
        .parameters
        .iter()
        .map(|parameter| &parameter.bounds[0])
        .collect::<Vec<_>>();
    assert!(matches!(
        bounds[0],
        TypeExpr::Callable(callable)
            if callable.capability == crate::ast::CallableCapability::Readonly
                && callable.parameters[0].name.is_none()
    ));
    assert!(matches!(
        bounds[1],
        TypeExpr::Callable(callable)
            if callable.capability == crate::ast::CallableCapability::Readwrite
                && callable.parameters[0].name.as_deref() == Some("value")
    ));
    assert!(matches!(
        bounds[2],
        TypeExpr::Callable(callable)
            if callable.capability == crate::ast::CallableCapability::Consuming
                && callable.result_provenance.is_some()
    ));
}

#[test]
fn rejects_removed_result_allocation_on_callable_types_contextually() {
    let output =
        parse_text("func apply<Factory: alloc &func(): Text>(factory: Factory): void { return }\n");
    assert!(output.ast.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("result `alloc` modifiers have been removed")
    }));

    let output = parse_text("func named(value: alloc): alloc { return value }\n");
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(named) = &ast.items[0] else {
        panic!("expected named function");
    };
    assert!(matches!(
        &named.parameters.parameters[0].ty,
        TypeExpr::Reference(reference) if reference.name == "alloc"
    ));
    assert!(matches!(
        &named.return_type,
        TypeExpr::Reference(reference) if reference.name == "alloc"
    ));
}
