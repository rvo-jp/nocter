//! Actual source admission and concrete lifecycle phases, without pretending to execute native code.
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
        assert!(alternative.before().iter().any(|step| matches!(step, Step::Resource(ResourceStep::Invoke { payload, .. }) if payload.disposal() == Disposal::OwnedCapture)));
        assert!(matches!(alternative.after(), [
            Step::DisposeEnvironment { argument: 0, part: EnvironmentPart::Capture, payload },
            Step::Resource(ResourceStep::Receive { .. })
        ] if payload.disposal() == Disposal::OwnedError));
    }
    for (&id, body) in program.bodies() {
        for (&at, action) in body.actions() {
            if matches!(operation(&program, id, at), Operation::CommitCall(_)) {
                for alternative in action.alternatives() {
                    assert!(
                        !alternative
                            .before()
                            .iter()
                            .any(|s| matches!(s, Step::Resource(ResourceStep::Receive { .. })))
                    );
                    assert!(
                        !alternative
                            .after()
                            .iter()
                            .any(|s| matches!(s, Step::Resource(ResourceStep::Invoke { .. })))
                    );
                }
            }
        }
    }
    for step in steps(&program) {
        if let Step::Resource(step) = step {
            step.try_map_payloads(|payload| {
                assert_ne!(payload.disposal(), Disposal::FunctionEnvironment);
                Ok::<_, ()>(payload)
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
    let body = &program.bodies()[&selected.target()];
    let mut disposed = Vec::new();
    let mut released = Vec::new();
    for alternative in body
        .actions()
        .values()
        .flat_map(lowering::Action::alternatives)
    {
        for step in alternative.before().iter().chain(alternative.after()) {
            match step {
                Step::Resource(ResourceStep::Dispose { source, .. })
                    if source.holder()
                        == nocter_checking::value_access::flow::resources::Holder::Local(
                            input.local,
                        ) =>
                {
                    disposed.push(source);
                }
                Step::Resource(ResourceStep::ReleaseContainer { source, .. }) => {
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
    let conditional = program.bodies()[&id]
        .actions()
        .values()
        .find(|action| {
            action.alternatives().len() == 2
                && action.alternatives().iter().any(|alternative| {
                    alternative
                        .after()
                        .iter()
                        .any(|s| matches!(s, Step::Resource(ResourceStep::ReleaseContainer { .. })))
                })
        })
        .unwrap();
    let alternatives = conditional.alternatives();
    assert_ne!(alternatives[0].condition(), alternatives[1].condition());
    let mut missing_counts = alternatives
        .iter()
        .map(|alternative| {
            alternative
                .condition()
                .owned()
                .map(|(_, missing)| missing.len())
                .sum::<usize>()
        })
        .collect::<Vec<_>>();
    missing_counts.sort_unstable();
    assert_eq!(missing_counts, [0, 1]);
    assert!(alternatives.iter().any(|a| matches!(a.after(), [Step::Resource(ResourceStep::Dispose { payload, .. })] if payload.disposal() == Disposal::Record)));
    assert!(alternatives.iter().any(|a| matches!(a.after(), [Step::Resource(ResourceStep::Dispose { payload, .. }), Step::Resource(ResourceStep::ReleaseContainer { .. })] if payload.disposal() == Disposal::OwnedError)));
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
        prepared.alternatives()[0].before(),
        [Step::Resource(ResourceStep::Move { .. })]
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
            alternative.after(),
            [Step::Resource(ResourceStep::Dispose { .. })]
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
    assert!(!steps(&program).any(|s| matches!(
        s,
        Step::DisposeEnvironment { .. }
            | Step::Resource(ResourceStep::Invoke { .. } | ResourceStep::Receive { .. })
    )));
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
            Step::DisposeEnvironment { argument: 0, part: EnvironmentPart::Capture, payload },
            Step::Resource(ResourceStep::Receive { .. })
        ] if payload.disposal() == Disposal::OwnedError));
    }
    assert!(
        steps(&program)
            .any(|step| matches!(step, Step::Resource(ResourceStep::ReplaceInput { .. })))
    );
}
