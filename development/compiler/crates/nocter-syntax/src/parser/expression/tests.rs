use nocter_source::{SourceMap, SourceName};

use crate::{NodeKind, ParseGoal, SyntaxElement, SyntaxTree, parse};

fn parse_module(text: &str) -> SyntaxTree {
    let mut sources = SourceMap::new();
    let source = sources
        .add_bytes(SourceName::new("expression.nct"), text.as_bytes())
        .unwrap();
    parse(sources.get(source).unwrap(), ParseGoal::SourceFile)
}

fn assert_ok(text: &str) {
    let tree = parse_module(text);
    assert!(tree.lexed().diagnostics().is_empty());
    assert!(tree.diagnostics().is_empty(), "{:?}", tree.diagnostics());
    assert_token_projection(&tree);
}

fn count_nodes(tree: &SyntaxTree, expected: NodeKind) -> usize {
    let mut count = 0;
    let mut pending = vec![tree.root_id()];
    while let Some(node) = pending.pop() {
        if tree.node(node).unwrap().kind() == expected {
            count += 1;
        }
        for child in tree.children(node) {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
        }
    }
    count
}

fn assert_token_projection(tree: &SyntaxTree) {
    let mut pieces = vec![Vec::new(); tree.lexed().tokens().len()];
    let mut pending = vec![tree.root_id()];
    while let Some(node) = pending.pop() {
        for child in tree.children(node) {
            match child {
                SyntaxElement::Node(child) => pending.push(*child),
                SyntaxElement::Token(token) => {
                    pieces[token.lexical().index()].push(token.range());
                }
                SyntaxElement::Missing(_) => {}
            }
        }
    }
    for (token, mut ranges) in tree.lexed().tokens().iter().zip(pieces) {
        ranges.sort_by_key(|range| range.start());
        assert_eq!(
            ranges.first().unwrap().start(),
            token.span().range().start()
        );
        assert_eq!(ranges.last().unwrap().end(), token.span().range().end());
        assert!(
            ranges
                .windows(2)
                .all(|pair| pair[0].end() == pair[1].start())
        );
    }
}

#[test]
fn parses_block_imports_statements_and_body_results() {
    assert_ok(
        "func flow(input: i32): i32 {\n    use std/io\n\n    let value: i32 = input\n    var total = 0\n    total\n        += value\n    drop input\n    total\n}\nfunc stop(): void { return }\nfunc repeat(): void { loop {\n    break\n    continue\n} }\n",
    );
}

#[test]
fn parses_precedence_conversion_and_outcome_layers() {
    assert_ok(
        "func calculate<T>(left: T, right: T, scale: T, expected: bool, ready: bool, fallback: bool, result: T?): bool {\n    let arithmetic = left + right * scale << 1\n    let logic = arithmetic == left && ready || fallback\n    let view = &left as &View\n    let explicit = (left == right) == expected\n    let nested = (move result?)?\n    logic\n}\n",
    );
}

#[test]
fn parses_construction_strings_closures_and_recovery() {
    assert_ok(
        "func build<T>(source: T, owned: T, value: i32, limit: i32, arena: Allocator): i32 {\n    let user = parser.User<T> { id: value }\n    let made = Vec<T>.new()\n    let present = parser.Option<T>.some(value)\n    let fixed = [1, 2, 3]\n    let grown = Vec [1, ...&source, ...move owned] using arena\n    let text = String \"hello\" using arena\n    let rendered = \"value: ${value}\"\n    let positive = (&limit; item: i32): bool { item > limit }\n    let recovered = load() catch error { recover(error) } otherwise { default_value() }\n    value\n}\n",
    );
}

#[test]
fn parses_control_headers_without_searching_for_a_later_brace() {
    assert_ok(
        "func control(flags: Flags): void {\n    if Empty {}\n    if (Flags { ready: true }).ready {}\n    if ((value) { value > 0 }) {}\n    while (if ready() { true } else { false }) {}\n    for index in 0..<count {}\n    for item in &values {}\n    region temporary using arenas.temp {}\n    match flags {\n        Flags.ready {}\n        Flags.value(item) {}\n        _ {}\n    }\n}\n",
    );
}

#[test]
fn rejects_closed_expression_and_statement_boundaries() {
    for source in [
        "func invalid(): void { value?? }\n",
        "func invalid(): void { move make_value() }\n",
        "func invalid(): void { left < middle < right }\n",
        "func invalid(): void { left == middle == right }\n",
        "func invalid<T>(): void { parser.Option<T> }\n",
        "func invalid(): void { Vec [...&+source] }\n",
        "func invalid(): void { String \"value: ${value}\" }\n",
        "func invalid(): void { (; value) { value } }\n",
        "func invalid(): void { drop value.field }\n",
        "func invalid(): void { value\nuse std/io }\n",
    ] {
        assert!(parse_module(source).has_errors(), "{source}");
    }
}

#[test]
fn classifies_only_declared_continuation_newlines() {
    let tree = parse_module(
        "func lines(left: i32, right: i32, value: State): i32 {\n    let sum = left\n        + right\n    let difference = left -\n        right\n    if value\n        is State.ready {}\n    for index in 0\n        ..<\n        count {}\n    value\n        .\n        field\n    left\n    -right\n}\n",
    );
    assert!(tree.diagnostics().is_empty(), "{:?}", tree.diagnostics());
    assert_eq!(count_nodes(&tree, NodeKind::AdditiveExpression), 2);
    assert_token_projection(&tree);

    assert!(
        parse_module("func invalid(left: i32, right: i32): i32 { let value = left\n\n+ right }")
            .has_errors()
    );
}

#[test]
fn flat_events_own_deep_left_associative_and_prefix_expressions() {
    let additive = std::iter::repeat_n("value", 5_000)
        .collect::<Vec<_>>()
        .join(" + ");
    let source = format!(
        "func deep(value: i32): i32 {{\n    let unary = {}value\n    {additive}\n}}\n",
        "&".repeat(5_000)
    );

    assert_ok(&source);
}

#[test]
fn contextual_discard_and_self_spellings_stay_closed() {
    for source in [
        "func _(): void {}\n",
        "struct Value { _: i32 }\n",
        "enum Value { _ }\n",
        "instance Self {}\n",
        "func generic<Self>(): void {}\n",
        "func value(): void { let item = _ }\n",
        "func value(): void { let Self = 1 }\n",
        "func value(): void { load() catch Self {} }\n",
        "func value(state: State): void { if state is State.some(Self) {} }\n",
    ] {
        assert!(parse_module(source).has_errors(), "{source}");
    }

    assert_ok(
        "func semantic(): Self\nfunc discards(state: State): void {\n    let _ = value()\n    load() catch _ {}\n    if state is State.some(_) {}\n}\n",
    );
}

#[test]
fn committed_forms_keep_their_identity_when_the_interior_is_malformed() {
    let tree = parse_module(
        "func invalid(): void {\n    value =\n}\nfunc closure(source: Value): void {\n    let first = (; value) { value }\n    let second = (&source value) { value }\n}\nfunc owner(): void {\n    let value = parser. {}\n}\n",
    );

    assert!(tree.has_errors());
    assert_eq!(count_nodes(&tree, NodeKind::AssignmentStatement), 1);
    assert_eq!(count_nodes(&tree, NodeKind::ClosureExpression), 2);
    assert_eq!(count_nodes(&tree, NodeKind::StructLiteral), 1);
    assert_token_projection(&tree);
}

#[test]
fn incomplete_expression_tokens_own_their_declared_newlines() {
    assert_ok(
        "func continuations(source: Values): void {\n    let value =\n        make()\n    let record = Record { value:\n\n        make() }\n    let values = Vec [...\n\n        &source]\n    let text = \"${\n\n        value\n    }\"\n}\n",
    );

    assert!(
        parse_module("func blank(): void { let value =\n\nnext() }").has_errors(),
        "a statement-level blank line cannot continue an initializer"
    );
}
