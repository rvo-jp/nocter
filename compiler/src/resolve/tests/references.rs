use super::support::resolve_text;
use crate::resolve::LocalSymbolKind;

#[test]
fn resolves_direct_function_calls() {
    let output = resolve_text(
        r#"func main(): i32 {
    return answer()
}

func answer(): i32 {
    return 1
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.call_targets.len(), 1);
    let symbol = output
        .call_targets
        .values()
        .next()
        .and_then(|id| output.symbols.get(*id))
        .unwrap();
    assert_eq!(symbol.name, "answer");
}

#[test]
fn resolves_local_identifier_references() {
    let output = resolve_text(
        r#"func main(path: &str): i32 {
    let code = value(path)
    return code
}

func value(path: &str): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let locals = output.local_symbols().collect::<Vec<_>>();
    assert!(
        locals
            .iter()
            .any(|local| local.name == "path" && local.kind == LocalSymbolKind::Parameter)
    );
    assert!(locals.iter().any(|local| {
        local.name == "code" && local.kind == LocalSymbolKind::Binding(crate::ast::BindingKind::Let)
    }));

    let resolved_names = output
        .local_identifier_targets
        .values()
        .filter_map(|id| output.local_symbol(*id))
        .map(|local| local.name.as_str())
        .collect::<Vec<_>>();
    assert!(resolved_names.contains(&"path"));
    assert!(resolved_names.contains(&"code"));
}

#[test]
fn reports_unresolved_identifier_references() {
    let output = resolve_text(
        r#"func main(): i32 {
    return missing
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0416");
    assert!(output.diagnostics[0].message.contains("missing"));
}

#[test]
fn reports_unresolved_call_callees() {
    let output = resolve_text(
        r#"func main(): i32 {
    return missing()
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0416");
    assert!(output.diagnostics[0].message.contains("missing"));
}
