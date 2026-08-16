use std::fmt::Write;

use nocter_source::{SourceMap, SourceName};

use super::{ParseGoal, parse};
use crate::{SyntaxElement, SyntaxTree};

struct SnapshotCase {
    name: &'static str,
    source: &'static str,
    shape: &'static str,
    goal: ParseGoal,
}

struct RejectedCase {
    name: &'static str,
    source: &'static str,
    goal: ParseGoal,
}

const CASES: &[SnapshotCase] = &[
    SnapshotCase {
        name: "G001 package directives",
        source: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g001-package.nct"
        )),
        shape: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g001-package.shape"
        )),
        goal: ParseGoal::PackageFile,
    },
    SnapshotCase {
        name: "G002-G006 module prefix and callable declaration",
        source: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g002-g006-module.nct"
        )),
        shape: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g002-g006-module.shape"
        )),
        goal: ParseGoal::ModuleSource,
    },
    SnapshotCase {
        name: "G007-G012 declaration containers",
        source: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g007-g012-declarations.nct"
        )),
        shape: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g007-g012-declarations.shape"
        )),
        goal: ParseGoal::ModuleSource,
    },
    SnapshotCase {
        name: "G013-G018 types and requirements",
        source: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g013-g018-types.nct"
        )),
        shape: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g013-g018-types.shape"
        )),
        goal: ParseGoal::ModuleSource,
    },
    SnapshotCase {
        name: "G001-G018 semantic boundaries",
        source: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g001-g018-semantic.nct"
        )),
        shape: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g001-g018-semantic.shape"
        )),
        goal: ParseGoal::ModuleSource,
    },
];

const REJECTED_CASES: &[RejectedCase] = &[
    RejectedCase {
        name: "G001 package data",
        source: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g001-package-reject.nct"
        )),
        goal: ParseGoal::PackageFile,
    },
    RejectedCase {
        name: "G002-G006 module prefix",
        source: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g002-g006-module-reject.nct"
        )),
        goal: ParseGoal::ModuleSource,
    },
    RejectedCase {
        name: "G007-G012 declaration containers",
        source: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g007-g012-declarations-reject.nct"
        )),
        goal: ParseGoal::ModuleSource,
    },
    RejectedCase {
        name: "G013-G018 types and requirements",
        source: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g013-g018-types-reject.nct"
        )),
        goal: ParseGoal::ModuleSource,
    },
];

#[test]
fn accepted_fixture_shapes_are_stable_and_lossless() {
    for case in CASES {
        let tree = parse_fixture(case.source, case.goal);
        assert!(
            tree.lexed().diagnostics().is_empty() && tree.diagnostics().is_empty(),
            "{}: {:?}",
            case.name,
            tree.diagnostics()
        );
        assert_token_projection(&tree, case.name);
        assert_eq!(syntax_shape(&tree), case.shape, "{}", case.name);
    }
}

#[test]
fn rejected_fixtures_retain_every_lexical_token() {
    for case in REJECTED_CASES {
        let tree = parse_fixture(case.source, case.goal);
        assert!(tree.has_errors(), "{} unexpectedly parsed", case.name);
        assert_token_projection(&tree, case.name);
    }
}

fn parse_fixture(text: &str, goal: ParseGoal) -> SyntaxTree {
    let mut sources = SourceMap::new();
    let source = sources
        .add_bytes(SourceName::new("fixture.nct"), text.as_bytes())
        .expect("fixtures are normalized UTF-8");
    parse(sources.get(source).expect("fixture source exists"), goal)
}

fn syntax_shape(tree: &SyntaxTree) -> String {
    let mut output = String::new();
    let mut pending = vec![(tree.root_id(), 0_usize)];
    while let Some((id, depth)) = pending.pop() {
        let node = tree.node(id).expect("tree child IDs belong to their tree");
        writeln!(output, "{:width$}{:?}", "", node.kind(), width = depth * 2)
            .expect("writing to a String cannot fail");

        for child in tree.children(id).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                pending.push((*child, depth + 1));
            }
        }
    }
    output
}

fn assert_token_projection(tree: &SyntaxTree, name: &str) {
    let mut pieces = vec![Vec::new(); tree.lexed().tokens().len()];
    let mut pending = vec![tree.root_id()];
    while let Some(id) = pending.pop() {
        for child in tree.children(id) {
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
            ranges.first().map(|range| range.start()),
            Some(token.span().range().start()),
            "{name}"
        );
        assert_eq!(
            ranges.last().map(|range| range.end()),
            Some(token.span().range().end()),
            "{name}"
        );
        assert!(
            ranges
                .windows(2)
                .all(|pair| pair[0].end() == pair[1].start()),
            "{name}"
        );
    }
}
