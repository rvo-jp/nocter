use super::*;

#[test]
fn one_recursive_type_grammar_recognizes_plain_callable_contracts_in_every_annotation_position() {
    for text in [
        "const CALLBACK: func(i32): i32 = named",
        "static CALLBACK: (func(i32): i32) = named",
        "struct Data { action: (func(): i32)?\nitems: [once(edit i32): i32; 2] }",
        "func nested(input: func(take func(i32): i32): func(): bool): void {}",
        "func result(): func(): func(): i32",
        "func local(): void { let value: ((mut(i32): void)) = named }",
        "func f(): void { let value = (input: (func(): i32)): (func(): i32) { input } }",
    ] {
        let parsed = tree(text, ParseGoal::V2CoreUnit);
        assert!(!parsed.has_errors(), "{text}: {:?}", parsed.diagnostics());
        assert!(
            parsed
                .nodes()
                .any(|(_, node)| node.kind() == NodeKind::CallableType)
        );
    }
}

#[test]
fn callable_result_chains_obey_the_same_parser_nesting_budget_as_delimited_type_recursion() {
    let text = format!("const CALLBACK: {}i32 = named", "func(): ".repeat(2048));
    let parsed = tree(&text, ParseGoal::V2CoreUnit);
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.kind() == crate::ParseDiagnosticKind::NestingLimit)
    );
}
