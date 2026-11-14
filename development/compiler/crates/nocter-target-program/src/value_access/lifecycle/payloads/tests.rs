//! Actual value transport must override a frame's default payload for the same semantic type.
use super::*;
use nocter_checking::value_access::accepted::SourceUnit;
use nocter_test_support::ValueSourceFixture;
use std::sync::Arc;

fn instances() -> Arc<InstanceProgram> {
    let fixture = ValueSourceFixture::new(
        "func first(): i32 { 41 }\nfunc second(): i32 { 42 }\nfunc work(take action: func(): i32): func(): i32 { action^ }\nfunc run(): i32 { let left = work(first)\nlet right = work(second)\nleft() + right() }",
    );
    let unit = Arc::new(SourceUnit::prepare(fixture.syntax()).unwrap().into_unit());
    let root = unit
        .program()
        .resolve(unit.program().symbols().get("run").unwrap())
        .unwrap();
    Arc::new(InstanceProgram::prepare(unit, root).unwrap())
}

#[test]
fn ordered_replacement_uses_the_supplied_value_instead_of_the_frames_type_default() {
    assert_replacement(false);
}

#[test]
fn scoped_replacement_keeps_the_retired_payload_separate_from_the_supplied_payload() {
    assert_replacement(true);
}

#[test]
fn contextual_retirement_without_an_original_write_target_cannot_use_the_input_snapshot() {
    let instances = instances();
    let work = instances
        .unit()
        .program()
        .resolve(instances.unit().program().symbols().get("work").unwrap())
        .unwrap();
    let (id, instance) = instances
        .instances()
        .iter()
        .find(|(_, instance)| instance.key().code() == work.into())
        .unwrap();
    let checked = instances.body(id).unwrap();
    let input = checked.typed().body().inputs().explicit()[0].local;
    let data = checked.typed().body().locals().get(input).unwrap().data;
    assert!(instance.data()[&data].payload().is_some());
    let at = Location {
        point: checked.graph().entry(),
        action: 0,
    };
    let mut budget = WorkBudget::new(std::num::NonZeroU64::new(10_000).unwrap());
    let mut projection = Projection::new(&instances, instance, id, at, &mut budget);
    assert_eq!(
        projection.retired(&Holder::Local(input).place(), data),
        Err(INVALID)
    );
    assert!(projection.values.is_empty());
}

fn assert_replacement(scoped: bool) {
    let instances = instances();
    let program = instances.unit().program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let work = code("work");
    let first = program.concrete_witness(code("first").into()).unwrap();
    let second = program.concrete_witness(code("second").into()).unwrap();
    let (id, instance) = instances
        .instances()
        .iter()
        .find(|(_, instance)| {
            instance.key().code() == work.into()
                && instance
                    .key()
                    .witnesses()
                    .values()
                    .any(|value| value == &first)
        })
        .unwrap();
    let body = instances.body(id).unwrap().typed().body();
    let local = body.inputs().explicit()[0].local;
    let data = body.locals().get(local).unwrap().data;
    let old = program.instantiate_function(data, &first).unwrap();
    let supplied = program.instantiate_function(data, &second).unwrap();
    assert_eq!(old.data(), supplied.data());
    assert_ne!(old, supplied);
    let mut budget = WorkBudget::new(std::num::NonZeroU64::new(10_000).unwrap());
    let at = Location {
        point: instances.body(id).unwrap().graph().entry(),
        action: 0,
    };
    let mut projection = Projection::new(&instances, instance, id, at, &mut budget);
    projection
        .values
        .insert(Holder::Local(local), Binding::Value(old.clone()));
    projection
        .values
        .insert(Holder::Return, Binding::Value(supplied.clone()));
    let contract = nocter_checking::value_access::flow::resources::payload(program, data).unwrap();
    let destination = Holder::Local(local).place();
    let source = Holder::Return.place();
    let mut steps = if scoped {
        vec![ResourceStep::ReplaceReferent {
            destination: destination.clone(),
            source,
            retired: contract,
            payload: contract,
        }]
    } else {
        vec![
            ResourceStep::Dispose {
                source: destination.clone(),
                payload: contract,
            },
            ResourceStep::Move {
                source,
                destination: destination.clone(),
                payload: contract,
            },
        ]
    };
    steps.push(ResourceStep::Dispose {
        source: destination,
        payload: contract,
    });
    let projected = projection.steps(&steps).unwrap();
    let (previous, moved, final_value) = replacement_payloads(&projected);
    assert_eq!(
        previous.instance(),
        &old.function_representation(program).unwrap().unwrap()
    );
    assert_eq!(
        moved.instance(),
        &supplied.function_representation(program).unwrap().unwrap()
    );
    assert_eq!(final_value, moved);
    assert_ne!(previous, final_value);
    assert_eq!(instance.data()[&data].payload().unwrap(), *previous);
}

fn replacement_payloads(steps: &[Step]) -> (&Payload, &Payload, &Payload) {
    match steps {
        [
            Step::Resource(ResourceStep::Dispose {
                payload: previous, ..
            }),
            Step::Resource(ResourceStep::Move { payload: moved, .. }),
            Step::Resource(ResourceStep::Dispose {
                payload: final_value,
                ..
            }),
        ]
        | [
            Step::Resource(ResourceStep::ReplaceReferent {
                retired: previous,
                payload: moved,
                ..
            }),
            Step::Resource(ResourceStep::Dispose {
                payload: final_value,
                ..
            }),
        ] => (previous, moved, final_value),
        _ => panic!("original replacement followed by final cleanup"),
    }
}

#[test]
fn lifecycle_work_failure_publishes_no_partial_program_and_the_same_instances_can_recover() {
    let instances = instances();
    let mut work = WorkBudget::new(std::num::NonZeroU64::new(1).unwrap());
    assert!(matches!(
        super::super::Program::prepare_with_budget(Arc::clone(&instances), &mut work),
        Err(Failure::Payload {
            failure: nocter_checking::value_access::InstantiationFailure::WorkLimit,
            ..
        })
    ));
    let complete = super::super::Program::prepare(instances).unwrap();
    assert_eq!(
        complete.bodies().len(),
        complete.instance_program().instances().len()
    );
}
