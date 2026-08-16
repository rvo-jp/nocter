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
