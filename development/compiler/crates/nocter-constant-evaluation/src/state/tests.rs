//! Closed-plan boundaries and failed transaction recovery, independent of source projection.
use super::*;
use crate::CompileTimeEvaluationLimits;
use nocter_model::ArenaBuilder;
use std::num::{NonZeroU32, NonZeroU64};
fn literal() -> Plan {
    let mut points = ArenaBuilder::new();
    let entry = points.insert(Point {
        actions: vec![Action {
            operation: Operation::Literal {
                destination: 0,
                value: Value::I32(42),
            },
            alternatives: vec![Alternative {
                owned: BTreeMap::new(),
                before: Box::new([]),
                after: vec![
                    Effect::Construct(0),
                    Effect::Move {
                        source: Place::root(0),
                        destination: Place::root(1),
                    },
                ]
                .into(),
            }]
            .into(),
        }]
        .into(),
        terminator: Terminator::Return(Place::root(1)),
    });
    Plan {
        slots: vec![
            Slot {
                data: 0,
                storage: Storage::Owned
            };
            2
        ]
        .into(),
        inputs: Box::new([]),
        result: 0,
        initializer: true,
        dependencies: Box::new([]),
        entry,
        points: points.finish(),
    }
}
fn code_ids() -> (AccessCallableId, AccessCallableId) {
    let mut ids = ArenaBuilder::new();
    (ids.insert(()), ids.insert(()))
}
#[test]
fn equal_ownership_guards_cannot_choose_conflicting_resource_phases() {
    let (id, _) = code_ids();
    let mut invalid = literal();
    invalid.points = invalid.points.map(|_, point| {
        let mut point = point.clone();
        let mut alternative = point.actions[0].alternatives[0].clone();
        alternative.after = vec![Effect::Construct(1)].into();
        point.actions[0].alternatives =
            vec![point.actions[0].alternatives[0].clone(), alternative].into();
        point
    });
    assert!(matches!(
        Program::new([Type::I32], BTreeMap::from([(id, invalid)])),
        Err(Rule::InvalidPlan)
    ));
}
#[test]
fn invalid_value_shapes_call_edges_and_recursive_schema_edges_reject_before_execution() {
    let (id, _) = code_ids();
    let mut wrong_value = literal();
    wrong_value.points = wrong_value.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Literal {
            destination: 0,
            value: Value::Bool(true),
        };
        point
    });
    assert!(Program::new([Type::I32], BTreeMap::from([(id, wrong_value)])).is_err());
    assert!(Program::new([Type::Record(vec![0].into())], BTreeMap::new()).is_err());
    let mut missing_call = literal();
    missing_call.points = missing_call.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Call {
            destination: 0,
            target: id,
            inputs: Box::new([]),
        };
        point
    });
    // An initializer is not an ordinary call target even when arity and data happen to agree.
    assert!(Program::new([Type::I32], BTreeMap::from([(id, missing_call)])).is_err());
}
#[test]
fn failed_execution_clears_frames_and_dependency_activity_before_the_next_request() {
    let (good, bad) = code_ids();
    let mut points = ArenaBuilder::new();
    let entry = points.next_id();
    points.insert(Point {
        actions: Box::new([]),
        terminator: Terminator::Jump(entry),
    });
    let loop_plan = Plan {
        slots: Vec::new().into(),
        inputs: Box::new([]),
        result: 0,
        initializer: true,
        dependencies: Box::new([]),
        entry,
        points: points.finish(),
    };
    let program = Program::new(
        [Type::I32],
        BTreeMap::from([(good, literal()), (bad, loop_plan)]),
    )
    .unwrap();
    let mut executor = Executor::new(
        &program,
        CompileTimeEvaluationLimits::new(NonZeroU64::new(20).unwrap(), NonZeroU32::new(8).unwrap()),
    );
    for _ in 0..2 {
        assert_eq!(
            executor.evaluate_initializer(bad).unwrap_err().rule,
            Rule::StepLimit
        );
        assert_eq!(
            executor.evaluate_initializer(good).unwrap().as_ref(),
            &Value::I32(42)
        );
    }
}

#[test]
fn sparse_guards_have_one_sorted_prefix_free_form_before_phase_matching() {
    let (id, _) = code_ids();
    for holes in [
        vec![vec![0], vec![0]],
        vec![vec![1], vec![0]],
        vec![Vec::new()],
    ] {
        let mut plan = literal();
        let mut slots = plan.slots.to_vec();
        slots.push(Slot {
            data: 1,
            storage: Storage::Owned,
        });
        plan.slots = slots.into();
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            point.actions[0].alternatives[0].owned.insert(
                2,
                holes.iter().cloned().map(Vec::into_boxed_slice).collect(),
            );
            point
        });
        assert!(matches!(
            Program::new(
                [Type::I32, Type::Record(vec![0, 0].into())],
                BTreeMap::from([(id, plan)])
            ),
            Err(Rule::InvalidPlan)
        ));
    }
}
