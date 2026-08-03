use super::support::{find_json_node, parse_text, parse_text_with_sources};
use crate::ast::{Expr, Item, LiteralShape, Stmt, TypeExpr};

#[test]
fn parses_sequence_literal_definition_and_pack_loop() {
    let (sources, output) = parse_text_with_sources(
        r#"pub literal Vec<T> [](...items: T): Self from current {
    for item in items {
        consume(move item)
    }
    return Self.empty()
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.expect("expected AST");
    let Item::Literal(literal) = &ast.items[0] else {
        panic!("expected literal definition");
    };
    assert_eq!(literal.shape, LiteralShape::Sequence);
    assert!(matches!(literal.target, TypeExpr::Generic(_)));
    let capture = literal.capture.as_ref().expect("expected element capture");
    assert_eq!(capture.name, "items");
    assert!(literal.parameters.parameters.is_empty());
    assert_eq!(
        literal.result_provenance.as_ref().unwrap().origins[0]
            .kind
            .source_label(),
        "current"
    );
    let Stmt::LiteralPackFor(loop_) = &literal.body.statements[0] else {
        panic!("expected literal-pack loop");
    };
    assert_eq!(loop_.name, "item");
    assert_eq!(loop_.pack_name, "items");

    let json = ast.to_json(&sources);
    assert!(find_json_node(&json, "literal_decl").is_some());
    assert!(find_json_node(&json, "result_provenance").is_some());
    assert!(find_json_node(&json, "literal_pack_for_statement").is_some());
}

#[test]
fn parses_typed_literals_with_context_overrides() {
    let output = parse_text(
        r#"func build(arena: Arena): void {
    let values = Vec<i32> [1, 2, 3] using arena
    let text = String "hello" using arena
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.expect("expected AST");
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function");
    };
    let Stmt::Binding(values) = &function.body.statements[0] else {
        panic!("expected values binding");
    };
    let Expr::TypedSequenceLiteral(values) = &values.initializer else {
        panic!("expected typed sequence literal");
    };
    assert_eq!(values.elements.len(), 3);
    assert!(values.using.is_some());

    let Stmt::Binding(text) = &function.body.statements[1] else {
        panic!("expected text binding");
    };
    let Expr::TypedStringLiteral(text) = &text.initializer else {
        panic!("expected typed string literal");
    };
    assert_eq!(text.text.value, "\"hello\"");
    assert!(text.using.is_some());
}

#[test]
fn keeps_adjacent_brackets_as_index_syntax() {
    let output = parse_text(
        r#"func first(values: Vec<i32>): i32 {
    return values[0]
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.expect("expected AST");
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function");
    };
    let Stmt::Return(statement) = &function.body.statements[0] else {
        panic!("expected return");
    };
    assert!(matches!(statement.expression, Some(Expr::Index(_))));
}

#[test]
fn rejects_sequence_spread_and_string_interpolation_in_phase_one() {
    let spread = parse_text(
        r#"func build(other: Vec<i32>): Vec<i32> {
    return Vec<i32> [...other]
}
"#,
    );
    assert!(spread.ast.is_none());
    assert!(spread.diagnostics[0].message.contains("spread"));

    let interpolation = parse_text(
        r#"func build(name: &str): String {
    return String "hello ${name}"
}
"#,
    );
    assert!(
        interpolation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("interpolation"))
    );
}
