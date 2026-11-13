use super::*;
use crate::value_access::{
    flow::{
        Location, Operation,
        resources::{Disposal, Holder, Step},
    },
    tests::{bound, input},
};
use nocter_declaration_lowering::access_declarations as declarations;
use nocter_json::Value;
use std::{fs, path::PathBuf};
fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    let Value::Object(members) = value else {
        panic!("object")
    };
    &members
        .iter()
        .find(|m| m.name.as_ref() == name)
        .unwrap()
        .value
}
fn string(value: &Value) -> &str {
    let Value::String(s) = value else {
        panic!("string")
    };
    s
}
fn accepted(text: &str) -> Unit {
    let (sources, tree) = input(text);
    SourceUnit::prepare(bound(&sources, &tree))
        .unwrap_or_else(|failure| panic!("{text}: {failure:?}"))
        .into_unit()
}
fn named<'a>(unit: &'a Unit, name: &str) -> &'a flow::Query {
    unit.bodies()
        .find(|(id, _)| {
            unit.program()
                .codes()
                .get(*id)
                .and_then(nocter_declarations::value_access::Code::name)
                .and_then(|symbol| unit.program().symbols().spelling(symbol))
                == Some(name)
        })
        .unwrap()
        .1
}
#[test]
fn corpus_is_validated_by_the_complete_production_boundary() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/value-bindings");
    let manifest =
        nocter_json::parse(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    let Value::Array(cases) = field(&manifest, "cases") else {
        panic!("cases")
    };
    let mut counts = [0; 6];
    let mut mismatches = Vec::new();
    for case in cases {
        let name = string(field(case, "file"));
        let text = fs::read_to_string(root.join(name)).unwrap();
        let mut sources = nocter_source::SourceMap::new();
        let source = sources
            .add_bytes(nocter_source::SourceName::new(name), text.as_bytes())
            .unwrap();
        let tree = nocter_syntax::parse(
            sources.get(source).unwrap(),
            nocter_syntax::ParseGoal::V2CoreUnit,
        );
        if string(field(case, "expectation")) == "syntax-reject" {
            assert!(tree.has_errors(), "{name}");
            counts[0] += 1;
            continue;
        }
        assert!(!tree.has_errors(), "{name}: {:?}", tree.diagnostics());
        if string(field(case, "expectation")) == "header-reject" {
            let Err(declarations::Failure::Headers(diagnostics)) =
                declarations::Query::prepare(bound(&sources, &tree))
            else {
                panic!("{name}: expected a source-backed header rejection");
            };
            assert!(!diagnostics.is_empty(), "{name}");
            assert_eq!(
                diagnostics[0].source().code(),
                string(field(case, "diagnostic")),
                "{name}"
            );
            counts[5] += 1;
            continue;
        }
        let result = SourceUnit::prepare(bound(&sources, &tree)).map_err(|failure| match failure {
            SourceFailure::Declarations(failure) => {
                panic!("{name}: unexpected header rejection: {failure:?}")
            }
            SourceFailure::Modules(failure) => {
                panic!("{name}: unexpected module input failure: {failure:?}")
            }
            SourceFailure::Checking(failure) => failure,
        });
        if string(field(case, "expectation")) == "accept" {
            let source = result.unwrap_or_else(|e| panic!("{name}: {e:?}"));
            let context = source.declarations().bind(bound(&sources, &tree)).unwrap();
            let unit = source.unit();
            assert_eq!(unit.bodies().count(), context.program().codes().len());
            assert!(unit.bind(&context).is_ok());
            counts[1] += 1;
        } else {
            match result {
                Err(Failure::Body {
                    failure: BodyFailure::Rejected(diagnostic),
                    ..
                }) => {
                    assert!(!diagnostic.source().primary().span().range().is_empty());
                    if diagnostic.source().code() != string(field(case, "diagnostic")) {
                        mismatches.push(format!(
                            "{name}: expected {}, received {:?}",
                            string(field(case, "diagnostic")),
                            diagnostic.source()
                        ));
                    }
                    counts[2] += 1;
                }
                Err(Failure::Flow { diagnostics, .. }) => {
                    assert!(!diagnostics.is_empty());
                    let expected = string(field(case, "diagnostic"));
                    assert_eq!(diagnostics[0].code(), expected, "{name}");
                    counts[if expected == "E0378" { 3 } else { 4 }] += 1;
                }
                other => panic!("{name}: expected rejection, got {:?}", other.err()),
            }
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    assert!(
        counts.iter().all(|&count| count > 0),
        "each verdict family must be exercised: {counts:?}"
    );
}
#[test]
fn unused_bad_definition_rejects_the_entire_unit_and_recursive_valid_bodies_are_closed() {
    let (sources, tree) = input("func good(): void {}\nfunc unused(input: error): error { input }");
    let q = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        Unit::prepare(&q.bind(bound(&sources, &tree)).unwrap()),
        Err(Failure::Body { .. })
    ));
    let unit = accepted(
        "func first(take input: error): error { second(input^) }\nfunc second(take input: error): error { first(input^) }",
    );
    assert_eq!(unit.bodies().count(), 2);
}
#[test]
fn whole_unit_rebinding_checks_every_body_and_the_exact_authority() {
    let text = "func first(): i32 { 1 }\nfunc second(): i32 { 2 }";
    let (sources, tree) = input(text);
    let q = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let c = q.bind(bound(&sources, &tree)).unwrap();
    let unit = Unit::prepare(&c).unwrap();
    let (same, other_tree) = input(&format!("// source moved\n{text}"));
    let same = q.bind(bound(&same, &other_tree)).unwrap();
    assert!(unit.bind(&same).is_ok());
    let fresh = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let fresh = fresh.bind(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        unit.bind(&fresh),
        Err(BodyFailure::ChangedAuthority)
    ));
    let (changed, changed_tree) = input("func first(): i32 { 1 }\nfunc second(): i32 { 3 }");
    let changed = q.bind(bound(&changed, &changed_tree)).unwrap();
    assert!(matches!(unit.bind(&changed), Err(BodyFailure::ChangedBody)));
}
#[test]
fn read_of_a_fresh_noncopy_result_disposes_one_evaluation_temporary() {
    let unit = accepted(
        "func forward(take input: error): error { input^ }\nfunc observe(input: error): void {}\nfunc sample(take input: error): void { observe(forward(input^)) }",
    );
    let q = named(&unit, "sample");
    let mut disposed = Vec::new();
    for (point, block) in q.graph().points().iter() {
        for action in 0..block.actions.len() {
            for t in q.access().resources().at(Location { point, action }) {
                for step in t.steps() {
                    if let Step::Dispose { source, payload } = step
                        && payload.disposal() == Disposal::OwnedError
                    {
                        disposed.push(source.holder());
                    }
                }
            }
        }
    }
    assert_eq!(disposed.len(), 1);
    assert!(matches!(disposed[0], Holder::Temporary { .. }));
}
#[test]
fn cancellation_of_a_reserved_take_prefix_leaves_its_owner_until_scope_cleanup() {
    let unit = accepted(
        "func consume(take value: error,number: i32): void {}\nfunc sample(take input: error,finish: bool): void { consume(input^,if finish { return } else { 0 }) }",
    );
    let q = named(&unit, "sample");
    let local = q.typed().body().locals().iter().next().unwrap().0;
    let mut seen = false;
    for (point, block) in q.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            if matches!(event.operation, Operation::CancelCall { prepared: 1, .. }) {
                for t in q.access().resources().at(Location { point, action }) {
                    assert!(t.initialized_before(Holder::Local(local)));
                    assert!(t.initialized_after(Holder::Local(local)));
                    assert!(
                        !t.steps()
                            .iter()
                            .any(|step| matches!(step, Step::Invoke { .. }))
                    );
                    seen = true;
                }
            }
        }
    }
    assert!(seen);
}
#[test]
fn edit_input_replacement_retains_its_initialized_outside_slot() {
    let unit = accepted(
        "func replace(edit target: error,take replacement: error): void { target = replacement^ }",
    );
    let q = named(&unit, "replace");
    let mut seen = false;
    for (point, block) in q.graph().points().iter() {
        for action in 0..block.actions.len() {
            for t in q.access().resources().at(Location { point, action }) {
                seen|=t.steps().iter().any(|step|matches!(step,Step::ReplaceReferent{payload,..} if payload.disposal()==Disposal::OwnedError));
            }
        }
    }
    assert!(seen);
}
#[test]
fn every_call_result_is_a_value_and_conditional_copies_do_not_retain_the_source() {
    accepted(
        "func count(): i32 { 0 }\nfunc sample(flag: bool): i32 { var value = count()\nvalue += 1\nlet answer = if flag { value } else { 2 }\nvalue = 3\nreturn answer + value }",
    );
}
#[test]
fn a_conditional_move_does_not_restore_the_source_on_another_path() {
    let (sources, tree) = input(
        "func invalid(take input: error,flag: bool): error { if flag { let _ = input^ }\nreturn input^ }",
    );
    let header = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        Unit::prepare(&header.bind(bound(&sources, &tree)).unwrap()),
        Err(Failure::Flow { .. })
    ));
}

#[test]
fn literal_control_does_not_invent_normal_loop_exits() {
    for text in [
        "func f(): i32 { loop { if false { break } } }",
        "func f(): i32 { while true { if false { break } } }",
        "func f(): i32 { loop { if true { continue } else { () }\nbreak } }",
        "func f(): i32 { loop { while false { break } } }",
    ] {
        accepted(text);
    }
    for text in [
        "func f(): i32 { loop { if true { break } } }",
        "func f(): i32 { loop { if false { continue } else { break } } }",
        "func f(flag: bool): i32 { loop { if flag { continue }\nbreak } }",
    ] {
        let (sources, tree) = input(text);
        let q = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        assert!(
            matches!(
                Unit::prepare(&q.bind(bound(&sources, &tree)).unwrap()),
                Err(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })
                    if diagnostic.rule() == crate::value_access::Rule::MissingResult
            ),
            "{text}"
        );
    }
    let (sources, tree) =
        input("func f(): i32 { loop { if false { let bad: i32 = true }\ncontinue } }");
    let q = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        Unit::prepare(&q.bind(bound(&sources, &tree)).unwrap()),
        Err(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })
            if diagnostic.rule() == crate::value_access::Rule::TypeMismatch
    ));
}

mod callbacks;
mod construction;
mod functions;
mod producer_recipes;
mod records;
mod result_captures;
mod results;
mod selection;
mod structural_results;
mod support;

mod supply;

mod closures;
mod contextual_closures;
mod dependent_results;
mod inferred_closure_results;
mod input_layout;
mod unproduced;

mod transfers;

mod const_conditions;
mod constants;

mod initializers;

mod booleans;

mod comparisons;

mod statics;

mod tuples;

mod array_selection;
mod arrays;
mod header_arrays;
mod integers;
mod staged;

mod array_repetition;

mod indexed_edits;

mod indexed_aliases;

mod indexed_writes;

mod indexed_regions;

mod indexed_bounds;
mod indexed_transfers;

mod const_outcomes;
mod fallible_values;
mod namespaces;
mod optional_values;
mod primitives;
mod text_values;

mod anonymous_inputs;
