use super::*;

#[test]
fn explicit_generic_calls_keep_their_named_or_member_callee_and_one_argument_tree() {
    for (body, lists, shifts) in [
        ("decode<Config>(value)?", 1, 0),
        ("receiver^.decode<Config>(value)?", 1, 0),
        ("decode<Table<Row, (WIDTH >> 1)>>(value)?", 2, 1),
        ("decode<WIDTH < LIMIT>(value)?", 1, 0),
        ("decode<\nTable<Row>,\n>(value)?", 2, 0),
    ] {
        let text = format!("func run(): i32! {{ {body} }}");
        let parsed = tree(&text, ParseGoal::V2CoreUnit);
        assert!(!parsed.has_errors(), "{text}: {:?}", parsed.diagnostics());
        let (call, _) = parsed
            .nodes()
            .find(|(_, node)| node.kind() == NodeKind::CallSuffix)
            .unwrap();
        assert!(crate::direct_node(&parsed, call, NodeKind::GenericArguments).is_some());
        assert_eq!(
            parsed
                .nodes()
                .filter(|(_, node)| node.kind() == NodeKind::GenericArguments)
                .count(),
            lists
        );
        assert_eq!(
            parsed
                .nodes()
                .filter(|(_, node)| node.kind() == NodeKind::ShiftExpression)
                .count(),
            shifts
        );
    }
}

#[test]
fn unterminated_angle_lists_finish_recovery_before_the_enclosing_block() {
    for (suffix, calls) in [("", 0), ("\nlet later = decode<i32>(0)\n", 1)] {
        let text = format!("func run(): bool {{ {}last{suffix} }}", "name<".repeat(48));
        let parsed = tree(&text, ParseGoal::V2CoreUnit);
        assert!(parsed.has_errors());
        assert_eq!(
            parsed
                .nodes()
                .filter(|(_, node)| node.kind() == NodeKind::CallSuffix)
                .count(),
            calls
        );
    }
}

#[test]
fn incomplete_or_separated_angle_lists_do_not_consume_a_comparison_operand() {
    for (body, parses) in [
        ("left<right", true),
        ("left < (right)", true),
        ("decode <Config>(value)", false),
        ("decode<Config>\n(value)", false),
    ] {
        let text = format!("func run(): bool {{ {body} }}");
        let parsed = tree(&text, ParseGoal::V2CoreUnit);
        assert_eq!(
            !parsed.has_errors(),
            parses,
            "{text}: {:?}",
            parsed.diagnostics()
        );
        assert!(
            parsed
                .nodes()
                .all(|(_, node)| node.kind() != NodeKind::GenericArguments)
        );
        if parses {
            assert!(
                parsed
                    .nodes()
                    .any(|(_, node)| node.kind() == NodeKind::OrderingExpression)
            );
        }
    }
}

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
