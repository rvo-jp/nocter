//! Source retirement, invocation commit and caller-owned access temporaries.
use super::support::steps;
use super::*;

fn rejects(text: &str, code: &str) {
    let (sources, tree) = input(text);
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    match Unit::prepare(&context) {
        Err(Failure::Body {
            failure: BodyFailure::Rejected(diagnostic),
            ..
        }) => {
            assert_eq!(diagnostic.source().code(), code, "{text}: {diagnostic:?}");
        }
        Err(Failure::Flow { diagnostics, .. }) => {
            assert!(
                diagnostics.iter().any(|d| d.code() == code),
                "{text}: {diagnostics:?}"
            );
        }
        other => panic!("expected {code}: {text}: {other:?}"),
    }
}

#[test]
fn existing_noncopy_sources_need_markers_at_every_value_boundary() {
    for source in [
        "func run(take value: error): error { value }",
        "func run(take value: error): void { let next = value }",
        "func consume(take value: error): void {}\nfunc run(take value: error): void { consume(value) }",
        "func run(take value: error): void { let action = (value;): void {} }",
    ] {
        rejects(source, "E0371");
    }
    accepted("func run(take value: error): error { let next = value^\nnext^ }");
}

#[test]
fn marked_copy_retires_but_unmarked_copy_preserves_and_var_can_restore() {
    accepted("func run(): i32 { let original = 42\nlet snapshot = original\noriginal + snapshot }");
    rejects(
        "func run(): i32 { let original = 42\nlet next = original^\noriginal }",
        "E0378",
    );
    rejects(
        "func run(): i32 { let original = 42\nlet next = original^\noriginal = 0\nnext }",
        "E0384",
    );
    accepted(
        "func run(): i32 { var original = 42\nlet next = original^\noriginal = next\noriginal }",
    );
}

#[test]
fn transfer_requires_an_owner_even_for_copy_and_rejects_fresh_or_repeated_sources() {
    for source in [
        "func run(value: i32): i32 { value^ }",
        "func run(edit value: i32): i32 { value^ }",
        "func run(): i32 { (1 + 2)^ }",
        "func run(): i32 { let value = 1\nvalue^^ }",
        "func make(): i32 { 1 }\nfunc run(): i32 { make()^ }",
        "func make(): i32 { 1 }\nfunc run(): void { let action = make^ }",
    ] {
        rejects(source, "E0371");
    }
}

#[test]
fn copy_transfer_reserves_until_call_commit_and_nested_construction_does_not() {
    let unit = accepted(
        "func sink(take value: i32, other: i32): void {}\nfunc run(): i32 {\nlet original = 42\nsink((original^), if true { return original } else { return 0 })\n0\n}",
    );
    assert!(
        !steps(named(&unit, "run"))
            .iter()
            .any(|s| matches!(s, Step::Invoke { .. }))
    );
    rejects(
        "struct Pair { first: i32\nsecond: i32 }\nfunc run(): i32 {\nlet original = 42\nlet pair = Pair { first: original^, second: if true { return original } else { return 0 } }\n0\n}",
        "E0378",
    );
}

#[test]
fn surviving_read_input_conflicts_with_copy_transfer_without_snapshot_repair() {
    rejects(
        "func sink(observed: i32, take value: i32): void {}\nfunc run(): void {\nlet original = 42\nsink(original, original^)\n}",
        "E0396",
    );
    accepted(
        "func inspect(value: i32): i32 { value }\nfunc sink(take value: i32, other: i32): void {}\nfunc run(): void {\nlet original = 42\nsink(original^, inspect(original))\n}",
    );
}

#[test]
fn capture_marks_formation_separately_from_consuming_its_environment() {
    let unit = accepted(
        "func run(take value: error): error {\nlet action = (value^;): error { value^ }\naction^()\n}",
    );
    assert_eq!(unit.closures().len(), 1);
    rejects(
        "func run(): i32 { let value = 42\nlet action = (value^;): i32 { value }\nvalue }",
        "E0378",
    );
    accepted(
        "func run(): i32 { let value = 42\nlet action = (value;): i32 { value }\naction() + value }",
    );
}

#[test]
fn function_adapter_is_materialized_at_commit_and_not_during_interrupted_preparation() {
    let unit = accepted(
        "func sink(take action: once(): void, value: i32): void {}\nfunc run(take action: func(): void): i32 {\nsink(action^, if true { return 1 } else { return 2 })\n0\n}",
    );
    let trace = steps(named(&unit, "run"));
    assert!(
        !trace
            .iter()
            .any(|s| matches!(s, Step::AdaptFunction { .. } | Step::Invoke { .. }))
    );
    assert_eq!(trace.iter().filter(|s| matches!(s, Step::Dispose { payload, .. } if payload.disposal() == Disposal::FunctionEnvironment)).count(), 1);
}

#[test]
fn read_and_edit_calls_lend_marked_sources_from_owned_evaluation_temporaries() {
    for mode in ["", "edit "] {
        let source = format!(
            "func touch({mode}value: i32): void {{}}\nfunc run(): void {{\nlet original = 42\ntouch(original^)\n}}"
        );
        let unit = accepted(&source);
        let trace = steps(named(&unit, "run"));
        assert!(!trace.iter().any(|s| matches!(s, Step::Invoke { .. })));
        assert_eq!(trace.iter().filter(|s| matches!(s, Step::Dispose { source, .. } if matches!(source.holder(), Holder::Temporary { .. }))).count(), 1);
        rejects(
            &source.replace("touch(original^)", "touch(original^)\nlet _ = original"),
            "E0378",
        );
    }
    accepted(
        "func touch(edit value: i32): void { value += 1 }\nfunc run(): void { touch(40 + 2) }",
    );
    rejects(
        "func touch(edit value: i32): void {}\nfunc run(): void { let value = 42\ntouch(value) }",
        "E0387",
    );
}

#[test]
fn field_transfer_retires_the_selected_copy_subplace_or_the_explicit_whole_parent() {
    let declaration = "copy struct Pair { first: i32\nsecond: i32 }\n";
    accepted(&format!(
        "{declaration}func run(): i32 {{\nlet pair = Pair {{ first: 1, second: 2 }}\nlet saved = pair.first^\npair.second\n}}"
    ));
    accepted(&format!(
        "{declaration}func run(): i32 {{\nvar pair = Pair {{ first: 1, second: 2 }}\nlet saved = pair.first^\npair.first = saved\npair.first + pair.second\n}}"
    ));
    rejects(
        &format!(
            "{declaration}func run(): Pair {{\nlet pair = Pair {{ first: 1, second: 2 }}\nlet saved = pair.first^\npair\n}}"
        ),
        "E0378",
    );
    rejects(
        &format!(
            "{declaration}func run(): i32 {{\nlet pair = Pair {{ first: 1, second: 2 }}\nlet saved = pair^.first\npair.second\n}}"
        ),
        "E0378",
    );
}

#[test]
fn a_produced_noncopy_update_input_keeps_exact_temporary_cleanup() {
    let unit = accepted(
        "func identity(take value: error): error { value^ }\nfunc touch(edit value: error): void {}\nfunc run(take value: error): void { touch(identity(value^)) }",
    );
    let trace = steps(named(&unit, "run"));
    assert_eq!(
        trace
            .iter()
            .filter(|s| matches!(s, Step::Invoke { .. }))
            .count(),
        1
    );
    assert_eq!(trace.iter().filter(|s| matches!(s, Step::Dispose { source, payload } if payload.disposal() == Disposal::OwnedError && matches!(source.holder(), Holder::Temporary { .. }))).count(), 1);
}
