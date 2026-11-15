//! Partial roots retain their original container context without creating a value or owner.
use super::*;

fn representation(program: &Program, key: &DataInstanceKey) -> DataInstanceKey {
    key.function_representation(program)
        .unwrap()
        .unwrap_or_else(|| key.clone())
}

fn fixture() -> (Unit, CodeInstanceKey) {
    let source = ValueSourceFixture::new(
        "func first(): i32 { 42 }\nfunc work(take action: func(): i32): i32 { let pair = (action^, 1)\nlet selected = pair.0^\nselected() + pair.1 - 1 }",
    );
    let unit = SourceUnit::prepare(source.syntax()).unwrap().into_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve_code(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let work = code("work");
    let parameter = program.codes().get(work).unwrap().parameters()[0];
    let frame = program
        .bind_code(work.into(), [(parameter.into(), code("first").into())])
        .unwrap();
    (unit, frame)
}

fn partial_transition(
    unit: &Unit,
    frame: &CodeInstanceKey,
) -> (Location, Vec<ResourceStep>, Vec<ResourceStep>) {
    let body = unit.code_body(frame.code()).unwrap();
    for (point, block) in body.graph().points().iter() {
        for action in 0..block.actions.len() {
            let at = Location { point, action };
            for transition in body.access().resources().at(at) {
                if transition.steps().iter().any(|step| matches!(step, ResourceStep::Move { source, .. } if !source.fields().is_root())) {
                    return (at, transition.before_steps().to_vec(), transition.after_steps().to_vec());
                }
            }
        }
    }
    panic!("the original source must transfer a selected tuple component");
}

#[test]
fn partial_roots_preserve_operations_and_match_the_selected_callable_representation() {
    let (unit, frame) = fixture();
    let (at, before, after) = partial_transition(&unit, &frame);
    let mut work = budget(10_000);
    let mut projection = unit.project_resource_data(&frame, at, &mut work).unwrap();
    let projected = projection
        .steps_with_roots(&before)
        .unwrap()
        .into_iter()
        .chain(projection.after_steps_with_roots(&after).unwrap())
        .collect::<Vec<_>>();
    let program = unit.program();
    let mut covered = 0;
    for (original, selected) in before.iter().chain(&after).zip(&projected) {
        assert_eq!(
            original.try_map_payloads::<(), ()>(|_| Ok(())).unwrap(),
            selected
                .operation()
                .try_map_payloads::<(), ()>(|_| Ok(()))
                .unwrap()
        );
        let ResourceStep::Move {
            source, payload, ..
        } = selected.operation()
        else {
            continue;
        };
        if source.fields().is_root() {
            continue;
        }
        let root = selected.root(source).unwrap();
        let leaf = root
            .project_path(program, &metadata_path(source, 0_u64))
            .unwrap();
        // A formal leaf retains its semantic type; its selected callable has the named key.
        assert_ne!(leaf.data(), payload.instance().data());
        assert_eq!(
            representation(program, &leaf),
            representation(program, payload.instance())
        );
        assert!(selected.root(&Holder::Return.place()).is_none());
        covered += 1;
    }
    assert_eq!(covered, 1);
}

#[test]
fn containing_root_work_failure_restores_only_the_failed_phase_and_allows_healthy_retry() {
    let (unit, frame) = fixture();
    let (at, before, after) = partial_transition(&unit, &frame);
    let mut exhausted = budget(1);
    let mut projection = unit
        .project_resource_data(&frame, at, &mut exhausted)
        .unwrap();
    let mut checkpoint = projection.values.clone();
    let result = projection.steps_with_roots(&before).and_then(|_| {
        checkpoint = projection.values.clone();
        projection.after_steps_with_roots(&after)
    });
    assert!(matches!(
        result,
        Err(ResourceDataFailure::Instantiation(
            InstantiationFailure::WorkLimit
        ))
    ));
    assert_eq!(projection.values, checkpoint);
    let mut healthy = budget(10_000);
    let mut retry = unit
        .project_resource_data(&frame, at, &mut healthy)
        .unwrap();
    let selected = retry
        .steps_with_roots(&before)
        .unwrap()
        .into_iter()
        .chain(retry.after_steps_with_roots(&after).unwrap())
        .collect::<Vec<_>>();
    assert!(selected.iter().any(|step| step.roots().next().is_some()));
    assert_eq!(selected.len(), before.len() + after.len());
}

#[test]
fn partial_destination_contracts_retain_the_original_parent_and_selected_receipt() {
    let fixture = ValueSourceFixture::new(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-replacement.nct"
    )));
    let unit = SourceUnit::prepare(fixture.syntax()).unwrap().into_unit();
    let program = unit.program();
    let code = program
        .resolve_code(program.symbols().get("calculate").unwrap())
        .unwrap();
    let frame = program.bind_code(code.into(), []).unwrap();
    let body = unit.code_body(frame.code()).unwrap();
    let mut covered = 0;
    let mut alias_covered = 0;
    for (point, block) in body.graph().points().iter() {
        for action in 0..block.actions.len() {
            let at = Location { point, action };
            for transition in body.access().resources().at(at) {
                let mut work = budget(10_000);
                let mut projection = unit.project_resource_data(&frame, at, &mut work).unwrap();
                let selected = projection
                    .steps_with_roots(transition.before_steps())
                    .unwrap()
                    .into_iter()
                    .chain(
                        projection
                            .after_steps_with_roots(transition.after_steps())
                            .unwrap(),
                    );
                for step in selected {
                    if let ResourceStep::ReplaceReferent {
                        destination,
                        payload,
                        ..
                    } = step.operation()
                    {
                        assert_alias_carriers(&mut projection, program, destination, payload);
                        alias_covered += 1;
                    }
                    let Some(roots) = step.destination_roots() else {
                        continue;
                    };
                    let ResourceStep::Move {
                        destination,
                        payload,
                        ..
                    } = step.operation()
                    else {
                        panic!("a containing contract cannot invent an operation")
                    };
                    assert_eq!(destination, roots.destination());
                    assert_eq!(roots.before().data(), roots.after().data());
                    assert_ne!(roots.before(), roots.after());
                    assert_eq!(
                        roots
                            .before()
                            .component(program, ValueField::Tuple(1))
                            .unwrap(),
                        roots
                            .after()
                            .component(program, ValueField::Tuple(1))
                            .unwrap()
                    );
                    assert_eq!(
                        representation(
                            program,
                            &roots
                                .after()
                                .component(program, ValueField::Tuple(0))
                                .unwrap()
                        ),
                        representation(program, payload.instance())
                    );
                    covered += 1;
                }
            }
        }
    }
    assert_eq!(covered, 1);
    assert_eq!(alias_covered, 3);
}

fn assert_alias_carriers(
    projection: &mut ResourceDataProjection<'_, '_>,
    program: &Program,
    destination: &Place,
    payload: &Payload,
) {
    let (_, carriers) = projection.replacement_carriers(destination).unwrap();
    let [before] = carriers.before() else {
        panic!("one original owner");
    };
    let [after] = carriers.after().unwrap() else {
        panic!("one completed owner");
    };
    assert_eq!(before.selection().slot, after.selection().slot);
    assert_eq!(before.selection().path, after.selection().path);
    assert_ne!(before.instance(), after.instance());
    assert_eq!(
        before
            .instance()
            .component(program, ValueField::Tuple(1))
            .unwrap(),
        after
            .instance()
            .component(program, ValueField::Tuple(1))
            .unwrap()
    );
    assert_eq!(
        representation(
            program,
            &after
                .instance()
                .component(program, ValueField::Tuple(0))
                .unwrap()
        ),
        representation(program, payload.instance())
    );
}
