use crate::{NodeKind, ParseGoal, SyntaxTree, parse, parse_reusable};
use nocter_json::Value;
use nocter_source::{SourceMap, SourceName};
use std::{fs, path::PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/value-bindings")
}
fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    let Value::Object(members) = value else {
        panic!("object")
    };
    &members
        .iter()
        .find(|member| member.name.as_ref() == name)
        .unwrap()
        .value
}
fn string(value: &Value) -> &str {
    let Value::String(text) = value else {
        panic!("string")
    };
    text
}
fn tree(text: &str, goal: ParseGoal) -> SyntaxTree {
    let mut sources = SourceMap::new();
    let id = sources
        .add_bytes(SourceName::new("binding.nct"), text.as_bytes())
        .unwrap();
    parse(sources.get(id).unwrap(), goal)
}
#[test]
fn corpus_uses_the_production_goal_with_lossless_tokens_and_structural_snapshots() {
    let root = root();
    let manifest =
        nocter_json::parse(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    let Value::Array(cases) = field(&manifest, "cases") else {
        panic!("cases")
    };
    let mut counts = [0; 2];
    for case in cases {
        let name = string(field(case, "file"));
        let text = fs::read_to_string(root.join(name)).unwrap();
        let parsed = tree(&text, ParseGoal::V2CoreUnit);
        super::super::snapshots::assert_token_projection(&parsed, name);
        assert_eq!(parsed.root().kind(), NodeKind::V2CoreUnit);
        if string(field(case, "expectation")) == "syntax-reject" {
            assert!(parsed.has_errors(), "{name}");
            counts[1] += 1;
        } else {
            assert!(!parsed.has_errors(), "{name}: {:?}", parsed.diagnostics());
            let shape = super::super::snapshots::syntax_shape(&parsed);
            let path = root.join(name).with_extension("shape");
            if std::env::var_os("NOCTER_UPDATE_ACCESS_SHAPES").is_some() {
                fs::write(&path, &shape).unwrap();
            }
            assert_eq!(shape, fs::read_to_string(path).unwrap(), "{name}");
            counts[0] += 1;
        }
    }
    assert_eq!(counts, [26, 4]);
}
#[test]
fn operation_nodes_preserve_contextual_names_without_type_lookup() {
    let text = "func edit(edit: i32, take take: i32): i32 {\nvar read = edit\nread = take\nread observed = read\nreturn observed\n}\n";
    let parsed = tree(text, ParseGoal::V2CoreUnit);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    assert_eq!(
        parsed
            .nodes()
            .filter(|(_, n)| n.kind() == NodeKind::InputOperation)
            .count(),
        1
    );
    assert_eq!(
        parsed
            .nodes()
            .filter(|(_, n)| n.kind() == NodeKind::BindingOperation)
            .count(),
        2
    );
    super::super::snapshots::assert_token_projection(&parsed, "contextual");
}
#[test]
fn retired_words_are_names_only_in_the_future_profile_and_throw_is_reserved() {
    for name in [
        "pub",
        "construct",
        "instance",
        "move",
        "noalloc",
        "notrap",
        "blocking",
    ] {
        let text = format!("func {name}(value: i32): i32 {{ return value }}\n");
        assert!(!tree(&text, ParseGoal::V2CoreUnit).has_errors(), "{text}");
        assert!(tree(&text, ParseGoal::SourceFile).has_errors(), "{text}");
    }
    assert!(tree("func throw(): void {}\n", ParseGoal::V2CoreUnit).has_errors());
    assert!(!tree("func throw(): void {}\n", ParseGoal::SourceFile).has_errors());
}
#[test]
fn no_qualifier_or_call_marker_path_and_no_released_grammar_fallback() {
    for text in [
        "func f(x: &i32): void {}",
        "func f(x: ^i32): void {}",
        "func f(x: i32): i32 { return ^x }",
        "func f(x: i32): void { f(&x) }",
        "func f(x: i32): void { f((^x)) }",
        "func f(x: i32): void { let y = &&x }",
        "func f<T>(x: T): T { return x }",
        "func f(): i32! { return 0 }",
        "func f(): void { let c = (x) { x } }",
    ] {
        assert!(tree(text, ParseGoal::V2CoreUnit).has_errors(), "{text}");
    }
    assert!(!tree("func f(x: &i32): void {}", ParseGoal::SourceFile).has_errors());
}
#[test]
fn unary_and_binary_precedence_retains_the_original_tokens() {
    let text =
        "func f(value: i32, ready: bool): bool { return !ready || value + -2 * 3 < 10 && ready }";
    let parsed = tree(text, ParseGoal::V2CoreUnit);
    assert!(!parsed.has_errors());
    super::super::snapshots::assert_token_projection(&parsed, "precedence");
    for kind in [
        NodeKind::LogicalOrExpression,
        NodeKind::LogicalAndExpression,
        NodeKind::OrderingExpression,
        NodeKind::AdditiveExpression,
        NodeKind::MultiplicativeExpression,
        NodeKind::UnaryExpression,
    ] {
        assert!(parsed.nodes().any(|(_, n)| n.kind() == kind), "{kind:?}");
    }
}
#[test]
fn reusable_parse_rebinds_only_equal_normalized_text() {
    let mut sources = SourceMap::new();
    let text = "func f(take value: error): error { let next = value\nreturn next }\n";
    let first = sources
        .add_bytes(SourceName::new("one.nct"), text.as_bytes())
        .unwrap();
    let second = sources
        .add_bytes(SourceName::new("two.nct"), text.as_bytes())
        .unwrap();
    let changed = sources
        .add_bytes(
            SourceName::new("changed.nct"),
            text.replace("take value", "value").as_bytes(),
        )
        .unwrap();
    let query = parse_reusable(sources.get(first).unwrap(), ParseGoal::V2CoreUnit);
    assert!(query.bind(sources.get(second).unwrap()).is_some());
    assert!(query.bind(sources.get(changed).unwrap()).is_none());
}
#[test]
fn nesting_limit_recovers_without_losing_the_source_projection() {
    let text = format!(
        "func f(): i32 {{ return {}0{} }}",
        "(".repeat(500),
        ")".repeat(500)
    );
    let parsed = tree(&text, ParseGoal::V2CoreUnit);
    assert!(parsed.has_errors());
    super::super::snapshots::assert_token_projection(&parsed, "depth");
}
