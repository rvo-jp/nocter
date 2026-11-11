//! Source-qualified concrete holes; unproved retained dynamic indices remain gated.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::Value as Datum;
#[test]
fn a_proven_element_transfer_leaves_siblings_and_retires_the_exact_source() {
    let source = "struct Item { value: i32 }\nconst func work(): i32 { let items = [Item { value: 40 }, Item { value: 2 }]\nlet first = items[0]^\nfirst.value + items[1].value }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    let unit = accepted(source);
    let trace = super::support::steps(named(&unit, "work"));
    assert!(trace.iter().any(|step| matches!(
        step,
        Step::DisposeArrayElements {
            start: 1,
            end: 2,
            ..
        }
    )));
    for use_after in [
        "let _ = items[0].value",
        "read view = items",
        "let _ = items[0]^",
        "read view = items[0]",
    ] {
        let source = format!(
            "struct Item {{ value: i32 }}\nfunc work(): void {{ let items = [Item {{ value: 1 }}]\nlet first = items[0]^\n{use_after} }}"
        );
        let (sources, tree) = input(&source);
        assert!(
            matches!(
                SourceUnit::prepare(bound(&sources, &tree)),
                Err(SourceFailure::Checking(Failure::Flow { .. }))
            ),
            "{source}"
        );
    }
}
#[test]
fn proven_replacement_restores_the_selected_owner_and_allows_whole_array_return() {
    let source = "struct Item { value: i32 }\nconst func work(): [Item; 2] { var items = [Item { value: 40 }, Item { value: 7 }]\nlet moved = items[0]^\nitems[0] = Item { value: moved.value + 2 }\nitems^ }\nconst ANSWER: [Item; 2] = work()";
    assert_eq!(
        answer(source, "ANSWER"),
        Datum::aggregate(vec![
            Datum::aggregate(vec![Datum::integer(42)]),
            Datum::aggregate(vec![Datum::integer(7)])
        ])
    );
}
#[test]
fn partial_record_elements_allow_disjoint_unknown_index_fields_without_restoring_a_hole() {
    let source = format!(
        "{}\nconst ANSWER: i32 = run()",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/arrays/element-field-hole.nct"
        ))
    );
    assert_eq!(answer(&source, "ANSWER"), Datum::integer(42));
    let unsafe_source = "struct Item { first: i32\nsecond: i32 }\nfunc work(index: usize): i32 { let items = [Item { first: 40, second: 2 }, Item { first: 7, second: 2 }]\nlet first = items[0].first^\nitems[index].first }";
    let (sources, tree) = input(unsafe_source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}
#[test]
fn repeated_known_selection_is_safe_only_when_restored_on_each_backedge() {
    let source = format!(
        "{}\nconst ANSWER: i32 = run()",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/arrays/element-restore-loop.nct"
        ))
    );
    assert_eq!(answer(&source, "ANSWER"), Datum::integer(42));
    let unsafe_source = "func work(flag: bool): void { let values = [1, 2]\nwhile flag { let moved = values[0]^ } }";
    let (sources, tree) = input(unsafe_source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}
#[test]
fn maximum_zero_byte_arrays_keep_sparse_holes_and_interval_cleanup_finite() {
    let source = format!(
        "{}\nconst ANSWER: i32 = run()",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/arrays/element-unit-maximum.nct"
        ))
    );
    assert_eq!(answer(&source, "ANSWER"), Datum::integer(42));
}
#[test]
fn indexed_take_reservations_commit_once_and_cancel_without_retiring_the_source() {
    let source = format!(
        "{}\nconst ANSWER: i32 = run()",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/arrays/element-transfer-call.nct"
        ))
    );
    assert_eq!(answer(&source, "ANSWER"), Datum::integer(42));
    let unit = accepted(
        "struct Item { reason: error }\nfunc consume(take item: Item, extra: i32): void {}\nfunc work(take reason: error): void { let items = [Item { reason: reason^ }]\nconsume(items[0]^, if true { return } else { 0 }) }",
    );
    let query = named(&unit, "work");
    assert!(
        query
            .graph()
            .points()
            .iter()
            .flat_map(|(_, block)| &block.actions)
            .any(|event| matches!(event.operation, Operation::CancelCall { prepared: 1, .. }))
    );
    let trace = super::support::steps(query);
    assert!(!trace.iter().any(|step| matches!(
        step,
        Step::DisposeArrayElements { .. } | Step::Invoke { .. }
    )));
}

#[test]
fn established_singleton_indices_keep_effects_and_alternative_holes_distinct() {
    let source = "const INDEX: usize = 0\nstruct Item { value: i32 }\nconst func work(flag: bool): i32 { var log = 0\nlet items = [Item { value: 40 }, Item { value: 2 }]\nlet moved = items[if flag { log += 1\nINDEX } else { log += 1\n0 }]^\nmoved.value + items[1].value + log - 1 }\nconst ANSWER: i32 = work(true) + work(false) - 42";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    let source = "struct Item { value: i32 }\nconst func work(flag: bool): i32 { let items = [Item { value: 20 }, Item { value: 22 }]\nif flag { let moved = items[0]^\nmoved.value } else { let moved = items[1]^\nmoved.value } }\nconst ANSWER: i32 = work(true) + work(false)";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn a_marker_grants_no_ownership_or_unproved_dynamic_hole_identity() {
    for (source, expected) in [
        (
            "func work(values: [i32; 2]): void { let moved = values[0]^ }",
            crate::value_access::Rule::MissingTransfer,
        ),
        (
            "func work(edit values: [i32; 2]): void { let moved = values[0]^ }",
            crate::value_access::Rule::MissingTransfer,
        ),
        (
            "func work(take values: [i32; 2], index: usize): void { let moved = values[index]^ }",
            crate::value_access::Rule::UnsupportedData,
        ),
        (
            "func work(): void { let moved = [1, 2][0]^ }",
            crate::value_access::Rule::MissingTransfer,
        ),
    ] {
        let (sources, tree) = input(source);
        assert!(
            matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: crate::value_access::Failure::Rejected(error), .. })) if error.rule() == expected),
            "{source}"
        );
    }
    let (sources, tree) = input(
        "const func work(): void { let values: [error; 0] = []\nlet _ = values[0]^ }\nconst ANSWER: void = work()",
    );
    assert!(
        matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) if diagnostic.rule() == crate::value_access::evaluation::Rule::Execution(nocter_constant_evaluation::state::Rule::IndexOutOfBounds))
    );
}

#[test]
fn a_pending_element_reservation_protects_its_source_until_cancellation() {
    let source = "struct Item { value: i32 }\nfunc consume(take item: Item, extra: i32): void {}\nfunc work(take items: [Item; 2]): void { consume(items[0]^, if true { drop items\nreturn } else { 0 }) }";
    let (sources, tree) = input(source);
    assert!(matches!(SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { diagnostics, .. }))
        if diagnostics.iter().any(|diagnostic| diagnostic.code() == "E0396")));
}

#[test]
fn owned_error_holes_leave_reverse_surviving_intervals_without_duplicate_disposal() {
    let unit = accepted(
        "func work(take a: error, take b: error, take c: error): void { let values = [a^, b^, c^]\nlet moved = values[1]^ }",
    );
    let trace = super::support::steps(named(&unit, "work"));
    let cleanup: Vec<_> = trace
        .iter()
        .filter_map(|step| match step {
            Step::Dispose { payload, .. } if payload.disposal() == Disposal::OwnedError => {
                Some("moved")
            }
            Step::DisposeArrayElements {
                start: 2, end: 3, ..
            } => Some("last"),
            Step::DisposeArrayElements {
                start: 0, end: 1, ..
            } => Some("first"),
            Step::ReleaseContainer { .. } => Some("container"),
            _ => None,
        })
        .collect();
    assert_eq!(cleanup, ["moved", "last", "first", "container"]);
}

#[test]
fn marked_elements_lend_caller_owned_read_and_edit_temporaries() {
    let source = format!(
        "{}\nconst ANSWER: i32 = run()",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/arrays/element-transfer-edit.nct"
        ))
    );
    assert_eq!(answer(&source, "ANSWER"), Datum::integer(42));
    let source = "struct Item { value: i32 }\nconst func inspect(item: Item): i32 { item.value }\nconst func work(): i32 { let items = [Item { value: 20 }, Item { value: 22 }]\ninspect(items[0]^) + items[1].value }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}

#[test]
fn moving_a_captured_element_selects_consuming_invocation_for_the_same_environment() {
    let unit = accepted(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/arrays/element-capture.nct"
    )));
    let (&id, _) = unit.closures().iter().next().unwrap();
    assert_eq!(
        unit.program().closure(id).unwrap().contract().invocation,
        nocter_model::ValueInvocation::Take
    );
}
