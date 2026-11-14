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
    let mut work = WorkBudget::new(std::num::NonZeroU64::new(1).unwrap());
    let mut projection = Projection::new(&instances, instance, id, at, &mut work).unwrap();
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
    let mut projection = Projection::new(&instances, instance, id, at, &mut work).unwrap();
    assert!(projection.steps(&[prefix]).is_ok());
}
