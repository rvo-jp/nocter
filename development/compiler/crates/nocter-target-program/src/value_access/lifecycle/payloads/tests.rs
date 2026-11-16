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
    let transition = body.access().resources().at(at).next().unwrap();
    let mut work = WorkBudget::new(std::num::NonZeroU64::new(1).unwrap());
    let mut projection = Projection::new(instance, id, at, transition, &mut work).unwrap();
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
    let mut projection = Projection::new(instance, id, at, transition, &mut work).unwrap();
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

#[test]
fn scoped_carrier_exclusions_require_original_ownership_at_every_boundary() {
    for (source, expected) in [
        (
            "func run(): i32 { var pair = (1, 2)\nedit selected = pair.0\nselected = 3\nselected + pair.1 }",
            Some(0),
        ),
        (
            "struct Item { value: i32 }\nfunc run(): i32 { var pair = (1, Item { value: 2 })\nlet saved = pair.1^\nedit selected = pair.0\nselected = 3\nselected + saved.value }",
            Some(1),
        ),
        (
            "func run(): i32 { var pair = (1, 2)\nedit selected = pair.0\nselected = pair.1^\nselected }",
            Some(1),
        ),
        (
            "func change(edit pair: (i32, i32)): void { edit selected = pair.0\nselected = 3 }\nfunc run(): i32 { var pair = (1, 2)\nchange(pair)\npair.0 }",
            None,
        ),
    ] {
        let fixture = ValueSourceFixture::new(source);
        let unit = Arc::new(SourceUnit::prepare(fixture.syntax()).unwrap().into_unit());
        let root = unit
            .program()
            .resolve(unit.program().symbols().get("run").unwrap())
            .unwrap();
        let instances = Arc::new(InstanceProgram::prepare(unit, root).unwrap());
        let program = super::super::Program::prepare(instances).unwrap();
        let mut found = 0;
        for body in program.bodies().values() {
            for action in body.actions().values() {
                for alternative in action.alternatives() {
                    for step in alternative.before().iter().chain(alternative.after()) {
                        let Step::ScopedReplacement(replacement) = step else {
                            continue;
                        };
                        let [carrier] = replacement.carriers().before() else {
                            panic!("one original carrier");
                        };
                        let nocter_checking::value_access::ValueSourceSlot::Local(local) =
                            carrier.selection().slot
                        else {
                            panic!("a local source slot");
                        };
                        assert_eq!(
                            replacement
                                .carrier_missing(Holder::Local(local))
                                .map(<[_]>::len),
                            expected,
                            "{source}"
                        );
                        found += 1;
                    }
                }
            }
        }
        assert_eq!(found, 1, "{source}");
    }
}

#[test]
fn stable_sparse_freezing_meters_original_exclusions_and_recovers_without_partial_lifecycle() {
    let fixture = ValueSourceFixture::new(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-scoped-growth.nct"
    )));
    let unit = Arc::new(SourceUnit::prepare(fixture.syntax()).unwrap().into_unit());
    let root = unit
        .program()
        .resolve(unit.program().symbols().get("run").unwrap())
        .unwrap();
    let instances = Arc::new(InstanceProgram::prepare(unit, root).unwrap());
    let mut found = false;
    for (id, _) in instances.instances().iter() {
        let body = instances.body(id).unwrap();
        for (point, block) in body.graph().points().iter() {
            for action in 0..block.actions.len() {
                for transition in body.access().resources().at(Location { point, action }) {
                    if !transition
                        .steps()
                        .iter()
                        .any(|step| matches!(step, ResourceStep::ReplaceReferent { .. }))
                        || !transition
                            .operation_condition()
                            .owned()
                            .any(|(_, missing)| !missing.is_empty())
                    {
                        continue;
                    }
                    let mut work = WorkBudget::new(std::num::NonZeroU64::new(1).unwrap());
                    work.step().unwrap();
                    assert_eq!(stable_carriers(transition, &mut work), Err(()));
                    let mut work = WorkBudget::new(std::num::NonZeroU64::new(1_000).unwrap());
                    assert!(
                        stable_carriers(transition, &mut work)
                            .unwrap()
                            .values()
                            .any(|missing| !missing.is_empty())
                    );
                    found = true;
                }
            }
        }
    }
    assert!(found);
    assert!(super::super::Program::prepare(instances).is_ok());
}
