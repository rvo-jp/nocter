use super::support::resolve_text;
use crate::ast::LiteralShape;

#[test]
fn attaches_and_resolves_literal_by_nominal_identity_and_shape() {
    let output = resolve_text(
        r#"struct Vec<T> {}

literal Vec<T> [](...items: T): Self {}

func build(): Vec<i32> {
    return Vec<i32> [1, 2]
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let target = output.type_symbol_by_name("Vec").expect("expected Vec");
    assert_eq!(target.literals.len(), 1);
    assert_eq!(target.literals[0].shape, LiteralShape::Sequence);
    assert_eq!(output.typed_literal_targets.len(), 1);
}

#[test]
fn rejects_duplicate_literal_shape_for_one_target() {
    let output = resolve_text(
        r#"struct Text {}

literal Text ""(text: &str): Self {}
literal Text ""(text: &str): Self {}
"#,
    );

    assert!(
        output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0421")
    );
}

#[test]
fn rejects_non_nominal_and_mismatched_generic_targets() {
    let alias = resolve_text(
        r#"type Words = [u8]
literal Words [](...items: u8): Self {}
"#,
    );
    assert!(
        alias
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("nominal struct or enum"))
    );

    let generic = resolve_text(
        r#"struct Vec<T> {}
literal Vec<U> [](...items: U): Self {}
"#,
    );
    assert!(generic.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("generic argument `U` must be `T`")
    }));
}

#[test]
fn diagnoses_missing_literal_definition_at_expression() {
    let output = resolve_text(
        r#"struct Vec<T> {}

func build(): Vec<i32> {
    return Vec<i32> [1]
}
"#,
    );

    assert!(
        output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0422")
    );
}
