use super::support::resolve_text;
use crate::resolve::ConstructionEntryKind;

#[test]
fn attaches_one_default_construction_surface_to_its_nominal_type() {
    let output = resolve_text(
        r#"struct Vec<T> { value: T }

construct Vec<T> {
    pub default literal [](...items: T): Self {}
    pub func new(): Self {}
}

func build(): Vec<i32> {
    return Vec<i32> [1]
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let symbol = output.type_symbol_by_name("Vec").expect("expected Vec");
    assert_eq!(symbol.construction.entries.len(), 3);
    assert_eq!(symbol.construction.default_entry, Some(1));
    assert!(!symbol.construction.entries[0].is_accessible);
    assert_eq!(
        symbol.construction.entries[1].kind,
        ConstructionEntryKind::Literal(crate::ast::LiteralShape::Sequence)
    );
    assert_eq!(
        symbol.construction.entries[2].kind,
        ConstructionEntryKind::Function("new".to_string())
    );
    assert_eq!(output.typed_literal_targets.len(), 1);
}

#[test]
fn keeps_public_structural_construction_as_the_implicit_default() {
    let output = resolve_text(
        r#"struct Point { pub x: i32, pub y: i32 }

construct Point {
    pub func origin(): Self { return Point { x: 0, y: 0 } }
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let symbol = output.type_symbol_by_name("Point").expect("expected Point");
    assert_eq!(symbol.construction.default_entry, Some(0));
    assert!(symbol.construction.entries[0].is_accessible);
}

#[test]
fn requires_default_when_raw_structural_construction_is_not_public() {
    let output = resolve_text(
        r#"struct Token { value: i32 }

construct Token {
    pub func new(): Self { return Token { value: 0 } }
}
"#,
    );

    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "E0460" && diagnostic.message.contains("requires a default member")
    }));
}

#[test]
fn rejects_duplicate_defaults_construct_blocks_and_non_self_results() {
    let output = resolve_text(
        r#"struct Value { value: i32 }

construct Value {
    pub default func first(): Self { return Value { value: 1 } }
    pub default func wrong(): i32 { return 0 }
}

construct Value {}
"#,
    );

    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "E0460" && diagnostic.message.contains("only one default member")
    }));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "E0460" && diagnostic.message.contains("must produce `Self`")
    }));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "E0460" && diagnostic.message.contains("already has a construct")
    }));
}
