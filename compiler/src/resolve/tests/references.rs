use super::support::resolve_text;

#[test]
fn resolves_direct_function_calls() {
    let output = resolve_text(
        r#"program(): i32 {
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
