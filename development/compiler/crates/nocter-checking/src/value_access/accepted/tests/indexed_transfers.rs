//! Source-qualified concrete and retained whole-element holes; general frontiers remain gated.
use super::initializers::answer;
use super::*;
use crate::value_access::ExpressionKind;
use nocter_constant_evaluation::state::Value as Datum;

#[test]
fn retained_element_sources_and_point_or_prefix_backedges_execute() {
    for &(_, source) in nocter_test_support::RETAINED_ARRAY_SOURCES {
        assert_eq!(
            answer(&format!("{source}\nconst ANSWER: i32 = run()"), "ANSWER"),
            Datum::integer(42)
        );
    }
}

#[test]
fn flow_proven_constants_freeze_selection_without_saved_index_owners() {
    let source = nocter_test_support::RETAINED_ARRAY_SOURCES
        .iter()
        .find(|(name, _)| *name == "proven-read-after-dynamic-take")
        .unwrap()
        .1;
    let unit = accepted(source);
    for name in [
        "plain",
        "called",
        "aliased",
        "restored",
        "sequential",
        "bundled",
    ] {
        let query = named(&unit, name);
        let trace = super::support::steps(query);
        assert!(
            !trace.iter().any(|step| matches!(step,
            Step::Move { destination, .. }
                if matches!(destination.holder(), Holder::IndexSnapshot(_)))),
            "{name}"
        );
        // This flow proof does not rewrite the original checking-owned bounds disposition.
        assert!(
            query
                .typed()
                .body()
                .expressions()
                .iter()
                .any(|(_, value)| matches!(
                    value.kind,
                    ExpressionKind::Index {
                        bounds: nocter_model::IndexBoundsCheck::Required,
                        ..
                    }
                )),
            "{name}"
        );
    }
}

#[test]
fn selection_refinement_preserves_holes_and_frozen_alias_conflicts() {
    for body in [
        "var index: usize = 0\nlet moved = items[index]^\nitems[index].value",
        "var index = input\nlet moved = items[index]^\nindex = 1\nitems[index].value",
        "var index: usize = 0\nread first = items[index]\nindex = 1\nlet moved = items[0]^\nfirst.value",
        "var index: usize = 0\nread first = items[index]\nindex = 1\nitems[0] = Item { value: 42 }\nfirst.value",
    ] {
        let source = format!(
            "struct Item {{ value: i32 }}\nfunc work(input: usize): i32 {{ var items = [Item {{ value: 40 }}, Item {{ value: 2 }}]\n{body} }}"
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
fn a_callee_write_through_an_alias_cannot_preserve_the_old_counter_progression() {
    let source = "struct Item { value: i32 }\nfunc reset(edit value: usize): void { edit target = value\ntarget = 0 }\nfunc work(): void { let items = [Item { value: 40 }, Item { value: 2 }]\nvar index: usize = 0\nwhile index < 2 { let item = items[index]^\nindex += 1\nreset(index) } }";
    let (sources, tree) = input(source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
    let source = source.replace("reset(index)", "edit selected = index\nreset(selected)");
    let (sources, tree) = input(&source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}

#[test]
fn point_restoration_before_a_later_prefix_epoch_keeps_its_ordinary_transition() {
    let source = nocter_test_support::RETAINED_ARRAY_SOURCES
        .iter()
        .find(|(name, _)| *name == "dynamic-element-prefix-epochs")
        .unwrap()
        .1;
    let unit = accepted(source);
    let query = named(&unit, "run");
    let first = query
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(_, expression)| match expression.kind {
            ExpressionKind::Move(crate::value_access::Referent::Evaluated(selection)) => {
                Some(selection)
            }
            _ => None,
        })
        .unwrap();
    let trace = super::support::steps(query);
    assert!(
        trace
            .iter()
            .any(|step| matches!(step, Step::MoveArrayElement { .. }))
    );
    assert!(!trace.iter().any(|step| matches!(step,
        Step::MoveArrayElement { index, .. } if index.selection() == first)));
}

#[test]
fn cancelling_a_tail_reservation_preserves_the_last_committed_prefix() {
    let source = nocter_test_support::RETAINED_ARRAY_SOURCES
        .iter()
        .find(|(name, _)| *name == "dynamic-element-prefix-array-cancel")
        .unwrap()
        .1;
    let unit = accepted(source);
    let query = named(&unit, "work");
    let cancelled = query
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(_, expression)| {
            let ExpressionKind::Call { ref arguments, .. } = expression.kind else {
                return None;
            };
            match arguments.first()?.action {
                crate::value_access::ArgumentAction::ReserveTransfer {
                    place: crate::value_access::Referent::Evaluated(id),
                    ..
                } => Some(id),
                _ => None,
            }
        })
        .unwrap();
    let trace = super::support::steps(query);
    assert!(
        trace
            .iter()
            .any(|step| matches!(step, Step::ConsolidateArrayPrefix { .. }))
    );
    assert!(!trace.iter().any(|step| matches!(step,
        Step::MoveArrayElement { index, .. } if index.selection() == cancelled)));
    assert!(!trace.iter().any(|step| matches!(step,
        Step::Move { destination, .. } if matches!(destination.holder(), crate::value_access::flow::resources::Holder::IndexSnapshot(_)))));
    assert!(
        query
            .graph()
            .points()
            .iter()
            .any(|(_, point)| point.actions.iter().any(|action| matches!(
                action.operation,
                flow::Operation::CancelCall { prepared: 1, .. }
            )))
    );
    // The same source recipe is insufficient: a cancelled reservation still needs an
    // initialized element. A committed call inside the loop also prevents the next prefix
    // proof until its closed write summary is available.
    for source in [
        source.replace("items[index + 1]^", "items[index]^"),
        source.replace("if true { return total } else { 0 }", "0"),
    ] {
        let (sources, tree) = input(&source);
        assert!(matches!(
            SourceUnit::prepare(bound(&sources, &tree)),
            Err(SourceFailure::Checking(Failure::Flow { .. }))
        ));
    }
}

#[test]
fn prefix_call_handoffs_protect_each_payload_before_replacing_the_endpoint() {
    use crate::value_access::flow::resources::Holder;
    let source = nocter_test_support::RETAINED_ARRAY_SOURCES
        .iter()
        .find(|(name, _)| *name == "dynamic-element-prefix-call")
        .unwrap()
        .1;
    let unit = accepted(source);
    let query = named(&unit, "work");
    let mut calls = 0;
    for (point, block) in query.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            let flow::Operation::CommitCall(call) = event.operation else {
                continue;
            };
            calls += 1;
            let transitions: Vec<_> = query
                .access()
                .resources()
                .at(Location { point, action })
                .collect();
            assert!(!transitions.is_empty());
            for transition in transitions {
                let before = transition.before_steps();
                let delivery = before.iter().position(|step| matches!(step,
                    Step::MoveArrayElement { destination: Holder::Prepared { call: actual, index: 0 }, .. }
                        if *actual == call)).unwrap();
                let endpoint = before.iter().position(|step| matches!(step,
                    Step::Move { destination, .. } if matches!(destination.holder(), Holder::ArrayFrontier(_)))).unwrap();
                let consolidate = before
                    .iter()
                    .position(|step| matches!(step, Step::ConsolidateArrayPrefix { .. }))
                    .unwrap();
                let handoff = before
                    .iter()
                    .position(|step| match step {
                        Step::Invoke { source, .. } | Step::Move { source, .. } => {
                            source.holder() == Holder::Prepared { call, index: 0 }
                        }
                        _ => false,
                    })
                    .unwrap();
                assert!(delivery < endpoint && endpoint < consolidate && consolidate < handoff);
            }
        }
    }
    assert_eq!(calls, 3);
    let (sources, tree) =
        input(&source.replace("consume(items[index]^", "consume(items[index - 1]^"));
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}

#[test]
fn a_prefix_bundle_needs_ordered_numeric_proofs_and_cancellation_retires_no_argument() {
    use crate::value_access::flow::resources::Holder;
    let source = nocter_test_support::RETAINED_ARRAY_SOURCES
        .iter()
        .find(|(name, _)| *name == "dynamic-element-prefix-bundle")
        .unwrap()
        .1;
    let unit = accepted(source);
    let query = named(&unit, "work");
    let mut committed = 0;
    let mut cancelled = 0;
    for (point, block) in query.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            let transitions: Vec<_> = query
                .access()
                .resources()
                .at(Location { point, action })
                .collect();
            match event.operation {
                flow::Operation::CommitCall(call) => {
                    committed += 1;
                    assert!(!transitions.is_empty());
                    for transition in transitions {
                        let arguments: Vec<_> = transition
                            .before_steps()
                            .iter()
                            .filter_map(|step| {
                                if let Step::MoveArrayElement {
                                    destination:
                                        Holder::Prepared {
                                            call: actual,
                                            index,
                                        },
                                    ..
                                } = step
                                {
                                    assert_eq!(*actual, call);
                                    Some(*index)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        assert_eq!(arguments, [0, 1]);
                    }
                }
                flow::Operation::CancelCall { prepared: 2, .. } => {
                    cancelled += 1;
                    assert!(!transitions.is_empty());
                    assert!(
                        transitions
                            .iter()
                            .all(|transition| transition.steps().is_empty())
                    );
                }
                _ => {}
            }
        }
    }
    assert_eq!((committed, cancelled), (1, 1));
    for replacement in [
        "items[index]^",
        "items[if true { index = 0\nindex } else { index }]^",
    ] {
        let (sources, tree) = input(&source.replace("items[index + 1]^", replacement));
        assert!(matches!(
            SourceUnit::prepare(bound(&sources, &tree)),
            Err(SourceFailure::Checking(Failure::Flow { .. }))
        ));
    }
}

#[test]
fn a_no_caller_write_proof_does_not_restore_a_consumed_counter() {
    let (sources, tree) = input(
        "struct Item { value: i32 }\nfunc consume(take index: usize): void {}\nfunc work(): void { let items = [Item { value: 40 }, Item { value: 2 }]\nvar index: usize = 0\nwhile index < 2 { let selected = items[index]^\nconsume(index^)\nindex += 1 } }",
    );
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}

#[test]
fn interrupted_write_arguments_do_not_request_uncommitted_effect_bindings() {
    let source = "const func write(edit value: usize): void { value = 9 }\nconst func run(): usize { write(if true { return 42 } else { 0 })\n7 }\nconst ANSWER: usize = run()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}

#[test]
fn dynamic_holes_block_unproved_reads_restoration_and_nonprogressing_recipe_execution() {
    for after in [
        "read later = items",
        "read later = items[index]",
        "let later = items[index]^",
        "items[index + 1] = Item { value: 0 }\nlet complete = items^",
        "items[0] = Item { value: 0 }\nlet complete = items^",
    ] {
        let source = format!(
            "struct Item {{ value: i32 }}\nfunc work(take items: [Item; 2], index: usize): void {{ let selected = items[index]^\n{after} }}"
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
    let (sources, tree) = input(
        "struct Item { value: i32 }\nfunc work(take items: [Item; 2]): void { var index: usize = 0\nwhile index < 2 { let selected = items[index]^\nindex += 0 } }",
    );
    // Re-executing a recipe does not supply the same moved element again.
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}

#[test]
fn restoring_the_last_prefix_element_does_not_restore_the_entire_prefix() {
    let source = "func work(): void { var items = [(); 3]\nvar index: usize = 0\nloop { let old = items[index]^\nindex += 1\nif index == 2 { break } }\nitems[index - 1] = ()\nlet complete = items^ }";
    let (sources, tree) = input(source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}

#[test]
fn restoring_a_point_after_another_field_hole_does_not_establish_a_complete_epoch() {
    let source = "struct Pair { left: i32\nright: i32 }\nfunc work(): void { var rows = [Pair { left: 40, right: 2 }, Pair { left: 20, right: 22 }]\nlet first = rows[0].left^\nlet second = rows[1]^\nrows[1] = second^\nvar index: usize = 0\nwhile index < 2 { let row = rows[index]^\nindex += 1 } }";
    let (sources, tree) = input(source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}

#[test]
fn retained_restoration_installs_before_retiring_backing_without_destroying_missing_data() {
    let unit = accepted(
        "func work(take original: error, take replacement: error, index: usize): void { var items = [original^]\nlet extracted = items[index]^\nitems[index] = replacement^ }",
    );
    let query = named(&unit, "work");
    let location = query
        .graph()
        .points()
        .iter()
        .find_map(|(point, block)| {
            block
                .actions
                .iter()
                .enumerate()
                .find_map(|(action, event)| {
                    matches!(event.operation, Operation::Assign { .. })
                        .then_some(Location { point, action })
                })
        })
        .unwrap();
    let transitions: Vec<_> = query.access().resources().at(location).collect();
    assert!(!transitions.is_empty());
    for transition in transitions {
        let steps = transition.steps();
        let (installed, destination) = steps
            .iter()
            .enumerate()
            .find_map(|(at, step)| match step {
                Step::Move { destination, .. }
                    if destination.fields().retained().next().is_some() =>
                {
                    Some((at, destination))
                }
                _ => None,
            })
            .unwrap();
        let snapshot = Holder::IndexSnapshot(destination.fields().retained().next().unwrap());
        let retired = steps
            .iter()
            .position(|step| {
                matches!(step,
            Step::Dispose { source, .. } if source.holder() == snapshot)
            })
            .unwrap();
        assert!(installed < retired);
        assert!(transition.initialized_after(destination.holder()));
        assert!(
            !transition
                .after_condition()
                .owned()
                .any(|(holder, _)| holder == snapshot)
        );
        assert!(!steps.iter().any(|step| matches!(step,
            Step::Dispose { payload, .. } if payload.disposal() == Disposal::OwnedError)));
    }
}

#[test]
fn conditional_restoration_uses_initialized_and_saved_point_alternatives() {
    let unit = accepted(
        "func work(take original: error, take replacement: error, index: usize, flag: bool): void { var items = [original^]\nif flag { let old = items[index]^ }\nitems[index] = replacement^ }",
    );
    let query = named(&unit, "work");
    let mut kinds = [false; 2];
    for (point, block) in query.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            if !matches!(event.operation, Operation::Assign { .. }) {
                continue;
            }
            for transition in query.access().resources().at(Location { point, action }) {
                let snapshots: Vec<_> = transition
                    .condition()
                    .owned()
                    .filter_map(|(holder, _)| {
                        matches!(holder, Holder::IndexSnapshot(_)).then_some(holder)
                    })
                    .collect();
                let replace = transition
                    .steps()
                    .iter()
                    .any(|step| matches!(step, Step::ReplaceReferent { .. }));
                kinds[usize::from(replace)] = true;
                if replace {
                    assert!(snapshots.is_empty());
                } else {
                    assert_eq!(snapshots.len(), 1);
                }
                assert!(
                    !transition
                        .after_condition()
                        .owned()
                        .any(|(holder, _)| matches!(holder, Holder::IndexSnapshot(_)))
                );
            }
        }
    }
    assert_eq!(kinds, [true, true]);
}

#[test]
fn conditional_restoration_does_not_assume_different_origin_indices_are_equal() {
    for body in [
        "if flag { let old = items[left]^ }\nitems[left + 1] = Item { value: 42 }",
        "if flag { let old = items[left]^ } else { let old = items[right]^ }\nitems[left] = Item { value: 42 }",
    ] {
        let source = format!(
            "struct Item {{ value: i32 }}\nfunc work(left: usize, right: usize, flag: bool): void {{ var items = [Item {{ value: 20 }}, Item {{ value: 22 }}]\n{body}\nlet complete = items^ }}"
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
fn dynamic_transfer_cancellation_creates_no_snapshot_or_hole_and_bounds_fail_first() {
    let copied = "const func work(take values: [i32; 2], index: usize): i32 { let selected = values[index]^\nselected }\nconst ANSWER: i32 = work([20, 22], 0) + work([20, 22], 1)";
    assert_eq!(answer(copied, "ANSWER"), Datum::integer(42));

    let source = "struct Item { value: i32 }\nconst func consume(take item: Item, other: i32): i32 { item.value + other }\nconst func work(take items: [Item; 2], index: usize): i32 { consume(items[index]^, if true { return 42 } else { 0 }) }\nconst func run(): i32 { work([Item { value: 1 }, Item { value: 2 }], 0) }\nconst ANSWER: i32 = run()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    let unit = accepted(source);
    let trace = super::support::steps(named(&unit, "work"));
    assert!(!trace.iter().any(|step| matches!(step,
        Step::Move { destination, .. } if matches!(destination.holder(), crate::value_access::flow::resources::Holder::IndexSnapshot(_)))));
    assert!(!trace.iter().any(|step| matches!(
        step,
        Step::DisposeArrayElements { .. } | Step::Invoke { .. }
    )));
    let (sources, tree) = input(
        "const func work(take items: [void; 2], index: usize): i32 { let selected = items[index]^\n42 }\nconst ANSWER: i32 = work([(), ()], 2)",
    );
    assert!(
        matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) if diagnostic.rule() == crate::value_access::evaluation::Rule::Execution(nocter_constant_evaluation::state::Rule::IndexOutOfBounds))
    );
}

#[test]
fn terminal_bounds_failure_never_grants_access_through_a_missing_array() {
    // Identifying the outer slot is allowed, but no inner place/value is ever produced.
    let (sources, tree) = input(
        "const func work(): void { let values: [[void; 0]; 2] = [[], []]\nlet moved = values[0]^\nlet _ = values[0][0] }\nconst ANSWER: void = work()",
    );
    assert!(
        matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) if diagnostic.rule() == crate::value_access::evaluation::Rule::Execution(nocter_constant_evaluation::state::Rule::IndexOutOfBounds))
    );
    // A normally completed inner selection does need the missing containing array.
    let (sources, tree) = input(
        "func work(): void { let values: [[void; 1]; 2] = [[()], [()]]\nlet moved = values[0]^\nlet _ = values[0][0] }",
    );
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}

#[test]
fn a_proven_element_transfer_leaves_siblings_and_retires_the_exact_source() {
    let source = "struct Item { value: i32 }\nconst func work(): i32 { let items = [Item { value: 40 }, Item { value: 2 }]\nlet first = items[0]^\nfirst.value + items[1].value }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    let unit = accepted(source);
    let trace = super::support::steps(named(&unit, "work"));
    assert!(trace.iter().any(|step| matches!(
        step,
        Step::DisposeArrayElements {
            range: nocter_model::ArrayRange::Constant { start: 1, end: 2 },
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
fn partial_dynamic_fields_do_not_restore_unproved_holes_or_capture_cancelled_arguments() {
    let source = nocter_test_support::RETAINED_ARRAY_SOURCES
        .iter()
        .find(|(name, _)| *name == "dynamic-element-field-hole")
        .unwrap()
        .1;
    let unit = accepted(source);
    let cancelled = named(&unit, "cancelled");
    let trace = super::support::steps(cancelled);
    assert!(!trace.iter().any(|step| matches!(step, Step::Invoke { .. })));
    assert!(!trace.iter().any(|step| matches!(step,
        Step::Move { destination, .. }
            if matches!(destination.holder(), crate::value_access::flow::resources::Holder::IndexSnapshot(_)))));
    for use_after in [
        "read view = items[index]",
        "let _ = items[other].first.value",
        "let _ = items[index].first^",
        "items[other].first = Item { value: 9 }",
    ] {
        let source = format!(
            "struct Item {{ value: i32 }}\nstruct Pair {{ first: Item\nsecond: Item }}\nfunc work(index: usize, other: usize): void {{ var items = [Pair {{ first: Item {{ value: 40 }}, second: Item {{ value: 2 }} }}; 1]\nlet moved = items[index].first^\n{use_after} }}"
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
fn disjoint_fields_allow_multiple_dynamic_holes_in_the_same_array() {
    let source = nocter_test_support::RETAINED_ARRAY_SOURCES
        .iter()
        .find(|(name, _)| *name == "multiple-dynamic-element-fields")
        .unwrap()
        .1;
    let unit = accepted(source);
    assert!(super::support::steps(named(&unit, "work")).iter().any(
        |step| matches!(step, Step::DisposeArrayRemainder { missing, .. } if missing.len() == 2)
    ));
}

#[test]
fn mixed_remainders_preserve_point_and_prefix_coverage_roles() {
    use crate::value_access::flow::resources::{Missing, Path};
    let source = nocter_test_support::RETAINED_ARRAY_SOURCES
        .iter()
        .find(|(name, _)| *name == "mixed-prefix-element-fields")
        .unwrap()
        .1;
    let unit = accepted(source);
    let trace = super::support::steps(named(&unit, "work"));
    let missing = trace
        .iter()
        .find_map(|step| match step {
            Step::DisposeArrayRemainder { missing, .. }
                if missing
                    .iter()
                    .any(|hole| matches!(hole, Missing::ArrayPrefix { .. })) =>
            {
                Some(missing)
            }
            _ => None,
        })
        .unwrap();
    assert!(missing.iter().any(
        |hole| matches!(hole, Missing::ArrayPrefix { frontier, .. } if frontier.bound() == 3)
    ));
    assert!(
        missing
            .iter()
            .filter_map(Missing::value)
            .flat_map(Path::retained)
            .any(|point| point.bound() == 2)
    );
}
#[test]
fn direct_scalar_alias_updates_cannot_reset_a_counter_or_widen_an_element_replacement() {
    for source in [
        "func work(): void { let items = [1, 2]\nvar index: usize = 0\nedit cursor = index\nwhile cursor < 2 { let moved = items[cursor]^\ncursor = 0 } }",
        "func work(): void { var items = [1, 2]\nlet old = items[0]^\nif true { edit selected = items[1]\nselected = 9 }\nvar index: usize = 0\nwhile index < 2 { let moved = items[index]^\nindex += 1 } }",
    ] {
        let (sources, tree) = input(source);
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
fn initialized_element_replacement_does_not_hide_a_hole_created_by_its_rhs() {
    let source = "func work(): void { var items = [1, 2]\nitems[0] = items[1]^\nvar index: usize = 0\nwhile index < 2 { let moved = items[index]^\nindex += 1 } }";
    let (sources, tree) = input(source);
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
            "func work(values: [[i32; 2]; 2], index: usize, inner: usize): void { let moved = values[index][inner]^ }",
            crate::value_access::Rule::MissingTransfer,
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
                range: nocter_model::ArrayRange::Constant { start: 2, end: 3 },
                ..
            } => Some("last"),
            Step::DisposeArrayElements {
                range: nocter_model::ArrayRange::Constant { start: 0, end: 1 },
                ..
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
