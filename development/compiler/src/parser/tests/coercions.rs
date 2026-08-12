use super::support::{find_json_node, parse_text, parse_text_with_sources};
use crate::ast::{Item, MethodReceiverMode, ResultProvenanceOriginKind, TypeExpr, Visibility};

#[test]
fn parses_borrow_coercion_entries_and_json() {
    let (sources, output) = parse_text_with_sources(
        r#"instance Vec<T> {
    pub coerce &self as &[T] from self {
        return self.view()
    }

    coerce &+self as &+[T] from self {
        return self.view_mut()
    }
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.expect("expected AST");
    let Item::Instance(instance) = &ast.items[0] else {
        panic!("expected instance declaration");
    };
    assert!(matches!(instance.target_ty, TypeExpr::Generic(_)));
    assert_eq!(instance.coercions.len(), 2);
    let first = instance.coercions[0].callable_method();
    let second = instance.coercions[1].callable_method();
    assert_eq!(first.visibility, Visibility::Public);
    assert_eq!(first.receiver.mode, MethodReceiverMode::ReadonlyBorrow);
    assert_eq!(second.visibility, Visibility::Private);
    assert_eq!(second.receiver.mode, MethodReceiverMode::ReadwriteBorrow);
    assert_eq!(
        first.result_provenance.as_ref().unwrap().origins[0].kind,
        ResultProvenanceOriginKind::Receiver
    );

    let json = ast.to_json(&sources);
    assert!(find_json_node(&json, "instance_decl").is_some());
    assert!(find_json_node(&json, "coercion_entry").is_some());
    assert!(find_json_node(&json, "instance_target_type").is_some());
    assert!(find_json_node(&json, "coerce_target_type").is_some());
}

#[test]
fn rejects_owned_coercion_receivers() {
    let output = parse_text(
        r#"instance Text {
    pub coerce self as &str from self { return self.view() }
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
fn rejects_standalone_declarations_and_accepts_package_entry_visibility() {
    let top_level = parse_text("pub coerce Text {}\n");
    assert!(top_level.ast.is_none());
    assert!(top_level.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("standalone `coerce` declarations were removed")
    }));

    let entry = parse_text(
        r#"instance Text {
    pub(/) coerce &self as &str from self { return self.view() }
}
"#,
    );
    let ast = entry
        .ast
        .expect("package-scoped coercion entry should parse");
    let crate::ast::Item::Instance(instance) = &ast.items[0] else {
        panic!("expected instance declaration");
    };
    assert_eq!(
        instance.coercions[0].callable_method().visibility,
        Visibility::Package
    );
}

#[test]
fn diagnoses_unclosed_coerce_blocks() {
    let output = parse_text("instance Text {\n    coerce &self as &str\n");
    assert!(output.ast.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("expected `}` to close instance block")
    }));
}

#[test]
fn parses_bodyless_public_coercion_contract() {
    let output = parse_text(
        r#"instance Text {
    pub coerce &self as &str from self
}
"#,
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.expect("expected AST");
    let Item::Instance(instance) = &ast.items[0] else {
        panic!("expected instance declaration");
    };
    assert!(instance.coercions[0].callable_method().body.is_none());
}

#[test]
fn parses_coercion_where_predicates_as_structural_requirements() {
    let output = parse_text("func view<T>(value: &T): &str where &T as &str { return value }\n");
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.expect("expected AST");
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function declaration");
    };
    let requirement = function
        .requirements
        .as_ref()
        .and_then(|clause| clause.coercion_requirements().next())
        .expect("expected coercion requirement");
    assert!(matches!(requirement.source, TypeExpr::Borrow(_)));
    assert!(matches!(requirement.target, TypeExpr::Borrow(_)));
}
