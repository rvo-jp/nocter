use super::support::{find_json_node, parse_text, parse_text_with_sources};
use crate::ast::{ConstructMemberDecl, Expr, Item, LiteralShape, Stmt, TypeExpr};

#[test]
fn parses_sequence_literal_definition_and_pack_loop() {
    let (sources, output) = parse_text_with_sources(
        r#"construct Vec<T> {
    pub default literal [](...items: T): Self {
    for item in items {
        consume(move item)
    }
    return Self.empty()
    }
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.expect("expected AST");
    let Item::Construct(construct) = &ast.items[0] else {
        panic!("expected construct declaration");
    };
    let ConstructMemberDecl::Literal(literal) = &construct.members[0].declaration else {
        panic!("expected construct literal member");
    };
    assert_eq!(literal.shape, LiteralShape::Sequence);
    assert!(matches!(literal.target, TypeExpr::Generic(_)));
    let capture = literal.capture.as_ref().expect("expected element capture");
    assert_eq!(capture.name, "items");
    assert!(literal.parameters.parameters.is_empty());
    assert!(literal.result_provenance.is_none());
    let Stmt::LiteralPackFor(loop_) = &literal.body.statements[0] else {
        panic!("expected literal-pack loop");
    };
    assert_eq!(loop_.name, "item");
    assert_eq!(loop_.pack_name, "items");

    let json = ast.to_json(&sources);
    assert!(find_json_node(&json, "construct_literal_member").is_some());
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
fn rejects_top_level_literal_definitions_with_a_construct_migration() {
    let output = parse_text(
        r#"literal Vec<T> [](...items: T): Self {
    return Vec<T> {}
}
"#,
    );

    assert!(output.ast.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("declare the literal inside `construct Type { ... }`")
    }));
}

#[test]
fn parses_explicit_sequence_spread_modes_and_rejects_typed_string_interpolation() {
    let spread = parse_text(
        r#"func build(other: Vec<i32>): Vec<i32> {
    return Vec<i32> [0, ...other, ...&other, ...move other]
}
"#,
    );
    assert!(spread.diagnostics.is_empty(), "{:?}", spread.diagnostics);
    let ast = spread.ast.expect("expected spread AST");
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function")
    };
    let Stmt::Return(statement) = &function.body.statements[0] else {
        panic!("expected return")
    };
    let Expr::TypedSequenceLiteral(literal) = statement.expression.as_ref().unwrap() else {
        panic!("expected typed sequence literal")
    };
    assert_eq!(literal.elements.len(), 4);
    for element in &literal.elements[1..] {
        let Expr::Unary(spread) = element else {
            panic!("expected spread unary")
        };
        assert_eq!(spread.operator, crate::ast::UnaryOperator::Spread);
    }

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
