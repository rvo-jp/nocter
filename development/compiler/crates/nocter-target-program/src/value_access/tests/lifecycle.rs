//! Actual source admission and concrete lifecycle phases, without pretending to execute native code.
mod flags;
use super::*;
use crate::value_access::lifecycle::{self as lowering, EnvironmentPart, Step};
use nocter_checking::value_access::flow::{Location, Operation, resources::Step as ResourceStep};

fn lower_source(source: &str) -> lowering::Program {
    let fixture = ValueSourceFixture::new(source);
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let instances = InstanceProgram::prepare(unit, root).unwrap();
    lowering::Program::prepare(Arc::new(instances)).unwrap()
}
fn lower_case(name: &str) -> lowering::Program {
    lowering::Program::prepare(Arc::new(close_case(name))).unwrap()
}
fn operation(program: &lowering::Program, id: AccessInstanceId, at: Location) -> &Operation {
    &program
        .instance_program()
        .body(id)
        .unwrap()
        .graph()
        .points()
        .get(at.point)
        .unwrap()
        .actions[at.action]
        .operation
}
fn steps(program: &lowering::Program) -> impl Iterator<Item = &Step> {
    program
        .bodies()
        .values()
        .flat_map(|body| body.actions().values())
        .flat_map(lowering::Action::alternatives)
        .flat_map(|alternative| alternative.before().iter().chain(alternative.after()))
}

#[test]
fn invocation_transfers_precede_the_body_and_adapter_cleanup_precedes_receipt() {
    let program = lower_source(
        "func observe(input: error): void {}\nfunc apply(take action: once(): void): void { action^() }\nfunc run(take input: error): void {\nlet action = (input^;): void { observe(input) }\napply(action^)\n}",
    );
    let (apply, _) = instance(program.instance_program(), "apply");
    let schedule = program.bodies()[&apply]
        .actions()
        .iter()
        .find(|(at, _)| matches!(operation(&program, apply, **at), Operation::CommitCall(_)))
        .unwrap()
        .1;
    for alternative in schedule.alternatives() {
        assert!(alternative.before().iter().any(|step| matches!(step.resource(), Some(ResourceStep::Invoke { payload, .. }) if payload.disposal() == Disposal::OwnedCapture)));
        assert!(matches!(alternative.after(), [
            Step::DisposeEnvironment { argument: 0, part: EnvironmentPart::Capture, payload, .. },
            Step::Resource(received)
        ] if payload.disposal() == Disposal::OwnedError && matches!(received.operation(), ResourceStep::Receive { .. })));
    }
    for (&id, body) in program.bodies() {
        for (&at, action) in body.actions() {
            if matches!(operation(&program, id, at), Operation::CommitCall(_)) {
                for alternative in action.alternatives() {
                    assert!(
                        !alternative
                            .before()
                            .iter()
                            .any(|s| matches!(s.resource(), Some(ResourceStep::Receive { .. })))
                    );
                    assert!(
                        !alternative
                            .after()
                            .iter()
                            .any(|s| matches!(s.resource(), Some(ResourceStep::Invoke { .. })))
                    );
                }
            }
        }
    }
    for step in steps(&program) {
        let resource = match step {
            Step::Resource(step) => Some(step.operation()),
            Step::ScopedReplacement(replacement) => Some(replacement.operation()),
            Step::MoveLocalFuture { .. }
            | Step::CaptureFutureInputs { .. }
            | Step::DisposeEnvironment { .. } => None,
        };
        if let Some(step) = resource {
            step.try_map_payloads(|payload| {
                assert_ne!(payload.disposal(), Disposal::FunctionEnvironment);
                Ok::<_, ()>(payload.clone())
            })
            .unwrap();
        }
    }
}

#[test]
fn a_consuming_body_keeps_its_partial_cleanup_without_second_adapter_destruction() {
    let program = lower_case("a141-owned-capture-partial.nct");
    let root = program
        .instance_program()
        .instances()
        .get(program.instance_program().root())
        .unwrap();
    let selected = root.calls().values().next().unwrap();
    let Environment::Capture { input, .. } = selected.environment() else {
        panic!("capture");
    };
    let body = &program.bodies()[&selected.target().instance().unwrap()];
    let mut disposed = Vec::new();
    let mut released = Vec::new();
    for alternative in body
        .actions()
        .values()
        .flat_map(lowering::Action::alternatives)
    {
        for step in alternative.before().iter().chain(alternative.after()) {
            match step.resource() {
                Some(ResourceStep::Dispose { source, .. })
                    if source.holder()
                        == nocter_checking::value_access::flow::resources::Holder::Local(
                            input.local,
                        ) =>
                {
                    disposed.push(source);
                }
                Some(ResourceStep::ReleaseContainer { source, .. }) => {
                    released.push(source);
                }
                _ => {}
            }
        }
    }
    assert_eq!(disposed.len(), 1);
    assert!(!disposed[0].fields().is_root());
    assert_eq!(released.len(), 1);
    assert!(!steps(&program).any(|s| matches!(s, Step::DisposeEnvironment { .. })));
}

#[test]
fn conditional_partial_cleanup_has_distinct_sparse_runtime_conditions() {
    let program = lower_source(
        "struct Pair { first: error\nsecond: error }\nfunc run(take pair: Pair, flag: bool): void {\nif flag { let saved = pair.first^ }\n}",
    );
    let id = program.instance_program().root();
    let (at, conditional) = program.bodies()[&id]
        .actions()
        .iter()
        .find(|(_, action)| {
            action.alternatives().len() == 2
                && action.alternatives().iter().any(|alternative| {
                    alternative.after().iter().any(|s| {
                        matches!(s.resource(), Some(ResourceStep::ReleaseContainer { .. }))
                    })
                })
        })
        .unwrap();
    let alternatives = conditional.alternatives();
    assert_ne!(
        alternatives[0].flags().tests(),
        alternatives[1].flags().tests()
    );
    let mut missing_counts = program
        .instance_program()
        .body(id)
        .unwrap()
        .access()
        .resources()
        .at(*at)
        .map(|transition| {
            transition
                .condition()
                .owned()
                .map(|(_, missing)| missing.len())
                .sum::<usize>()
        })
        .collect::<Vec<_>>();
    missing_counts.sort_unstable();
    assert_eq!(missing_counts, [0, 1]);
    assert!(alternatives.iter().any(|a| matches!(a.after().iter().map(Step::resource).collect::<Vec<_>>().as_slice(), [Some(ResourceStep::Dispose { payload, .. })] if payload.disposal() == Disposal::Aggregate)));
    assert!(alternatives.iter().any(|a| matches!(a.after().iter().map(Step::resource).collect::<Vec<_>>().as_slice(), [Some(ResourceStep::Dispose { payload, .. }), Some(ResourceStep::ReleaseContainer { .. })] if payload.disposal() == Disposal::OwnedError)));
}

#[test]
fn caller_owned_input_moves_before_preparation_but_condition_disposal_follows_reading() {
    let program = lower_source(
        "func touch(edit value: i32): void { value += 1 }\nfunc run(): i32 {\ntouch(1 + 2)\nif 1 == 2 { 3 } else { 4 }\n}",
    );
    let id = program.instance_program().root();
    let body = &program.bodies()[&id];
    let prepared = body
        .actions()
        .iter()
        .find(|(at, _)| {
            matches!(
                operation(&program, id, **at),
                Operation::PrepareArgument { .. }
            )
        })
        .unwrap()
        .1;
    assert!(matches!(
        prepared.alternatives()[0]
            .before()
            .iter()
            .map(Step::resource)
            .collect::<Vec<_>>()
            .as_slice(),
        [Some(ResourceStep::Move { .. })]
    ));
    let condition = body
        .actions()
        .iter()
        .find(|(at, _)| matches!(operation(&program, id, **at), Operation::Condition(_)))
        .unwrap()
        .1;
    for alternative in condition.alternatives() {
        assert!(alternative.before().is_empty());
        assert!(matches!(
            alternative
                .after()
                .iter()
                .map(Step::resource)
                .collect::<Vec<_>>()
                .as_slice(),
            [Some(ResourceStep::Dispose { .. })]
        ));
    }
}

#[test]
fn interrupted_preparation_has_no_call_or_adapter_completion_schedule() {
    let program = lower_source(
        "func sink(take value: error, number: i32): void {}\nfunc run(take value: error, take answer: error): error {\nsink(value^, if true { return answer^ } else { return answer^ })\nvalue^\n}",
    );
    let id = program.instance_program().root();
    assert!(
        program
            .instance_program()
            .instances()
            .get(id)
            .unwrap()
            .calls()
            .is_empty()
    );
    let instance = program.instance_program().instances().get(id).unwrap();
    let typed = program.instance_program().body(id).unwrap().typed().body();
    let call = typed
        .expressions()
        .iter()
        .find_map(|(call, expression)| {
            matches!(
                expression.kind,
                nocter_checking::value_access::ExpressionKind::Call { .. }
            )
            .then_some(call)
        })
        .unwrap();
    assert!(instance.prepared(call, 0).is_some());
    assert!(instance.prepared(call, 1).is_none());
    assert_eq!(program.instance_program().instances().len(), 1);
    assert!(
        !steps(&program).any(|s| matches!(s, Step::DisposeEnvironment { .. })
            || matches!(
                s.resource(),
                Some(ResourceStep::Invoke { .. } | ResourceStep::Receive { .. })
            ))
    );
    assert!(program.bodies()[&id].actions().keys().any(|&at| matches!(
        operation(&program, id, at),
        Operation::CancelCall { prepared: 1, .. }
    )));
}

#[test]
fn one_generic_body_gets_distinct_concrete_lifecycles_for_named_and_captured_code() {
    let program = lower_source(
        "func plain(): void {}\nfunc apply(take action: once(): void): void { action^() }\nfunc run(take input: error): void {\nlet named = plain\nlet captured = (input^;): void {}\napply(named^)\napply(captured^)\n}",
    );
    let apply = callable(program.instance_program().unit(), "apply");
    let bodies = program
        .instance_program()
        .instances()
        .iter()
        .filter(|(_, instance)| instance.key().code() == ValueCode::Declaration(apply))
        .collect::<Vec<_>>();
    assert_eq!(bodies.len(), 2);
    let mut saw_token = false;
    let mut saw_capture = false;
    for (id, _) in bodies {
        for step in program.bodies()[&id]
            .actions()
            .values()
            .flat_map(lowering::Action::alternatives)
            .flat_map(lowering::Alternative::after)
        {
            match step {
                Step::DisposeEnvironment {
                    part: EnvironmentPart::CodeToken,
                    payload,
                    ..
                } => {
                    assert_eq!(payload.disposal(), Disposal::Trivial);
                    saw_token = true;
                }
                Step::DisposeEnvironment {
                    part: EnvironmentPart::Capture,
                    payload,
                    ..
                } => {
                    assert_eq!(payload.disposal(), Disposal::OwnedError);
                    saw_capture = true;
                }
                _ => {}
            }
        }
    }
    assert!(saw_token && saw_capture);
}

#[test]
fn structural_control_and_callable_source_families_have_complete_lifecycle_schedules() {
    for (name, root) in [
        ("a01-observe-noncopy-twice.nct", "preserve"),
        ("a03-fixed-owner-transfer.nct", "forward"),
        ("a07-rhs-first-self-transfer.nct", "forward"),
        ("a08-consume-and-restore.nct", "forward"),
        ("a10-branch-value-transfer.nct", "choose"),
        ("a12-loop-restores-owner.nct", "cycle"),
        ("a16-record-restore.nct", "restore"),
        ("a17-record-partial-drop.nct", "release"),
        ("a20-record-branch-restore.nct", "run"),
        ("a21-record-loop-restore.nct", "run"),
        ("a24-all-fields-moved-drop.nct", "extract"),
        ("a33-record-value-order.nct", "build"),
        ("a36-conditional-record-values.nct", "build"),
        ("a50-read-computed-field.nct", "run"),
        ("a51-take-computed-field.nct", "run"),
        ("a52-computed-field-replacement.nct", "run"),
        ("a65-function-take-input.nct", "run"),
        ("a82-callback-forward-read.nct", "run"),
        ("a84-callback-forward-adapter.nct", "run"),
        ("a92-callback-taking-error.nct", "run"),
        ("a124-dependent-result-relay.nct", "run"),
        ("a125-dependent-result-recursion.nct", "run"),
        ("a136-dependent-result-mut.nct", "run"),
        ("a138-owned-capture-read.nct", "run"),
        ("a140-owned-capture-take.nct", "run"),
        ("a141-owned-capture-partial.nct", "run"),
        ("a147-owned-capture-input-operations.nct", "run"),
        ("a148-edit-input-computed-field.nct", "run"),
    ] {
        let fixture = ValueSourceFixture::corpus(name);
        let unit = accepted(&fixture);
        let entry = callable(&unit, root);
        let instances = InstanceProgram::prepare(unit, entry)
            .unwrap_or_else(|failure| panic!("{name} root {root}: {failure:?}"));
        lowering::Program::prepare(Arc::new(instances))
            .unwrap_or_else(|failure| panic!("{name}: {failure:?}"));
    }
}

#[test]
fn a_consuming_adapter_destroys_the_live_post_update_environment() {
    let program = lower_source(
        "func reset(edit target: error, take replacement: error): void { target = replacement^ }\nfunc apply(take action: once(take error): void, take replacement: error): void { action^(replacement^) }\nfunc run(take input: error, take replacement: error): void {\nlet action = (input^; take next: error): void { reset(input, next^) }\napply(action^, replacement^)\n}",
    );
    let (apply, body) = instance(program.instance_program(), "apply");
    let call = body.calls().values().next().unwrap();
    let Environment::Capture { input, .. } = call.environment() else {
        panic!("capture");
    };
    assert_eq!(input.operation, nocter_model::ValueInputOperation::Edit);
    let action = program.bodies()[&apply]
        .actions()
        .iter()
        .find(|(at, _)| matches!(operation(&program, apply, **at), Operation::CommitCall(_)))
        .unwrap()
        .1;
    for alternative in action.alternatives() {
        assert!(matches!(alternative.after(), [
            Step::DisposeEnvironment { argument: 0, part: EnvironmentPart::Capture, payload, .. },
            Step::Resource(received)
        ] if payload.disposal() == Disposal::OwnedError && matches!(received.operation(), ResourceStep::Receive { .. })));
    }
    assert!(steps(&program).any(|step| matches!(step, Step::ScopedReplacement(_))));
}

#[test]
fn a_lent_callable_temporary_is_destroyed_once_by_its_caller_after_the_edit_call() {
    use nocter_checking::value_access::flow::resources::Holder;
    let program = lower_source(
        "func reset(edit target: error, take replacement: error): void { target = replacement^ }\nfunc lend(edit action: mut(take error): i32, take next: error): i32 { action(next^) }\nfunc run(take held: error, take next: error): i32 {\nlet action = (held^; take replacement: error): i32 { reset(held, replacement^)\n42 }\nlend(action^, next^)\n}",
    );
    let (run, root) = instance(program.instance_program(), "run");
    let (&call, _) = root.calls().iter().next().unwrap();
    assert_eq!(root.calls().len(), 1);
    let temporary = Holder::Temporary { call, index: 0 };
    let call_at = program.bodies()[&run].actions().keys().find(|&&at| {
        matches!(operation(&program, run, at), Operation::CommitCall(selected) if *selected == call)
    }).copied().unwrap();
    let mut disposals = 0;
    let mut transfers = 0;
    for (&at, action) in program.bodies()[&run].actions() {
        for alternative in action.alternatives() {
            for (after, steps) in [(false, alternative.before()), (true, alternative.after())] {
                for step in steps {
                    match step.resource() {
                        Some(ResourceStep::Move {
                            destination,
                            payload,
                            ..
                        }) if destination.holder() == temporary => {
                            assert_eq!(at, call_at);
                            assert!(!after);
                            assert_eq!(payload.disposal(), Disposal::OwnedCapture);
                            transfers += 1;
                        }
                        Some(ResourceStep::Dispose { source, payload })
                            if source.holder() == temporary =>
                        {
                            assert_eq!(at.point, call_at.point);
                            assert!(at.action > call_at.action || at == call_at && after);
                            assert!(source.fields().is_root());
                            assert_eq!(payload.disposal(), Disposal::OwnedCapture);
                            disposals += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    assert_eq!((transfers, disposals), (1, 1));
    let (lend, _) = instance(program.instance_program(), "lend");
    assert!(
        !program.bodies()[&lend]
            .actions()
            .values()
            .flat_map(lowering::Action::alternatives)
            .flat_map(|alternative| alternative.before().iter().chain(alternative.after()))
            .any(|step| matches!(step, Step::DisposeEnvironment { .. }))
    );
}

#[test]
fn scoped_field_replacement_retains_carrier_roots_separately_from_leaf_cleanup() {
    let program = lower_source(
        "struct Pair { first: i32\nsecond: i32 }\nfunc set(edit pair: Pair): void { pair.first = 42 }\nfunc run(): i32 { var bundle = (Pair { first: 41, second: 1 }, 0)\nset(bundle.0)\nedit pair = bundle.0\npair.first = 42\nbundle.0.first }",
    );
    let mut replacements = 0;
    let mut enclosing = 0;
    for step in steps(&program) {
        let Step::ScopedReplacement(replacement) = step else {
            continue;
        };
        let ResourceStep::ReplaceReferent {
            destination,
            retired,
            payload,
            ..
        } = replacement.operation()
        else {
            panic!("matched scoped replacement");
        };
        assert!(!destination.fields().is_root());
        assert_ne!(replacement.root_before().data(), retired.data());
        assert_eq!(replacement.root_before(), replacement.root_after());
        assert_eq!(retired.data(), payload.data());
        assert_eq!(retired.disposal(), Disposal::Trivial);
        let [before] = replacement.carriers().before() else {
            panic!("one original containing slot")
        };
        let [after] = replacement.carriers().after().unwrap() else {
            panic!("one updated containing slot")
        };
        assert_eq!(before.selection().slot, after.selection().slot);
        assert_eq!(before.selection().path, after.selection().path);
        assert_eq!(before.instance(), after.instance());
        let selected = if before.selection().path.is_root() {
            before.instance().clone()
        } else {
            assert_eq!(
                before.selection().path.fields(),
                &[nocter_model::ValueProjection::Tuple(0)]
            );
            before
                .instance()
                .component(
                    program.instance_program().unit().program(),
                    nocter_model::ValueField::Tuple(0),
                )
                .unwrap()
        };
        assert_eq!(selected, *replacement.root_before());
        enclosing += usize::from(before.instance().data() != replacement.root_before().data());
        replacements += 1;
    }
    assert_eq!(replacements, 2);
    assert_eq!(
        enclosing, 1,
        "the lexical alias retains its larger tuple carrier"
    );
}

#[test]
fn partial_receipt_holes_survive_simplification_without_splitting_the_callable_instance() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-conditional-hole-growth.nct"
    ));
    let program = lower_source(source);
    let (id, selected) = instance(program.instance_program(), "calculate");
    assert_eq!(
        program
            .instance_program()
            .instances()
            .iter()
            .filter(|(_, instance)| instance.key().code() == selected.key().code())
            .count(),
        1
    );
    let mut found = 0;
    for action in program.bodies()[&id].actions().values() {
        let receipts = action
            .alternatives()
            .iter()
            .flat_map(|alternative| {
                alternative
                    .after()
                    .iter()
                    .filter_map(move |step| match step {
                        Step::Resource(resource)
                            if resource.data().destination_roots().is_some() =>
                        {
                            Some((alternative, resource))
                        }
                        _ => None,
                    })
            })
            .collect::<Vec<_>>();
        if receipts.len() != 2 || receipts[0].1.data() != receipts[1].1.data() {
            continue;
        }
        let sizes = receipts
            .iter()
            .map(|(_, resource)| resource.destination_missing().unwrap().len())
            .collect::<std::collections::BTreeSet<_>>();
        if sizes != std::collections::BTreeSet::from([0, 1]) {
            continue;
        }
        for (alternative, _) in receipts {
            assert!(!alternative.flags().tests().is_empty());
        }
        found += 1;
    }
    assert!(
        found >= 2,
        "growth/shrinkage need distinct original guards even when data steps agree"
    );
}
