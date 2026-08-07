use super::support::resolve_text;
use crate::ast::{MethodReceiverMode, Visibility};
use crate::resolve::{SymbolKind, TypeSymbol};

#[test]
fn indexes_type_owned_coercions_with_stable_contracts() {
    let output = resolve_text(
        r#"struct Vec<T> {}

coerce Vec<T> {
    pub &self as &[T] from self { return self.view() }
    &+self as &+[T] from self { return self.view_mut() }
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let SymbolKind::Type(TypeSymbol { coercions, .. }) =
        &output.symbols.symbol_by_name("Vec").unwrap().kind
    else {
        panic!("expected type symbol");
    };
    assert_eq!(coercions.len(), 2);
    assert_eq!(coercions[0].visibility, Visibility::Public);
    assert_eq!(
        coercions[0].receiver.mode,
        MethodReceiverMode::ReadonlyBorrow
    );
    assert_eq!(coercions[1].visibility, Visibility::Private);
    assert_eq!(
        coercions[1].receiver.mode,
        MethodReceiverMode::ReadwriteBorrow
    );
    assert!(coercions.iter().all(|coercion| coercion.is_accessible));
}

#[test]
fn rejects_non_borrowed_and_capability_strengthening_targets() {
    let output = resolve_text(
        r#"struct Text {}

coerce Text {
    pub &self as Text from self { return make() }
    pub &self as &+Text from self { return self }
}
"#,
    );

    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("coercion target must be a borrowed type or view")
    }));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("readonly coercion receiver cannot produce a readwrite target")
    }));
}

#[test]
fn accepts_elided_self_provenance_and_rejects_other_origins() {
    let elided = resolve_text(
        r#"struct Text {}
coerce Text {
    pub &self as &str { return self.view() }
}
"#,
    );
    assert!(elided.diagnostics.is_empty(), "{:?}", elided.diagnostics);

    let wrong = resolve_text(
        r#"struct Text {}
coerce Text {
    pub &self as &str from static { return "fixed" }
}
"#,
    );
    assert!(
        wrong
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("must be exactly `from self`"))
    );
}

#[test]
fn rejects_duplicate_coercion_keys_across_blocks() {
    let output = resolve_text(
        r#"struct Text {}
coerce Text {
    pub &self as &str from self { return self.first() }
}
coerce Text {
    pub &self as &str from self { return self.second() }
}
"#,
    );

    let duplicate = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("already defines coercion"))
        .expect("expected duplicate diagnostic");
    assert_eq!(duplicate.code, "E0465");
    assert_eq!(duplicate.notes.len(), 1);
}

#[test]
fn validates_source_generic_binding_order() {
    let output = resolve_text(
        r#"struct Pair<A, B> {}
coerce Pair<B, A> {
    pub &self as &[A] from self { return self.values() }
}
"#,
    );

    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("generic argument `B` must be `A`")
    }));
}
