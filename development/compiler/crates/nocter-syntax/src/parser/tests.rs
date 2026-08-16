use std::collections::BTreeMap;

use nocter_source::{SourceMap, SourceName};

use super::*;
use crate::SyntaxElement;

fn parse_text(text: &str, goal: ParseGoal) -> SyntaxTree {
    let mut sources = SourceMap::new();
    let id = sources
        .add_bytes(SourceName::new("test.nct"), text.as_bytes())
        .unwrap();
    parse(sources.get(id).unwrap(), goal)
}

fn assert_syntax_ok(text: &str, goal: ParseGoal) -> SyntaxTree {
    let tree = parse_text(text, goal);
    assert!(tree.lexed().diagnostics().is_empty());
    assert!(tree.diagnostics().is_empty(), "{:?}", tree.diagnostics());
    assert_complete_token_projection(&tree);
    tree
}

fn assert_complete_token_projection(tree: &SyntaxTree) {
    fn visit(tree: &SyntaxTree, ranges: &mut BTreeMap<usize, Vec<TextRange>>) {
        let mut pending = vec![tree.root_id()];
        while let Some(node) = pending.pop() {
            for child in tree.children(node) {
                match child {
                    SyntaxElement::Node(child) => pending.push(*child),
                    SyntaxElement::Token(token) => {
                        ranges
                            .entry(token.lexical().index())
                            .or_default()
                            .push(token.range());
                    }
                    SyntaxElement::Missing(_) => {}
                }
            }
        }
    }

    let mut projected = BTreeMap::new();
    visit(tree, &mut projected);
    assert_eq!(projected.len(), tree.lexed().tokens().len());

    for (index, lexical) in tree.lexed().tokens().iter().enumerate() {
        let mut ranges = projected.remove(&index).unwrap();
        ranges.sort_by_key(|range| range.start());
        assert_eq!(
            ranges.first().unwrap().start(),
            lexical.span().range().start()
        );
        assert_eq!(ranges.last().unwrap().end(), lexical.span().range().end());
        assert!(
            ranges
                .windows(2)
                .all(|pair| pair[0].end() == pair[1].start())
        );
    }
}

fn has_node_kind(tree: &SyntaxTree, expected: NodeKind) -> bool {
    let mut pending = vec![tree.root_id()];
    while let Some(node) = pending.pop() {
        if tree.node(node).is_some_and(|node| node.kind() == expected) {
            return true;
        }
        for child in tree.children(node) {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
        }
    }
    false
}

#[test]
fn parses_empty_and_nested_package_files() {
    assert_syntax_ok("", ParseGoal::PackageFile);
    assert_syntax_ok(
        "#name: \"example\"\n#version: \"0.1.0\"\n#dependencies: {\n    json: {\n        git: \"https://example.test/json.git\",\n    },\n}\n#test: { name: \"unit\", module: \"./tests/unit\", }\n",
        ParseGoal::PackageFile,
    );
    assert_syntax_ok(
        "#executable: {\n    name: \"app\"\n}\n",
        ParseGoal::PackageFile,
    );
}

#[test]
fn keeps_duplicate_directives_as_a_semantic_boundary() {
    assert_syntax_ok(
        "#name: \"first\"\n#name: \"second\"\n",
        ParseGoal::PackageFile,
    );
}

#[test]
fn rejects_non_data_and_interpolated_directive_values() {
    let boolean = parse_text("#name: true\n", ParseGoal::PackageFile);
    assert!(boolean.has_errors());

    let interpolation = parse_text("#name: \"${name}\"\n", ParseGoal::PackageFile);
    assert!(interpolation.has_errors());
}

#[test]
fn rejects_record_fields_without_commas() {
    let tree = parse_text(
        "#executable: {\n    name: \"app\"\n    module: \".\"\n}\n",
        ParseGoal::PackageFile,
    );

    assert!(tree.diagnostics().iter().any(|diagnostic| {
        diagnostic.kind()
            == ParseDiagnosticKind::Expected(ExpectedSyntax::Punctuation(Punctuation::Comma))
    }));
}

#[test]
fn eof_recovery_never_duplicates_the_eof_token() {
    let tree = parse_text("#", ParseGoal::PackageFile);
    assert!(tree.has_errors());
    assert_complete_token_projection(&tree);
}

#[test]
fn parses_private_scoped_and_selected_imports() {
    assert_syntax_ok(
        "use std/io\nuse /parser.Parser\nuse ../shared/path.{Path, normalize as clean,}\npub use ./public\npub(../../) use ./internal.Value\n",
        ParseGoal::ModuleSource,
    );
}

#[test]
fn rejects_empty_selection_and_namespace_alias() {
    let empty = parse_text("use ./parser.{}\n", ParseGoal::ModuleSource);
    assert!(empty.has_errors());

    let alias = parse_text("use std/io as console\n", ParseGoal::ModuleSource);
    assert!(alias.has_errors());
}

#[test]
fn enforces_the_lexical_module_segment_language() {
    for source in ["use Parser\n", "use _\n", "use std/type\n"] {
        let tree = parse_text(source, ParseGoal::ModuleSource);
        assert!(tree.diagnostics().iter().any(|diagnostic| {
            diagnostic.kind() == ParseDiagnosticKind::Expected(ExpectedSyntax::ModuleSegment)
        }));
        assert_complete_token_projection(&tree);
    }

    assert_syntax_ok("use package_2/source_file\n", ParseGoal::ModuleSource);
}

#[test]
fn recognizes_late_use_after_an_item_shaped_line() {
    let tree = parse_text(
        "func main(): void {}\nuse std/io\n",
        ParseGoal::ModuleSource,
    );

    assert!(
        tree.diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.kind() == ParseDiagnosticKind::LateUseDeclaration)
    );
}

#[test]
fn parses_callable_declarations_and_nested_type_closers() {
    assert_syntax_ok(
        "pub func choose<T>(left: &T, right: &T): &T from left | right\npub primitive invoke<T>(callback: &+func(input: &T): T, input: &T): T\ntype Nested<T> = parser.Outer<Inner<T>>\ntype DoubleBorrow<T> = &&T\n",
        ParseGoal::ModuleSource,
    );
}

#[test]
fn parses_opaque_and_layered_callable_results() {
    assert_syntax_ok(
        "func values<T>(): some Source<T, Item = &T>?\nfunc load<T>(): T?!\n",
        ParseGoal::ModuleSource,
    );
}

#[test]
fn keeps_bodyless_private_functions_as_a_semantic_boundary() {
    assert_syntax_ok("func contract(): i32\n", ParseGoal::ModuleSource);
}

#[test]
fn parses_target_attachment_and_empty_bodies() {
    assert_syntax_ok(
        "#target: \"arm64-darwin\"\npub func main(): void {}\n",
        ParseGoal::ModuleSource,
    );

    let same_line = parse_text(
        "#target: \"arm64-darwin\" func main(): void {}\n",
        ParseGoal::ModuleSource,
    );
    assert!(same_line.has_errors());
}

#[test]
fn rejects_empty_generics_reversed_outcomes_and_missing_results() {
    for source in [
        "type Empty<> = i32\n",
        "func reversed<T>(): T!?\n",
        "func missing()\n",
    ] {
        assert!(parse_text(source, ParseGoal::ModuleSource).has_errors());
    }
}

#[test]
fn parses_every_requirement_shape_without_type_driven_disambiguation() {
    assert_syntax_ok(
        "func constrained<T, U, C, I>(value: &T): T where T: Interface<U> + &func(&T): U, copy U, T.Item = U, (&T == &T): bool, (&T < &T): bool, (&C[usize]): &U, (&+C[usize]): &+U, &T as &str, (...&C): I\n",
        ParseGoal::ModuleSource,
    );
}

#[test]
fn copy_spelling_remains_a_capability_binder_before_colon() {
    assert_syntax_ok(
        "func constrained<copy>(): copy where copy: Interface\n",
        ParseGoal::ModuleSource,
    );
}

#[test]
fn rejects_missing_requirement_commas_and_undeclared_operator_shapes() {
    for source in [
        "func missing<T>(): T where copy T T: Interface\n",
        "func unsupported<T>(): T where (&T <= &T): bool\n",
        "func trailing<T>(): T where T: Interface,\n",
    ] {
        assert!(parse_text(source, ParseGoal::ModuleSource).has_errors());
    }
}

#[test]
fn flat_tree_owns_deep_non_recursive_prefix_syntax() {
    let source = format!("type Deep = {}i32\n", "*".repeat(10_000));
    let tree = assert_syntax_ok(&source, ParseGoal::ModuleSource);

    assert_eq!(tree.root().kind(), NodeKind::ModuleSource);
}

#[test]
fn recursive_delimiters_stop_at_the_declared_nesting_limit() {
    let source = format!("type TooDeep = {}i32{}\n", "(".repeat(300), ")".repeat(300));
    let tree = parse_text(&source, ParseGoal::ModuleSource);

    assert!(
        tree.diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.kind() == ParseDiagnosticKind::NestingLimit })
    );
    assert_complete_token_projection(&tree);
}

#[test]
fn nested_type_arguments_are_parsed_once_per_level() {
    let source = format!("type Deep = {}T{}\n", "Outer<".repeat(128), ">".repeat(128));

    assert_syntax_ok(&source, ParseGoal::ModuleSource);
}

#[test]
fn parses_structs_enums_and_semantically_empty_enums() {
    let tree = assert_syntax_ok(
        "pub copy struct Pair<T> where copy T {\n    pub left: T\n    right: T\n}\nenum Maybe<T> {\n    some(value: T)\n    missing\n}\nenum Empty {}\n",
        ParseGoal::ModuleSource,
    );

    for kind in [
        NodeKind::StructDeclaration,
        NodeKind::StructField,
        NodeKind::EnumDeclaration,
        NodeKind::EnumVariant,
        NodeKind::EnumPayload,
    ] {
        assert!(has_node_kind(&tree, kind));
    }
}

#[test]
fn rejects_commas_between_line_separated_nominal_members() {
    for source in [
        "struct Pair { left: i32, right: i32 }\n",
        "enum Maybe { some, none }\n",
    ] {
        assert!(parse_text(source, ParseGoal::ModuleSource).has_errors());
    }
}

#[test]
fn parses_interface_requirements_and_default_methods() {
    let tree = assert_syntax_ok(
        "interface Source<T> where copy T {\n    pub type Item: Iterable<T> + &func(T): T\n    pub method &+self.next(): Self.Item?\n    pub method self.consume(): void {}\n}\n",
        ParseGoal::ModuleSource,
    );

    assert!(has_node_kind(&tree, NodeKind::AssociatedTypeDeclaration));
    assert!(has_node_kind(&tree, NodeKind::InterfaceMethod));
    assert!(has_node_kind(&tree, NodeKind::SelfType));
}

#[test]
fn interface_members_require_bare_public_visibility() {
    for source in [
        "interface Source { method &self.read(): void }\n",
        "interface Source { pub(./) method &self.read(): void }\n",
    ] {
        assert!(parse_text(source, ParseGoal::ModuleSource).has_errors());
    }

    assert_syntax_ok(
        "interface Source { pub type Item\npub type Item }\n",
        ParseGoal::ModuleSource,
    );
}

#[test]
fn parses_construction_functions_and_both_literal_shapes() {
    let tree = assert_syntax_ok(
        "construct Vec<T> {\n    pub default literal [](...items: T): Self {}\n    pub literal \"\"(text: &str): Self\n    pub func with_capacity(capacity: usize): Self {}\n}\n",
        ParseGoal::ModuleSource,
    );

    assert!(has_node_kind(&tree, NodeKind::LiteralShape));
    assert!(has_node_kind(&tree, NodeKind::LiteralParameters));
    assert!(has_node_kind(&tree, NodeKind::ConstructionFunction));
}

#[test]
fn construction_members_require_visibility_but_defaults_remain_semantic() {
    assert!(
        parse_text(
            "construct Value { func new(): Self {} }\n",
            ParseGoal::ModuleSource
        )
        .has_errors()
    );
    assert_syntax_ok(
        "construct Value { pub default func first(): Self {}\npub default func second(): Self {} }\nconstruct External {}\n",
        ParseGoal::ModuleSource,
    );
}

#[test]
fn parses_every_instance_member_family() {
    let tree = assert_syntax_ok(
        "instance Text<T> where copy T {\n    pub method &self.len(): usize {}\n    coerce &self as &str from self {}\n    pub operator (&self == other: &Self): bool {}\n    pub operator (&self < other: &Self): bool {}\n    pub operator (&self[index: usize]): &T from self {}\n    pub operator (&+self[index: usize]): &+T from self {}\n    pub operator (...&self): Iterator<T> {}\n    pub operator (...self): Iterator<T> {}\n}\n",
        ParseGoal::ModuleSource,
    );

    for kind in [
        NodeKind::InherentMethod,
        NodeKind::CoercionDeclaration,
        NodeKind::EqualityOperator,
        NodeKind::OrderingOperator,
        NodeKind::IndexOperator,
        NodeKind::ExpansionOperator,
    ] {
        assert!(has_node_kind(&tree, kind));
    }
}

#[test]
fn rejects_closed_instance_and_pattern_forms() {
    for source in [
        "pub instance Value {}\n",
        "instance Buffer<i32> {}\n",
        "instance Buffer<error> {}\n",
        "instance Buffer<Self> {}\n",
        "instance Buffer<_> {}\n",
        "instance Value { operator (&self != other: &Self): bool {} }\n",
        "instance Value { coerce self as View {} }\n",
    ] {
        assert!(parse_text(source, ParseGoal::ModuleSource).has_errors());
    }
}

#[test]
fn parses_conformance_bindings_and_body_bearing_methods() {
    let tree = assert_syntax_ok(
        "conform Source<T> for Input<T> where copy T {\n    type Item = T\n    method &+self.next(): Self.Item? {}\n}\n",
        ParseGoal::ModuleSource,
    );

    assert!(has_node_kind(&tree, NodeKind::AssociatedTypeBinding));
    assert!(has_node_kind(&tree, NodeKind::ConformMethod));
}

#[test]
fn conformance_members_reject_visibility_and_missing_bodies() {
    for source in [
        "conform Source for Input { pub method &self.read(): void {} }\n",
        "conform Source for Input { method &self.read(): void }\n",
    ] {
        assert!(parse_text(source, ParseGoal::ModuleSource).has_errors());
    }
}

#[test]
fn parses_drop_and_test_declarations() {
    let tree = assert_syntax_ok(
        "drop Buffer<T>(&+self) {}\ntest empty {}\n",
        ParseGoal::ModuleSource,
    );

    assert!(has_node_kind(&tree, NodeKind::DropDeclaration));
    assert!(has_node_kind(&tree, NodeKind::TestDeclaration));
}

#[test]
fn drop_and_test_declarations_keep_their_closed_headers() {
    for source in [
        "pub drop Buffer(&+self) {}\n",
        "drop Buffer(&self) {}\n",
        "test named(): void {}\n",
        "#target: \"arm64-darwin\"\ntest targeted {}\n",
    ] {
        assert!(parse_text(source, ParseGoal::ModuleSource).has_errors());
    }
}
