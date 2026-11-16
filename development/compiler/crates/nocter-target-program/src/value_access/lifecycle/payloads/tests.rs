//! Actual value transport must override a frame's default payload for the same semantic type.
use super::*;
use crate::value_access::InstanceProgram;
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

#[test]
fn an_unmatched_replacement_is_rejected_before_projecting_its_prefix() {
    use nocter_checking::value_access::flow::resources::{Holder, payload};
    let instances = instances();
    let code = instances
        .unit()
        .program()
        .resolve_code(instances.unit().program().symbols().get("work").unwrap())
        .unwrap();
    let (id, instance) = instances
        .instances()
        .iter()
        .find(|(_, instance)| instance.key().code() == code.into())
        .unwrap();
    let body = instances.body(id).unwrap();
    let local = body.typed().body().inputs().explicit()[0].local;
    let data = body.typed().body().locals().get(local).unwrap().data;
    let payload = payload(instances.unit().program(), data).unwrap();
    let prefix = ResourceStep::Move {
        source: Holder::Local(local).place(),
        destination: Holder::Return.place(),
        payload,
    };
    let invalid = ResourceStep::ReplaceReferent {
        source: Holder::Return.place(),
        destination: Holder::Local(local).place(),
        retired: payload,
        payload,
    };
    let at = Location {
        point: body.graph().entry(),
        action: 0,
    };
    let condition = body.access().resources().at(at).next().unwrap().condition();
    let mut work = WorkBudget::new(std::num::NonZeroU64::new(1).unwrap());
    let mut projection = Projection::new(instance, id, at, &condition, &[], &mut work).unwrap();
    assert_eq!(
        projection.steps(&[prefix.clone(), invalid]),
        Err(Failure::InvalidInstanceProgram)
    );
    drop(projection);
    assert!(
        work.step().is_ok(),
        "preflight failure must not start payload projection"
    );
    let mut work = WorkBudget::new(std::num::NonZeroU64::new(1_000).unwrap());
    let mut projection = Projection::new(instance, id, at, &condition, &[], &mut work).unwrap();
    assert!(projection.steps(&[prefix]).is_ok());
}

#[test]
fn destination_annotation_work_is_bounded_and_failed_freezing_can_retry() {
    let fixture = ValueSourceFixture::new(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-conditional-hole-growth.nct"
    )));
    let unit = Arc::new(SourceUnit::prepare(fixture.syntax()).unwrap().into_unit());
    let root = unit
        .program()
        .resolve_code(unit.program().symbols().get("run").unwrap())
        .unwrap();
    let instances = InstanceProgram::prepare(unit, root).unwrap();
    let selected = instances.instances().iter().find_map(|(id, _)| {
        let body = instances.body(id).unwrap();
        for (point, block) in body.graph().points().iter() {
            for action in 0..block.actions.len() {
                for transition in body.access().resources().at(Location { point, action }) {
                    let condition = transition.condition();
                    if transition.steps().iter().any(|step| matches!(step, ResourceStep::Move { destination, .. } if !destination.fields().is_root() && condition.owned().any(|(holder, missing)| holder == destination.holder() && !missing.is_empty()))) {
                        return Some((transition.steps().to_vec(), condition));
                    }
                }
            }
        }
        None
    }).unwrap();
    let budget = |limit| WorkBudget::new(std::num::NonZeroU64::new(limit).unwrap());
    let mut exhausted = budget(1);
    assert_eq!(
        destinations(&selected.0, &selected.1, &mut exhausted),
        Err(())
    );
    assert!(exhausted.step().is_err());
    let mut healthy = budget(1_000);
    let complete = destinations(&selected.0, &selected.1, &mut healthy).unwrap();
    assert!(complete.values().any(|missing| !missing.is_empty()));
    assert_eq!(
        complete,
        destinations(&selected.0, &selected.1, &mut budget(1_000)).unwrap()
    );
}
