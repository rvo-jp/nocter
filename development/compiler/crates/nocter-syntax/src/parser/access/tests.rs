use std::{fs, path::PathBuf};

use nocter_json::Value;
use nocter_source::{SourceMap, SourceName};

use crate::{NodeKind, ParseDiagnosticKind, ParseGoal, SyntaxTree, parse, parse_reusable};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/v2-access")
}

fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    let Value::Object(members) = value else {
        panic!("expected fixture object")
    };
    &members
        .iter()
        .find(|member| member.name.as_ref() == name)
        .expect("required fixture field")
        .value
}

fn string(value: &Value) -> &str {
    let Value::String(value) = value else {
        panic!("expected fixture string")
    };
    value
}

fn parse_text(text: &str, goal: ParseGoal) -> SyntaxTree {
    let mut sources = SourceMap::new();
    let source = sources
        .add_bytes(SourceName::new("access.nct"), text.as_bytes())
        .unwrap();
    parse(sources.get(source).unwrap(), goal)
}

#[test]
fn manifest_sources_parse_or_reject_at_the_owning_stage() {
    let root = fixture_root();
    let manifest =
        nocter_json::parse(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(string(field(&manifest, "parse_goal")), "V2CoreUnit");
    let Value::Array(cases) = field(&manifest, "cases") else {
        panic!("expected fixture array")
    };
    let mut accepted = 0;
    let mut rejected = 0;
    for case in cases {
        let name = string(field(case, "file"));
        let source = fs::read_to_string(root.join(name)).unwrap();
        let tree = parse_text(&source, ParseGoal::V2CoreUnit);
        assert_eq!(tree.root().kind(), NodeKind::V2CoreUnit);
        super::super::snapshots::assert_token_projection(&tree, name);
        if string(field(case, "expectation")) == "syntax-reject" {
            assert!(tree.has_errors(), "accepted {name}");
            let primary = string(field(case, "primary_text"));
            let start = source.find(primary).unwrap();
            let end = start + primary.len();
            assert!(
                tree.diagnostics().iter().any(|diagnostic| {
                    let range = diagnostic.span().range();
                    let offset = range.start().get() as usize;
                    offset >= start && offset <= end
                }),
                "{name}: no diagnostic at {primary:?}: {:?}",
                tree.diagnostics()
            );
            rejected += 1;
        } else {
            // check-reject cases must parse: this test does not claim semantic acceptance.
            assert!(
                !tree.has_errors(),
                "{name}: {:?}; lexical: {:?}",
                tree.diagnostics(),
                tree.lexed().diagnostics()
            );
            let shape = super::super::snapshots::syntax_shape(&tree);
            let path = root.join(name).with_extension("shape");
            if std::env::var_os("NOCTER_UPDATE_ACCESS_SHAPES").is_some() {
                fs::write(&path, &shape).unwrap();
            }
            assert_eq!(shape, fs::read_to_string(path).unwrap(), "{name}");
            accepted += 1;
        }
    }
    assert_eq!((accepted, rejected), (44, 6));
}

#[test]
fn future_qualifiers_cannot_be_confused_with_released_borrow_nodes() {
    let source = "func edit(value: &i32): void {}\n";
    let current = parse_text(source, ParseGoal::SourceFile);
    let future = parse_text(source, ParseGoal::V2CoreUnit);
    assert!(!current.has_errors() && !future.has_errors());
    assert!(
        current
            .nodes()
            .any(|(_, node)| node.kind() == NodeKind::BorrowType)
    );
    assert!(
        !future
            .nodes()
            .any(|(_, node)| node.kind() == NodeKind::BorrowType)
    );
    assert!(
        future
            .nodes()
            .any(|(_, node)| node.kind() == NodeKind::AccessType)
    );
    assert_ne!(current.root().kind(), future.root().kind());
    assert!(
        parse_text(
            "func forward(value: ^i32): ^i32 { ^value }\n",
            ParseGoal::SourceFile
        )
        .has_errors()
    );
}

#[test]
fn postfix_precedes_transfer_and_grouping_preserves_direct_operand_restriction() {
    let tree = parse_text(
        "func f(): void { let value = ^make() }\n",
        ParseGoal::V2CoreUnit,
    );
    assert!(!tree.has_errors());
    let transfer = tree
        .nodes()
        .find(|(_, node)| node.kind() == NodeKind::TransferExpression)
        .unwrap()
        .0;
    assert!(
        crate::child_node_iter(&tree, transfer)
            .any(|child| tree.node(child).unwrap().kind() == NodeKind::PostfixExpression)
    );
    for operand in ["&value", "(^value)", "((( &value )))"] {
        let text = format!("func f(): void {{ take({operand}) }}\n");
        let tree = parse_text(&text, ParseGoal::V2CoreUnit);
        assert!(tree.has_errors(), "{text}");
    }
    for operand in ["if true { ^left } else { ^right }", "-^value", "&value + 1"] {
        let text = format!("func f(): void {{ take({operand}) }}\n");
        assert!(
            !parse_text(&text, ParseGoal::V2CoreUnit).has_errors(),
            "{text}"
        );
    }
}

#[test]
fn grammar_edges_and_recovery_are_lossless_without_acceptance_fallback() {
    for source in [
        "",
        "\n",
        "func f(): void {}\n",
        "func f(a: i32,): void { return }\n",
        "func f(\na: i32,\n\nb: &i32,\n): void {\nreturn\n}\n",
        "func f(): ^void {\nlet value = (\n\n)\nreturn ^value\n}\n",
        "func f(): void {\nlet value = &&source\n}\n",
        "func f(): void {\nlet drop = 1\ndrop(drop)\n}\n",
        "func f(): void { if true {} else if false {} else {} }\n",
        "func f(): void { take(1,\n\n2,) }\n",
        "func f(): void {\nlet value = 1 +\n2 * 3 - 4\n}\n",
    ] {
        let tree = parse_text(source, ParseGoal::V2CoreUnit);
        assert!(!tree.has_errors(), "{source:?}: {:?}", tree.diagnostics());
        super::super::snapshots::assert_token_projection(&tree, source);
    }
    for source in [
        "func f(): void { let value = }\n",
        "func f(a: i32 b: i32): void {}\n",
        "func f(): void { let value = 1 < 2 < 3 }\n",
        "func f(): void { let value = true == false == true }\n",
        "func f(): void { let value = (1, 2) }\n",
        "func f(): void { let value = &+source }\n",
        "pub func f(): void {}\n",
        "noalloc func f(): void {}\n",
        "async func f(): void {}\n",
        "func f(): void { let value = move source }\n",
        "func f(): void { use std/io }\n",
        "func f(value: &+i32): void {}\n",
        "func f(): void { let (a, b) = pair }\n",
        "func f(): void { take(1,,2) }\n",
        "func f(): void {\nlet value = 1 +\n\n2\n}\n",
    ] {
        let tree = parse_text(source, ParseGoal::V2CoreUnit);
        assert!(tree.has_errors(), "accepted {source:?}");
        super::super::snapshots::assert_token_projection(&tree, source);
    }
}

#[test]
fn deep_grouping_and_conditionals_stop_at_the_shared_nesting_limit() {
    for body in [
        format!("{}0{}", "(".repeat(300), ")".repeat(300)),
        format!("{}true{}", "if ".repeat(300), " {}".repeat(300)),
    ] {
        let text = format!("func f(): void {{ {body} }}\n");
        let tree = parse_text(&text, ParseGoal::V2CoreUnit);
        assert!(
            tree.diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.kind() == ParseDiagnosticKind::NestingLimit)
        );
        super::super::snapshots::assert_token_projection(&tree, "deep access input");
    }
}

#[test]
fn reusable_future_parse_rebinds_without_changing_the_goal_or_shape() {
    let text = "func f(value: ^i32): ^i32 { return ^value }\n";
    let mut first = SourceMap::new();
    let first_id = first
        .add_bytes(SourceName::new("first.nct"), text.as_bytes())
        .unwrap();
    let product = parse_reusable(first.get(first_id).unwrap(), ParseGoal::V2CoreUnit);
    let mut second = SourceMap::new();
    second
        .add_bytes(SourceName::new("prefix.nct"), b"")
        .unwrap();
    let second_id = second
        .add_bytes(SourceName::new("second.nct"), text.as_bytes())
        .unwrap();
    let rebound = product.bind(second.get(second_id).unwrap()).unwrap();
    assert_eq!(rebound.source(), second_id);
    assert_eq!(rebound.root().kind(), NodeKind::V2CoreUnit);
    assert_eq!(
        super::super::snapshots::syntax_shape(&rebound),
        super::super::snapshots::syntax_shape(&parse_text(text, ParseGoal::V2CoreUnit))
    );
}
