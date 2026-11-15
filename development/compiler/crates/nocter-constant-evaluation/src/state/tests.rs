//! Closed-plan boundaries and failed transaction recovery, independent of source projection.
use super::*;
use crate::CompileTimeEvaluationLimits;
use nocter_model::ArenaBuilder;
use std::num::{NonZeroU32, NonZeroU64};
mod arrays;
mod captured_invocation;
mod captured_values;
mod contexts;
mod empty_domains;
mod expression_roots;
mod lexical_codes;
mod named_functions;
mod unavailable;
pub(super) fn literal() -> Plan {
    let mut points = ArenaBuilder::new();
    let entry = points.insert(Point {
        actions: vec![Action {
            operation: Operation::Literal {
                destination: 0,
                value: Value::integer(42),
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
        statics: Box::new([]),
        environment_backing: None,
        inputs: Box::new([]),
        result: 0,
        initializer: true,
        entry,
        points: points.finish(),
    }
}
pub(super) fn code_ids() -> (AccessCodeId, AccessCodeId) {
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
        Program::new(
            [Type::Integer(nocter_model::BuiltinType::I32)],
            BTreeMap::from([(id.into(), invalid)]),
            BTreeMap::new()
        ),
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
            value: Value::boolean(true),
        };
        point
    });
    assert!(
        Program::new(
            [Type::Integer(nocter_model::BuiltinType::I32)],
            BTreeMap::from([(id.into(), wrong_value)]),
            BTreeMap::new()
        )
        .is_err()
    );
    assert!(
        Program::new(
            [Type::Record(vec![0].into())],
            BTreeMap::new(),
            BTreeMap::new()
        )
        .is_err()
    );
    let mut missing_call = literal();
    missing_call.points = missing_call.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Call {
            destination: 0,
            target: id.into(),
            inputs: Box::new([]),
        };
        point
    });
    // An initializer is not an ordinary call target even when arity and data happen to agree.
    assert!(
        Program::new(
            [Type::Integer(nocter_model::BuiltinType::I32)],
            BTreeMap::from([(id.into(), missing_call)]),
            BTreeMap::new()
        )
        .is_err()
    );
}
#[test]
fn exhausted_work_is_not_replenished_by_another_closed_request() {
    let (good, bad) = code_ids();
    let mut points = ArenaBuilder::new();
    let entry = points.next_id();
    points.insert(Point {
        actions: Box::new([]),
        terminator: Terminator::Jump(entry),
    });
    let loop_plan = Plan {
        slots: Vec::new().into(),
        statics: Box::new([]),
        environment_backing: None,
        inputs: Box::new([]),
        result: 0,
        initializer: true,
        entry,
        points: points.finish(),
    };
    let program = Program::new(
        [Type::Integer(nocter_model::BuiltinType::I32)],
        BTreeMap::from([(good.into(), literal()), (bad.into(), loop_plan)]),
        BTreeMap::new(),
    )
    .unwrap();
    let limits =
        CompileTimeEvaluationLimits::new(NonZeroU64::new(20).unwrap(), NonZeroU32::new(8).unwrap());
    let mut budget = Budget::new(limits);
    let mut executor = Executor::new(&program, &mut budget, 0);
    assert_eq!(
        executor.evaluate_initializer(bad).unwrap_err().rule,
        Rule::StepLimit
    );
    assert_eq!(
        executor.evaluate_initializer(good).unwrap_err().rule,
        Rule::StepLimit
    );
    // Opening a new executor over the same dependency budget must not replenish it either.
    let mut executor = Executor::new(&program, &mut budget, 0);
    assert_eq!(
        executor.evaluate_initializer(good).unwrap_err().rule,
        Rule::StepLimit
    );
    let mut fresh_budget = Budget::new(limits);
    let mut executor = Executor::new(&program, &mut fresh_budget, 0);
    assert_eq!(
        executor.evaluate_initializer(good).unwrap(),
        Value::integer(42)
    );
}

#[test]
fn arithmetic_failure_releases_its_frame_before_reusing_the_executor() {
    let (good, bad) = code_ids();
    let mut failure = literal();
    failure.points = failure.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Literal {
            destination: 0,
            value: Value::integer(i32::MIN),
        };
        let mut actions = point.actions.into_vec();
        let after = vec![
            Effect::Dispose(Place::root(1)),
            Effect::Construct(0),
            Effect::Move {
                source: Place::root(0),
                destination: Place::root(1),
            },
        ];
        actions.push(Action {
            operation: Operation::Unary {
                destination: 0,
                operation: crate::CompileTimeUnaryOperation::Negate,
                operand: Place::root(1),
            },
            alternatives: vec![Alternative {
                owned: BTreeMap::from([(1, Box::new([]) as Box<[Missing]>)]),
                before: Box::new([]),
                after: after.into(),
            }]
            .into(),
        });
        point.actions = actions.into();
        point
    });
    let program = Program::new(
        [Type::Integer(nocter_model::BuiltinType::I32)],
        BTreeMap::from([(good.into(), literal()), (bad.into(), failure)]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(CompileTimeEvaluationLimits::new(
        NonZeroU64::new(100).unwrap(),
        NonZeroU32::new(1).unwrap(),
    ));
    let mut executor = Executor::new(&program, &mut budget, 0);
    for _ in 0..2 {
        assert_eq!(
            executor.evaluate_initializer(bad).unwrap_err().rule,
            Rule::ArithmeticFailure
        );
        assert_eq!(
            executor.evaluate_initializer(good).unwrap(),
            Value::integer(42)
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
            point.actions[0].alternatives[0]
                .owned
                .insert(2, holes.iter().cloned().map(Missing::from).collect());
            point
        });
        assert!(matches!(
            Program::new(
                [
                    Type::Integer(nocter_model::BuiltinType::I32),
                    Type::Record(vec![0, 0].into())
                ],
                BTreeMap::from([(id.into(), plan)]),
                BTreeMap::new()
            ),
            Err(Rule::InvalidPlan)
        ));
    }
}

#[test]
fn unread_scoped_inputs_still_require_initialized_caller_data() {
    let (root, function) = code_ids();
    let mut target = literal();
    target.initializer = false;
    let mut slots = target.slots.to_vec();
    slots.push(Slot {
        data: 0,
        storage: Storage::Alias,
    });
    target.slots = slots.into();
    target.inputs = vec![(2, nocter_model::ValueInputOperation::Read)].into();
    let mut source = literal();
    source.points = source.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Call {
            destination: 0,
            target: function.into(),
            inputs: vec![Input::Read(Place::root(0))].into(),
        };
        point
    });
    let program = Program::new(
        [Type::Integer(nocter_model::BuiltinType::I32)],
        BTreeMap::from([(root.into(), source), (function.into(), target)]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(CompileTimeEvaluationLimits::default());
    assert_eq!(
        Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(root)
            .unwrap_err()
            .rule,
        Rule::InvalidPlan
    );
    assert_eq!(budget.storage_used(), 0);
}

#[test]
fn widened_integer_payloads_still_require_their_exact_slot_domain() {
    let (id, _) = code_ids();
    for (ty, value, valid) in [
        (
            Type::Integer(nocter_model::BuiltinType::I32),
            i128::from(i32::MAX),
            true,
        ),
        (
            Type::Integer(nocter_model::BuiltinType::I32),
            i128::from(u64::MAX),
            false,
        ),
        (
            Type::Integer(nocter_model::BuiltinType::Usize),
            i128::from(u64::MAX),
            true,
        ),
        (Type::Integer(nocter_model::BuiltinType::Usize), -1, false),
        (
            Type::Integer(nocter_model::BuiltinType::Usize),
            i128::from(u64::MAX) + 1,
            false,
        ),
    ] {
        let mut plan = literal();
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            point.actions[0].operation = Operation::Literal {
                destination: 0,
                value: Value::integer(value),
            };
            point
        });
        assert_eq!(
            Program::new([ty], BTreeMap::from([(id.into(), plan)]), BTreeMap::new()).is_ok(),
            valid
        );
    }
}

#[test]
fn integer_descriptor_rejects_noninteger_builtins_even_without_values_or_plans() {
    for &builtin in nocter_model::BuiltinType::ALL {
        let expected = nocter_model::BuiltinNumericType::for_builtin(builtin)
            .and_then(nocter_model::BuiltinNumericType::integer_bounds)
            .is_some();
        assert_eq!(
            Program::new([Type::Integer(builtin)], BTreeMap::new(), BTreeMap::new()).is_ok(),
            expected,
            "{builtin:?}"
        );
    }
}

mod variant_values;

mod deferred_plans;
