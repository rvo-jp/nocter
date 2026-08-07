use super::support::{find_json_node, parse_text, parse_text_with_sources};
use crate::ast::{Item, MethodReceiverMode, ResultProvenanceOriginKind, TypeExpr, Visibility};

#[test]
fn parses_borrow_coercion_entries_and_json() {
    let (sources, output) = parse_text_with_sources(
        r#"coerce Vec<T> {
    pub &self as &[T] from self {
        return self.view()
    }

    &+self as &+[T] from self {
        return self.view_mut()
    }
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.expect("expected AST");
    let Item::Coerce(coerce) = &ast.items[0] else {
        panic!("expected coerce declaration");
    };
    assert!(matches!(coerce.target, TypeExpr::Generic(_)));
    assert_eq!(coerce.entries.len(), 2);
    assert_eq!(coerce.entries[0].visibility, Visibility::Public);
    assert_eq!(
        coerce.entries[0].receiver.mode,
        MethodReceiverMode::ReadonlyBorrow
    );
    assert_eq!(coerce.entries[1].visibility, Visibility::Private);
    assert_eq!(
        coerce.entries[1].receiver.mode,
        MethodReceiverMode::ReadwriteBorrow
    );
    assert_eq!(
        coerce.entries[0]
            .result_provenance
            .as_ref()
            .unwrap()
            .origins[0]
            .kind,
        ResultProvenanceOriginKind::Receiver
    );

    let json = ast.to_json(&sources);
    assert!(find_json_node(&json, "coerce_decl").is_some());
    assert!(find_json_node(&json, "coercion_entry").is_some());
    assert!(find_json_node(&json, "coerce_source_type").is_some());
    assert!(find_json_node(&json, "coerce_target_type").is_some());
}

#[test]
fn rejects_owned_coercion_receivers() {
    let output = parse_text(
        r#"coerce Text {
    pub self as &str from self { return self.view() }
}
"#,
    );

    assert!(output.ast.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("coercion receiver must be borrowed")
    }));
}

#[test]
fn rejects_top_level_visibility_and_nocter_entry_visibility() {
    let top_level = parse_text("pub coerce Text {}\n");
    assert!(top_level.ast.is_none());
    assert!(top_level.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("coerce` declarations do not use visibility modifiers")
    }));

    let entry = parse_text(
        r#"coerce Text {
    pub(nocter) &self as &str from self { return self.view() }
}
"#,
    );
    assert!(entry.ast.is_none());
    assert!(entry.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("coercion entries cannot use `pub(nocter)`")
    }));
}

#[test]
fn diagnoses_unclosed_coerce_blocks() {
    let output = parse_text("coerce Text {\n");
    assert!(output.ast.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("expected `}` to close coerce declaration")
    }));
}
