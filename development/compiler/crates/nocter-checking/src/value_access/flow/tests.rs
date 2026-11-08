use super::*;
use crate::value_access::{
    Failure, Query as TypedQuery,
    tests::{bound, field, input, string},
};
use nocter_declaration_lowering::access_declarations as declarations;
use nocter_json::Value;
use std::{fs, path::PathBuf, sync::Arc};

pub(super) fn queries(text: &str) -> Vec<Query> {
    let (sources, tree) = input(text);
    let header = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let current = header.bind(bound(&sources, &tree)).unwrap();
    current
        .program()
        .callables()
        .iter()
        .map(|(id, _)| {
            Query::prepare(Arc::new(TypedQuery::prepare(&current, id).unwrap())).unwrap()
        })
        .collect()
}
pub(super) fn named<'a>(queries: &'a [Query], name: &str) -> &'a Query {
    queries
        .iter()
        .find(|query| {
            query
                .typed()
                .program()
                .callables()
                .get(query.typed().callable())
                .unwrap()
                .name()
                == query.typed().program().symbols().get(name).unwrap()
        })
        .unwrap()
}
fn passing(text: &str) -> Vec<Query> {
    let all = queries(text);
    for query in &all {
        assert!(
            query.initialization().violations().is_empty(),
            "{text}: {:?}",
            query.initialization().violations()
        );
    }
    all
}
fn reject(text: &str, state: InitializationState) {
    let all = queries(text);
    let violations = all
        .iter()
        .flat_map(|query| query.initialization().violations())
        .collect::<Vec<_>>();
    assert!(!violations.is_empty(), "{text}");
    assert!(
        violations.iter().any(|violation| violation.state == state),
        "{text}: {violations:?}"
    );
}
fn actions(query: &Query) -> Vec<&Action> {
    query
        .graph()
        .points()
        .iter()
        .filter(|(id, _)| query.initialization().reachable(*id))
        .flat_map(|(_, point)| point.actions.iter())
        .collect()
}

#[test]
fn corpus_rejects_seven_initialization_cases_without_claiming_the_other_four_safe() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/v2-access");
    let manifest =
        nocter_json::parse(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    let Value::Array(cases) = field(&manifest, "cases") else {
        panic!("cases");
    };
    let mut counts = [0; 3];
    for case in cases {
        let expectation = string(field(case, "expectation"));
        if matches!(expectation, "syntax-reject" | "declaration-reject")
            || string(field(case, "body_typing")) == "reject"
        {
            continue;
        }
        let text = fs::read_to_string(root.join(string(field(case, "file")))).unwrap();
        let all = queries(&text);
        let rejected = all
            .iter()
            .any(|query| !query.initialization().violations().is_empty());
        assert_eq!(
            rejected,
            string(field(case, "slot_initialization")) == "reject",
            "{}",
            string(field(case, "id"))
        );
        counts[if rejected {
            0
        } else if expectation == "accept" {
            1
        } else {
            2
        }] += 1;
        if rejected {
            let (sources, tree) = input(&text);
            let header = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
            let current = header.bind(bound(&sources, &tree)).unwrap();
            let required = string(field(case, "primary_text"));
            let start = text.find(required).unwrap();
            let mut found_primary = false;
            for (id, _) in current.program().callables().iter() {
                let query =
                    Query::prepare(Arc::new(TypedQuery::prepare(&current, id).unwrap())).unwrap();
                for diagnostic in query.bind(&current).unwrap().diagnostics().unwrap() {
                    assert_eq!(diagnostic.source().code(), "E0378");
                    assert!(diagnostic.source().notes().len() >= 2);
                    let range = diagnostic.source().primary().span().range();
                    found_primary |= usize::try_from(range.start().get()).unwrap() >= start
                        && usize::try_from(range.end().get()).unwrap() <= start + required.len();
                }
            }
            assert!(found_primary, "{}", string(field(case, "id")));
        }
    }
    assert_eq!(counts, [7, 19, 4]);
}

#[test]
fn assignment_uses_rhs_state_and_cleanup_is_conditional() {
    let all = passing(
        "func run(value: ^error, other: ^error, flag: bool): ^error {\n if flag { drop value }\n value = ^other\n return ^value\n}\n",
    );
    let run = named(&all, "run");
    assert!(
        run.initialization()
            .cleanup()
            .iter()
            .any(|fact| fact.state == InitializationState::Maybe)
    );
    let all = passing("func run(value: ^error): ^error {\n value = ^value\n return ^value\n}\n");
    let run = &all[0];
    for fact in run.initialization().cleanup() {
        let operation = &run
            .graph()
            .points()
            .get(fact.location.point)
            .unwrap()
            .actions[fact.location.action]
            .operation;
        if matches!(operation, Operation::Assign { .. }) {
            assert_eq!(fact.state, InitializationState::Uninitialized);
        }
    }
    reject(
        "func run(value: ^i32): void {\n drop value\n drop value\n}\n",
        InitializationState::Uninitialized,
    );
}

#[test]
fn loop_backedges_and_zero_iteration_paths_are_both_required() {
    passing(
        "func run(value: ^error, flag: bool): ^error {\n while flag {\n let next = ^value\n value = ^next\n continue\n }\n return ^value\n}\n",
    );
    reject(
        "func run(value: ^error, flag: bool): void {\n while flag { let next = ^value }\n}\n",
        InitializationState::Maybe,
    );
    reject(
        "func run(value: ^i32, flag: bool): void {\n drop value\n while flag { value = 1 }\n let inspect = value\n}\n",
        InitializationState::Maybe,
    );
    passing("func run(): ^i32 { while true {} }\n");
}

#[test]
fn dead_edges_do_not_contribute_moves_or_uses() {
    passing(
        "func take(value: ^error): bool { true }\nfunc run(value: ^error): void {\n let first = false && take(value)\n let second = true || take(value)\n while false { drop value }\n if false { drop value }\n let inspect = value\n}\n",
    );
    let all = passing(
        "func run(value: ^i32): i32 {\n return value\n let moved = ^value\n let read = value\n}\n",
    );
    assert!(
        !actions(&all[0])
            .iter()
            .any(|action| matches!(action.operation, Operation::Bind { .. }))
    );
    reject(
        "func take(value: ^error): bool { true }\nfunc run(value: ^error): void {\n let answer = true && take(value)\n let inspect = value\n}\n",
        InitializationState::Uninitialized,
    );
}

#[test]
fn call_reservation_is_not_an_early_transfer_or_read() {
    let all = passing(
        "func take(value: ^error, other: i32): void {}\nfunc run(value: ^error, flag: bool): ^i32 {\n take(value, if flag { return 1 } else { 2 })\n return 0\n}\n",
    );
    let run = named(&all, "run");
    let events = actions(run);
    assert!(
        events
            .iter()
            .any(|action| matches!(action.operation, Operation::CancelCall { prepared: 1, .. }))
    );
    let call = events
        .iter()
        .find_map(|action| {
            if let Operation::CommitCall(call) = action.operation {
                Some(call)
            } else {
                None
            }
        })
        .unwrap();
    let crate::value_access::ExpressionKind::Call { arguments, .. } =
        &run.typed().body().expressions().get(call).unwrap().kind
    else {
        panic!("call");
    };
    let argument = arguments[0].value;
    assert!(
        !events
            .iter()
            .any(|action| action.operation == Operation::Evaluate(argument))
    );
    let value = run.typed().body().locals().iter().next().unwrap().0;
    let cleanup = run
        .initialization()
        .cleanup()
        .iter()
        .filter(|fact| fact.local == value)
        .map(|fact| fact.state)
        .collect::<Vec<_>>();
    assert!(cleanup.contains(&InitializationState::Initialized));
    assert!(cleanup.contains(&InitializationState::Uninitialized));
}

#[test]
fn return_stages_before_interleaved_scope_and_call_cancellation() {
    let all = passing(
        "func forward(value: ^error): ^error { ^value }\nfunc take(left: ^error, right: i32): void {}\nfunc run(value: ^error, flag: bool): ^i32 {\n take(forward(value), if flag { return 1 } else { 2 })\n return 0\n}\n",
    );
    let run = named(&all, "run");
    let point =
        run.graph()
            .points()
            .iter()
            .map(|(_, point)| point)
            .find(|point| {
                point.actions.iter().any(|action| {
                    matches!(action.operation, Operation::CancelCall { prepared: 1, .. })
                })
            })
            .unwrap();
    let kinds = point
        .actions
        .iter()
        .map(|action| &action.operation)
        .collect::<Vec<_>>();
    let stage = kinds
        .iter()
        .position(|action| matches!(action, Operation::StageReturn(_)))
        .unwrap();
    let cancel = kinds
        .iter()
        .position(|action| matches!(action, Operation::CancelCall { prepared: 1, .. }))
        .unwrap();
    assert!(matches!(kinds[stage + 1], Operation::ExitScope { .. }));
    assert!(stage < cancel);
    assert!(matches!(kinds[cancel + 1], Operation::ExitScope { .. }));
}

#[test]
fn initialization_is_not_an_alias_or_escape_certificate() {
    for text in [
        "func take(left: ^i32, right: ^i32): void {}\nfunc run(value: ^i32): void { take(value, value) }\n",
        "func run(value: ^i32): void {\n let edit = &value\n value += 1\n edit += 1\n}\n",
        "func run(): &i32 {\n let value = 0\n return &value\n}\n",
        "func take(value: ^i32, other: i32): void {}\nfunc run(value: ^i32): void {\n take(value, if true { value = 2\n 0 } else { 0 })\n}\n",
    ] {
        passing(text);
    }
}

#[test]
fn diagnostics_rebind_current_sources_without_resolving_flow_again() {
    let source = "func run(value: ^i32): i32 {\n let next = ^value\n return value\n}\n";
    let (sources, tree) = input(source);
    let header = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let current = header.bind(bound(&sources, &tree)).unwrap();
    let id = current.program().callables().iter().next().unwrap().0;
    let query = Query::prepare(Arc::new(TypedQuery::prepare(&current, id).unwrap())).unwrap();
    let (later_sources, later_tree) = input(&format!("\n\n{source}"));
    let later = header.bind(bound(&later_sources, &later_tree)).unwrap();
    let context = query.bind(&later).unwrap();
    assert!(std::ptr::eq(context.query().graph(), query.graph()));
    for diagnostic in context.diagnostics().unwrap() {
        assert_eq!(diagnostic.source().primary().source(), later_tree.source());
        assert!(
            diagnostic
                .source()
                .notes()
                .iter()
                .all(|note| note.origin().source() == later_tree.source())
        );
    }
    let (changed_sources, changed_tree) =
        input(&source.replace("let next = ^value", "let next: ^i32 = value"));
    let changed = header.bind(bound(&changed_sources, &changed_tree)).unwrap();
    assert!(matches!(query.bind(&changed), Err(Failure::ChangedBody)));
    let independent_header = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let independent = independent_header.bind(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        query.bind(&independent),
        Err(Failure::ChangedAuthority)
    ));
}

#[test]
fn nested_loop_condition_uses_the_already_selected_outer_break_target() {
    passing(
        "func run(flag: bool): void {\n loop {\n while (if flag { break } else { false }) {}\n }\n}\n",
    );
}

#[test]
fn primitive_operands_are_prepared_before_a_later_operand_can_consume_the_source() {
    let all = passing(
        "func consume(value: ^i32): i32 { value }\nfunc run(value: ^i32): ^i32 { value + consume(value) }\n",
    );
    let run = named(&all, "run");
    let events = actions(run);
    let first = events
        .iter()
        .position(|action| {
            matches!(
                action.operation,
                Operation::PrepareOperand { operand: 0, .. }
            )
        })
        .unwrap();
    let call = events
        .iter()
        .position(|action| matches!(action.operation, Operation::CommitCall(_)))
        .unwrap();
    let second = events
        .iter()
        .position(|action| {
            matches!(
                action.operation,
                Operation::PrepareOperand { operand: 1, .. }
            )
        })
        .unwrap();
    assert!(first < call && call < second);
}

#[test]
fn ordinary_scope_exit_preserves_reverse_cleanup_and_staged_output() {
    let all = passing(
        "func run(first: ^error, second: ^error): ^error {\n let moved = ^first\n let other = ^second\n return ^moved\n}\n",
    );
    let query = &all[0];
    let mut all_locals = query
        .typed()
        .body()
        .locals()
        .iter()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    all_locals.reverse();
    let events = actions(query);
    let stage = events
        .iter()
        .position(|action| matches!(action.operation, Operation::StageReturn(_)))
        .unwrap();
    let Operation::ExitScope { locals, .. } = &events[stage + 1].operation else {
        panic!("scope exit");
    };
    assert_eq!(locals.as_ref(), all_locals);
    let facts = query
        .initialization()
        .cleanup()
        .iter()
        .map(|fact| fact.state)
        .collect::<Vec<_>>();
    assert_eq!(
        facts,
        [
            InitializationState::Initialized,
            InitializationState::Uninitialized,
            InitializationState::Uninitialized,
            InitializationState::Uninitialized
        ]
    );
}

#[test]
fn nested_preparation_cancels_each_completed_prefix_in_stack_order() {
    let all = passing(
        "func outer(value: ^error, number: i32): void {}\nfunc inner(first: i32, second: i32): i32 { first + second }\nfunc run(value: ^error, flag: bool): ^i32 {\n outer(value, inner(0, if flag { return 1 } else { 2 }))\n return 0\n}\n",
    );
    let run = named(&all, "run");
    let point = run
        .graph()
        .points()
        .iter()
        .map(|(_, point)| point)
        .find(|point| {
            point
                .actions
                .iter()
                .filter(|action| matches!(action.operation, Operation::CancelCall { .. }))
                .count()
                == 2
        })
        .unwrap();
    let calls = point
        .actions
        .iter()
        .filter_map(|action| {
            if let Operation::CancelCall { call, prepared } = action.operation {
                assert_eq!(prepared, 1);
                Some(call)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let targets = calls
        .iter()
        .map(
            |call| match run.typed().body().expressions().get(*call).unwrap().kind {
                crate::value_access::ExpressionKind::Call { target, .. } => target,
                _ => panic!("call"),
            },
        )
        .collect::<Vec<_>>();
    assert_eq!(
        targets,
        [
            named(&all, "inner").typed().callable(),
            named(&all, "outer").typed().callable()
        ]
    );
}

#[test]
fn sibling_declaration_order_does_not_change_body_flow_or_slot_facts() {
    let first = "func run(value: ^i32): i32 {\n helper(value)\n return value\n}\n";
    let second = "func helper(value: ^i32): void {}\n";
    let forward = queries(&format!("{first}{second}"));
    let reverse = queries(&format!("{second}{first}"));
    for name in ["run", "helper"] {
        let a = named(&forward, name);
        let b = named(&reverse, name);
        assert_eq!(a.graph(), b.graph());
        assert_eq!(
            a.initialization().violations(),
            b.initialization().violations()
        );
        assert_eq!(a.initialization().cleanup(), b.initialization().cleanup());
    }
}
