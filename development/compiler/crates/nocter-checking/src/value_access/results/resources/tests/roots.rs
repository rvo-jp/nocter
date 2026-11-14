//! Partial roots retain their original container context without creating a value or owner.
use super::*;

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
    let representation = |key: &DataInstanceKey| {
        key.function_representation(program)
            .unwrap()
            .unwrap_or_else(|| key.clone())
    };
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
        assert_eq!(representation(&leaf), representation(payload.instance()));
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
