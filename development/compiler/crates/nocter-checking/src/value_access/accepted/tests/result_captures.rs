//! Result-resolved owned environments close before supply, containment and body admission.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::Value as Datum;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/native/value-access/result-owned-captures.nct"
));

#[test]
fn a_factory_environment_change_invalidates_copy_and_closed_capture_reuse() {
    let (sources, tree) = input(SOURCE);
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let copied = context
        .program()
        .resolve(context.program().symbols().get("copied").unwrap())
        .unwrap();
    let raw = crate::value_access::Query::prepare(&context, copied).unwrap();
    let unit = Unit::prepare(&context).unwrap();
    let changed = SOURCE.replace(
        "(amount; value: i32): i32 { amount + value }",
        "let item = Item { value: amount }\n(item^; value: i32): i32 { item.value + value }",
    );
    assert_ne!(SOURCE, changed);
    let (changed_sources, changed_tree) = input(&changed);
    let current = headers
        .bind(bound(&changed_sources, &changed_tree))
        .unwrap();
    assert!(raw.bind(&current).is_ok());
    assert!(matches!(
        unit.body(copied).unwrap().typed().bind(&current),
        Err(crate::value_access::Failure::ChangedBody)
    ));
    assert!(
        matches!(Unit::prepare(&current), Err(Failure::Body { failure: crate::value_access::Failure::Rejected(error), .. }) if error.rule() == crate::value_access::Rule::MissingTransfer)
    );
}

#[test]
fn an_absent_captured_application_never_becomes_a_signature_derived_witness() {
    for body in ["missing(41)", "let next = forward(missing)\nnext(41)"] {
        let source = format!(
            "func forward(take callback: func(i32): i32): func(i32): i32 {{ callback^ }}\nfunc run(): i32 {{ let missing = forward(if true {{ return 42 }} else {{ return 0 }})\nlet outer = (missing;): i32 {{ {body} }}\n0 }}"
        );
        let (sources, tree) = input(&source);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        assert!(
            matches!(&failure, SourceFailure::Checking(Failure::Body { failure: crate::value_access::Failure::Rejected(error), .. }) if error.rule() == crate::value_access::Rule::TypeMismatch),
            "{failure:?}"
        );
    }
}

#[test]
fn result_owned_captures_preserve_instances_invocation_and_supply_in_required_execution() {
    assert_eq!(answer(SOURCE, "ANSWER"), Datum::integer(42));
}

#[test]
fn loop_recreation_preserves_forwarded_captures_and_surviving_sibling_views() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/recreated-captured-callables.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    let invalid = source.replace(
        "let value = action^()",
        "let value = action^()\nlet _ = action^()",
    );
    assert_ne!(source, invalid);
    let (sources, tree) = input(&invalid);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}

#[test]
fn even_an_unused_pending_environment_requires_result_closure_before_flow_admission() {
    let source = "func leaf(value: i32): i32 { value }\nfunc make(): func(i32): i32 { leaf }\nfunc run(): void { let callback = make()\nlet unused = (callback;): i32 { 42 } }";
    let (sources, tree) = input(source);
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let id = context
        .program()
        .resolve(context.program().symbols().get("run").unwrap())
        .unwrap();
    let raw = crate::value_access::Query::prepare(&context, id).unwrap();
    let nested = raw.closures.values().next().unwrap();
    assert!(!nested.typed.results_closed());
    assert!(matches!(
        flow::Query::prepare(nested.typed.clone()),
        Err(flow::IntegrityError::UnclosedResultWitnesses)
    ));
    Unit::prepare(&context).unwrap();
}

#[test]
fn resolved_owned_capture_cycles_are_local_source_construction_failures() {
    for source in [
        "func bad(): func(i32): i32 { let callback = bad()\n(callback; value: i32): i32 { callback(value) } }",
        "func bad(): func(i32): i32 { let callbacks = [bad()]\n(callbacks; value: i32): i32 { callbacks[0](value) } }",
    ] {
        let (sources, tree) = input(source);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        assert!(
            matches!(&failure, SourceFailure::Checking(Failure::Body { failure: crate::value_access::Failure::Rejected(error), .. }) if error.rule() == crate::value_access::Rule::InvalidConstruction && error.source().message().contains("contain itself")),
            "{failure:?}"
        );
    }
}

#[test]
fn resolved_capture_copy_does_not_implicitly_move_or_restore_a_retired_value() {
    for (suffix, copy_failure) in [
        (
            "func rejected(): void { let callback = owned(42)\nlet outer = (callback;): i32 { callback^() } }",
            true,
        ),
        (
            "func rejected(): void { let callback = owned(42)\nlet outer = (callback^;): i32 { callback() } }",
            true,
        ),
        (
            "func rejected(): i32 { let callback = make(2)\nlet outer = (callback^;): i32 { callback(40) }\ncallback(40) }",
            false,
        ),
    ] {
        let source = format!("{SOURCE}\n{suffix}");
        let (sources, tree) = input(&source);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        if copy_failure {
            assert!(
                matches!(failure, SourceFailure::Checking(Failure::Body { failure: crate::value_access::Failure::Rejected(error), .. }) if error.rule() == crate::value_access::Rule::MissingTransfer)
            );
        } else {
            assert!(matches!(
                failure,
                SourceFailure::Checking(Failure::Flow { .. })
            ));
        }
    }
}
