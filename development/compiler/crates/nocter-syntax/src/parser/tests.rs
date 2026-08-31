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

#[test]
fn reusable_parse_products_bind_every_identity_to_the_current_source() {
    let text = "/// docs\nfunc broken(value i32): void { return }\n";
    let mut sources = SourceMap::new();
    let first = sources
        .add_bytes(SourceName::new("first.nct"), text.as_bytes())
        .unwrap();
    let second = sources
        .add_bytes(SourceName::new("second.nct"), text.as_bytes())
        .unwrap();
    let parsed = crate::parse_reusable(sources.get(first).unwrap(), ParseGoal::SourceFile);
    let tree = parsed.bind(sources.get(second).unwrap()).unwrap();

    assert_eq!(tree.source(), second);
    assert!(tree.nodes().all(|(node, _)| node.source() == second));
    assert!(
        tree.lexed()
            .tokens()
            .iter()
            .all(|token| token.span().source() == second)
    );
    assert!(
        tree.lexed()
            .comments()
            .iter()
            .all(|comment| comment.span().source() == second)
    );
    assert!(
        tree.diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.span().source() == second)
    );
    assert!(
        crate::descendant_token_iter(&tree, tree.root_id()).all(|token| token.source() == second)
    );
}

#[test]
fn reusable_parse_products_reject_different_source_text() {
    let mut sources = SourceMap::new();
    let first = sources
        .add_bytes(SourceName::new("first.nct"), b"func first(): void {}\n")
        .unwrap();
    let second = sources
        .add_bytes(SourceName::new("second.nct"), b"func second(): void {}\n")
        .unwrap();
    let parsed = crate::parse_reusable(sources.get(first).unwrap(), ParseGoal::SourceFile);

    assert!(parsed.bind(sources.get(second).unwrap()).is_none());
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

fn count_node_kind(tree: &SyntaxTree, expected: NodeKind) -> usize {
    tree.nodes()
        .filter(|(_, node)| node.kind() == expected)
        .count()
}

#[test]
fn parses_empty_and_nested_package_directive_values() {
    assert_syntax_ok("", ParseGoal::SourceFile);
    assert_syntax_ok(
        "#package: { name: \"example\", version: \"0.1.0\", }\n#dependencies: {\n    json: {\n        git: \"https://example.test/json.git\",\n    },\n}\n#test: { name: \"unit\", module: \"./tests/unit\", }\n",
        ParseGoal::SourceFile,
    );
    assert_syntax_ok(
        "#executable: {\n    name: \"app\"\n}\n",
        ParseGoal::SourceFile,
    );
}

#[test]
fn keeps_duplicate_directives_as_a_semantic_boundary() {
    assert_syntax_ok(
        "#package: { name: \"first\", version: \"0.0.0\", }\n#package: { name: \"second\", version: \"0.0.0\", }\n",
        ParseGoal::SourceFile,
    );
}

#[test]
fn rejects_non_data_and_interpolated_directive_values() {
    let boolean = parse_text("#package: true\n", ParseGoal::SourceFile);
    assert!(boolean.has_errors());

    let interpolation = parse_text(
        "#package: { name: \"${name}\", version: \"0.0.0\", }\n",
        ParseGoal::SourceFile,
    );
    assert!(interpolation.has_errors());
}

#[test]
fn rejects_record_fields_without_commas() {
    let tree = parse_text(
        "#executable: {\n    name: \"app\"\n    module: \".\"\n}\n",
        ParseGoal::SourceFile,
    );

    assert!(tree.diagnostics().iter().any(|diagnostic| {
        diagnostic.kind()
            == ParseDiagnosticKind::Expected(ExpectedSyntax::Punctuation(Punctuation::Comma))
    }));
}

#[test]
fn eof_recovery_never_duplicates_the_eof_token() {
    let tree = parse_text("#", ParseGoal::SourceFile);
    assert!(tree.has_errors());
    assert_complete_token_projection(&tree);
}

#[test]
fn parses_private_scoped_and_selected_imports() {
    assert_syntax_ok(
        "use std/io\nuse /parser.Parser\nuse /.RootValue\nuse ../shared/path.{Path, normalize as clean,}\npub use ./public\npub(../../) use ./internal.Value\n",
        ParseGoal::SourceFile,
    );
}

#[test]
fn parses_exact_source_visibility_separately_from_module_uses() {
    let tree = assert_syntax_ok(
        "see ./index.nct\nsee ./internal/search.nct\nsee ../shared.nct\nuse ./parser\n",
        ParseGoal::SourceFile,
    );
    assert_eq!(tree.root().kind(), NodeKind::SourceFile);
    assert_eq!(
        tree.children(tree.root_id())
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) => tree.node(*node),
                _ => None,
            })
            .filter(|node| node.kind() == NodeKind::SourceVisibilityDeclaration)
            .count(),
        3
    );
}

#[test]
fn rejects_noncanonical_source_visibility_paths() {
    for source in [
        "see ./search\n",
        "see ../search\n",
        "see ./../search.nct\n",
        "see /search.nct\n",
        "see std/search.nct\n",
    ] {
        let tree = parse_text(source, ParseGoal::SourceFile);
        assert!(tree.has_errors(), "accepted {source:?}");
        assert_complete_token_projection(&tree);
    }
}

#[test]
fn rejects_empty_selection() {
    let empty = parse_text("use ./parser.{}\n", ParseGoal::SourceFile);
    assert!(empty.has_errors());
}

#[test]
fn enforces_the_lexical_module_segment_language() {
    for source in ["use Parser\n", "use _\n", "use std/type\n"] {
        let tree = parse_text(source, ParseGoal::SourceFile);
        assert!(tree.diagnostics().iter().any(|diagnostic| {
            diagnostic.kind() == ParseDiagnosticKind::Expected(ExpectedSyntax::ModuleSegment)
        }));
        assert_complete_token_projection(&tree);
    }

    assert_syntax_ok("use package_2/source_file\n", ParseGoal::SourceFile);
}

#[test]
fn recognizes_late_use_after_an_item_shaped_line() {
    let tree = parse_text("func main(): void {}\nuse std/io\n", ParseGoal::SourceFile);

    assert!(
        tree.diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.kind() == ParseDiagnosticKind::LateDependencyDeclaration)
    );
}

#[test]
fn recognizes_late_see_after_an_item_shaped_line() {
    let tree = parse_text(
        "func main(): void {}\nsee ./helper.nct\n",
        ParseGoal::SourceFile,
    );

    assert!(
        tree.diagnostics().iter().any(|diagnostic| {
            diagnostic.kind() == ParseDiagnosticKind::LateDependencyDeclaration
        })
    );
}

#[test]
fn parses_callable_declarations_and_nested_type_closers() {
    assert_syntax_ok(
        "pub func choose<T>(left: &T, right: &T): &T from left | right\npub primitive func invoke<T>(callback: &+func(input: &T): T, input: &T): T\npub primitive type i32\ntype Nested<T> = parser.Outer<Inner<T>>\ntype DoubleBorrow<T> = &&T\n",
        ParseGoal::SourceFile,
    );
}

#[test]
fn parses_opaque_and_layered_callable_results() {
    assert_syntax_ok(
        "func values<T>(): some Source<T> { .Item = &T }?\nfunc load<T>(): T?!\n",
        ParseGoal::SourceFile,
    );
}

#[test]
fn keeps_bodyless_private_functions_as_a_semantic_boundary() {
    assert_syntax_ok("func contract(): i32\n", ParseGoal::SourceFile);
}

#[test]
fn recognizes_bodyless_nominal_and_operator_contracts() {
    assert_syntax_ok(
        "pub struct String\npub enum Token\ninstance String {\n    pub operator (&self == other: &Self): bool\n}\n",
        ParseGoal::SourceFile,
    );
}

#[test]
fn parses_target_attachment_and_empty_bodies() {
    assert_syntax_ok(
        "#target: \"arm64-darwin\"\npub func main(): void {}\n",
        ParseGoal::SourceFile,
    );

    let same_line = parse_text(
        "#target: \"arm64-darwin\" func main(): void {}\n",
        ParseGoal::SourceFile,
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
        assert!(parse_text(source, ParseGoal::SourceFile).has_errors());
    }
}

#[test]
fn parses_every_requirement_shape_without_type_driven_disambiguation() {
    assert_syntax_ok(
        "func constrained<T, U, C, I>(value: &T): T where T impl Interface<U> { .Item = U }, copy U, (&T == &T): bool, (&T < &T): bool, (&C[usize]): &U, (&+C[usize]): &+U, &T as &str, (...&C): I\n",
        ParseGoal::SourceFile,
    );
}

#[test]
fn copy_spelling_remains_a_requirement_subject_before_impl() {
    assert_syntax_ok(
        "func constrained<copy>(): copy where copy impl Interface\n",
        ParseGoal::SourceFile,
    );
}

#[test]
fn rejects_missing_requirement_commas_and_undeclared_operator_shapes() {
    for source in [
        "func missing<T>(): T where copy T T impl Interface\n",
        "func unsupported<T>(): T where (&T <= &T): bool\n",
        "func trailing<T>(): T where T impl Interface,\n",
    ] {
        assert!(parse_text(source, ParseGoal::SourceFile).has_errors());
    }
}

#[test]
fn flat_tree_owns_deep_non_recursive_prefix_syntax() {
    let source = format!("type Deep = {}i32\n", "*".repeat(10_000));
    let tree = assert_syntax_ok(&source, ParseGoal::SourceFile);

    assert_eq!(tree.root().kind(), NodeKind::SourceFile);
}

#[test]
fn recursive_delimiters_stop_at_the_declared_nesting_limit() {
    let source = format!("type TooDeep = {}i32{}\n", "(".repeat(300), ")".repeat(300));
    let tree = parse_text(&source, ParseGoal::SourceFile);

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

    assert_syntax_ok(&source, ParseGoal::SourceFile);
}

#[test]
fn parses_the_complete_type_atom_and_prefix_surface() {
    let tree = assert_syntax_ok(
        "type Scalar = bool\ntype Signed = i64\ntype Text = str\ntype Failure = error\ntype Unit = void\ntype Bottom = never\ntype Projection<T> = &parser.Buffer<T>.Item?\ntype Slice<T> = [T]\ntype Array<T> = [T; 16]\ntype Group<T> = (*(&+T))\ntype Callback<T> = &+func(input: &T): &T from input\n",
        ParseGoal::SourceFile,
    );

    for kind in [
        NodeKind::NamedType,
        NodeKind::BorrowType,
        NodeKind::PointerType,
        NodeKind::SliceType,
        NodeKind::FixedArrayType,
        NodeKind::GroupedType,
        NodeKind::CallableType,
    ] {
        assert!(has_node_kind(&tree, kind));
    }
}

#[test]
fn rejects_closed_type_shapes_without_semantic_assistance() {
    for source in [
        "type Reversed = i32!?\n",
        "type GenericSelf = Self<T>\n",
        "type MissingResult = func(value: i32)\n",
    ] {
        assert!(parse_text(source, ParseGoal::SourceFile).has_errors());
    }
}

#[test]
fn parses_fixed_array_lengths_as_constant_expressions() {
    assert_syntax_ok(
        "const WIDTH: usize = 4\ntype Buffer = [u8; WIDTH * 2]\n",
        ParseGoal::SourceFile,
    );
}

#[test]
fn parses_module_namespace_aliases_in_top_level_and_block_imports() {
    let tree = assert_syntax_ok(
        "use std/io as console\nuse / as root\nfunc main(): void {\n    use ./support as local_support\n\n    return\n}\n",
        ParseGoal::SourceFile,
    );

    assert_eq!(
        count_node_kind(&tree, NodeKind::ModuleAlias),
        3,
        "every namespace alias must have one explicit syntax node"
    );
}

#[test]
fn keeps_type_validity_and_provenance_checks_out_of_parsing() {
    assert_syntax_ok(
        "type AssociatedArguments<T, U> = T.Item<U>\ntype BuiltinSelection = str.Item\nfunc origin<T>(value: &T): &T from missing\nfunc hidden(): some Source { .Missing = u8 }\ninstance Pair<T, T> {}\nfunc equality<T, U>(): T where T = U\n",
        ParseGoal::SourceFile,
    );
}

#[test]
fn opaque_results_keep_their_contextual_boundary() {
    assert_syntax_ok(
        "func values<T>(): some Source<T> { .Item = &T }?! {}\n",
        ParseGoal::SourceFile,
    );

    for source in [
        "func unnamed(): some {}\n",
        "type NotCallable = some Source\n",
    ] {
        assert!(parse_text(source, ParseGoal::SourceFile).has_errors());
    }
}

#[test]
fn parses_structs_enums_and_semantically_empty_enums() {
    let tree = assert_syntax_ok(
        "pub copy struct Pair<T> where copy T {\n    pub left: T\n    right: T\n}\nenum Maybe<T> {\n    some(value: T)\n    missing\n}\nenum Empty {}\n",
        ParseGoal::SourceFile,
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
        assert!(parse_text(source, ParseGoal::SourceFile).has_errors());
    }
}

#[test]
fn parses_interface_requirements_and_default_methods() {
    let tree = assert_syntax_ok(
        "interface Source<T> where copy T {\n    pub type Item impl Iterable<T> + Comparable\n    pub method &+self.next(): Self.Item?\n    pub default method self.consume(): void {}\n}\ninterface Source {\n    default method self.consume(): void {}\n}\n",
        ParseGoal::SourceFile,
    );

    assert!(has_node_kind(&tree, NodeKind::AssociatedTypeDeclaration));
    assert!(has_node_kind(&tree, NodeKind::InterfaceMethod));
    assert!(has_node_kind(&tree, NodeKind::SelfType));
}

#[test]
fn parses_interface_self_structural_prerequisites() {
    let tree = assert_syntax_ok(
        "interface Capability where (&Self == &Self): bool, (...Self): i32 {}\n",
        ParseGoal::SourceFile,
    );

    assert!(has_node_kind(&tree, NodeKind::OperatorPredicate));
    assert!(has_node_kind(&tree, NodeKind::ExpansionPredicate));
    assert!(has_node_kind(&tree, NodeKind::SelfType));
}

#[test]
fn rejects_implicit_interface_default_bodies() {
    let tree = parse_text(
        "interface Source { pub method self.consume(): void {} }\n",
        ParseGoal::SourceFile,
    );

    assert!(tree.diagnostics().iter().any(|diagnostic| {
        diagnostic.kind() == ParseDiagnosticKind::Expected(ExpectedSyntax::Contextual("default"))
    }));
}

#[test]
fn associated_binding_lists_report_their_own_expected_syntax() {
    let tree = parse_text(
        "func make(): some Source { .Item = i32, invalid } { return }\n",
        ParseGoal::SourceFile,
    );

    assert!(tree.diagnostics().iter().any(|diagnostic| {
        diagnostic.kind() == ParseDiagnosticKind::Expected(ExpectedSyntax::AssociatedTypeBinding)
    }));
}

#[test]
fn interface_members_require_bare_public_visibility() {
    for source in [
        "interface Source { method &self.read(): void }\n",
        "interface Source { pub(./) method &self.read(): void }\n",
    ] {
        assert!(parse_text(source, ParseGoal::SourceFile).has_errors());
    }

    assert_syntax_ok(
        "interface Source { pub type Item\npub type Item }\n",
        ParseGoal::SourceFile,
    );
}

#[test]
fn parses_construction_functions_and_both_literal_shapes() {
    let tree = assert_syntax_ok(
        "construct Vec<T> {\n    pub literal [](...items: T): Self {}\n    pub literal \"\"(text: &str): Self\n    pub func with_capacity(capacity: usize): Self {}\n}\n",
        ParseGoal::SourceFile,
    );

    assert!(has_node_kind(&tree, NodeKind::LiteralShape));
    assert!(has_node_kind(&tree, NodeKind::ArgumentPackModifier));
    assert!(has_node_kind(&tree, NodeKind::ConstructionFunction));
}

#[test]
fn parses_keyed_argument_packs_and_mapping_literals() {
    let tree = assert_syntax_ok(
        "construct Map<K, V> {\n    pub literal [:](...entries: K: V): Self {}\n}\nfunc load(...entries: &str: i32): void {\n    for key: value in entries { load(key: value) }\n    let values = Map [\"one\": 1, \"two\": 2]\n    let empty = Map<&str, i32> [:]\n}\n",
        ParseGoal::SourceFile,
    );

    for kind in [
        NodeKind::ArgumentPackValueType,
        NodeKind::TypedMappingLiteral,
        NodeKind::MappingBody,
        NodeKind::MappingElement,
        NodeKind::KeyedArgument,
        NodeKind::ForBindings,
    ] {
        assert!(has_node_kind(&tree, kind), "missing {kind:?}");
    }
}

#[test]
fn callable_types_distinguish_value_and_keyed_packs() {
    let tree = assert_syntax_ok(
        "type Values = func(...i32): void\ntype Entries = func(...&str: i32): void\n",
        ParseGoal::SourceFile,
    );

    assert_eq!(
        tree.nodes()
            .filter(|(_, node)| node.kind() == NodeKind::ArgumentPackValueType)
            .count(),
        1
    );
}

#[test]
fn callable_argument_packs_and_call_spreads_share_the_general_expression_grammar() {
    let tree = assert_syntax_ok(
        "func collect<T>(first: T, ...rest: T): void { collect(first, ...rest) }\n",
        ParseGoal::SourceFile,
    );

    assert!(has_node_kind(&tree, NodeKind::ArgumentPackModifier));
    assert!(has_node_kind(&tree, NodeKind::SpreadExpression));
}

#[test]
fn construction_visibility_remains_semantic() {
    assert_syntax_ok(
        "construct Value { func new(): Self {} }\n",
        ParseGoal::SourceFile,
    );
}

#[test]
fn rejects_construction_default_and_empty_surfaces() {
    for source in [
        "construct Value { pub default func new(): Self {} }\n",
        "construct Value {}\n",
    ] {
        assert!(parse_text(source, ParseGoal::SourceFile).has_errors());
    }
}

#[test]
fn parses_every_instance_member_family() {
    let tree = assert_syntax_ok(
        "instance Text<T> where copy T {\n    pub method &self.len(): usize {}\n    coerce &self as &str from self {}\n    pub operator (&self == other: &Self): bool {}\n    pub operator (&self < other: &Self): bool {}\n    pub operator (&self[index: usize]): &T from self {}\n    pub operator (&+self[index: usize]): &+T from self {}\n    pub operator (...&self): Iterator<T> {}\n    pub operator (...self): Iterator<T> {}\n}\n",
        ParseGoal::SourceFile,
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
        assert!(parse_text(source, ParseGoal::SourceFile).has_errors());
    }
}

#[test]
fn parses_instance_owned_interface_implementation() {
    let tree = assert_syntax_ok(
        "instance Input<T> where copy T {\n    impl Source<T> { .Item = T }\n    method &+self.next(): Self.Item? {}\n}\n",
        ParseGoal::SourceFile,
    );

    assert!(has_node_kind(&tree, NodeKind::InterfaceImplementation));
    assert!(has_node_kind(&tree, NodeKind::InterfaceApplication));
    assert!(has_node_kind(&tree, NodeKind::AssociatedTypeBinding));
    assert!(has_node_kind(&tree, NodeKind::InherentMethod));
}

#[test]
fn interface_implementation_rejects_visibility_and_a_member_body() {
    assert!(
        parse_text(
            "instance Input { pub impl Source }\n",
            ParseGoal::SourceFile,
        )
        .has_errors()
    );
    assert!(
        parse_text(
            "instance Input { impl Source { method &self.read(): void {} } }\n",
            ParseGoal::SourceFile,
        )
        .has_errors()
    );
    assert!(
        parse_text(
            "instance Input { impl Source { type Item = i32 } }\n",
            ParseGoal::SourceFile,
        )
        .has_errors()
    );
    assert!(
        parse_text(
            "instance Input { impl Source { Item = i32 } }\n",
            ParseGoal::SourceFile,
        )
        .has_errors()
    );
    assert!(
        parse_text(
            "func old(): some Source { Item = i32 } { return }\n",
            ParseGoal::SourceFile,
        )
        .has_errors()
    );
    assert!(
        parse_text(
            "func malformed(): some Source { .Item i32 } { return }\n",
            ParseGoal::SourceFile,
        )
        .has_errors()
    );
}

#[test]
fn interface_and_callable_requirements_have_disjoint_separators() {
    assert!(
        parse_text(
            "func invalid<F>(): void where F impl func(i32): i32 { return }\n",
            ParseGoal::SourceFile,
        )
        .has_errors()
    );

    let tree = assert_syntax_ok(
        "interface Source {}\nfunc parsed<T, F>(): void where T: Source, F: func(i32): i32 { return }\n",
        ParseGoal::SourceFile,
    );
    assert!(has_node_kind(&tree, NodeKind::CallablePredicate));
}

#[test]
fn parses_drop_and_test_declarations() {
    let tree = assert_syntax_ok(
        "drop Buffer<T>(&+self) {}\ntest empty {}\n",
        ParseGoal::SourceFile,
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
        assert!(parse_text(source, ParseGoal::SourceFile).has_errors());
    }
}
