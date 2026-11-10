//! A schema can name uninstantiated data without granting it any evaluator payload.
use super::*;
#[test]
fn no_value_can_inhabit_an_unavailable_shape_or_its_positive_element() {
    let (code, _) = code_ids();
    for value in [
        Value::unit(),
        Value::boolean(true),
        Value::integer(0),
        Value::aggregate(Vec::new()),
    ] {
        let mut plan = literal();
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            point.actions[0].operation = Operation::Literal {
                destination: 0,
                value: value.clone(),
            };
            point
        });
        assert!(matches!(
            Program::new(
                [Type::Unavailable],
                BTreeMap::from([(code.into(), plan.clone())]),
                BTreeMap::new()
            ),
            Err(Rule::InvalidPlan)
        ));
        let mut slots = plan.slots.to_vec();
        for slot in &mut slots {
            slot.data = 1;
        }
        plan.slots = slots.into();
        plan.result = 1;
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            point.actions[0].operation = Operation::Literal {
                destination: 0,
                value: Value::aggregate(vec![value.clone()]),
            };
            point
        });
        assert!(matches!(
            Program::new(
                [
                    Type::Unavailable,
                    Type::Array {
                        element: 0,
                        length: 1
                    }
                ],
                BTreeMap::from([(code.into(), plan)]),
                BTreeMap::new()
            ),
            Err(Rule::InvalidPlan)
        ));
    }
}
#[test]
fn an_empty_array_can_retain_an_unavailable_element_identity() {
    let (code, _) = code_ids();
    let mut plan = literal();
    let mut slots = plan.slots.to_vec();
    for slot in &mut slots {
        slot.data = 1;
    }
    plan.slots = slots.into();
    plan.result = 1;
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Literal {
            destination: 0,
            value: Value::aggregate(Vec::new()),
        };
        point
    });
    let program = Program::new(
        [
            Type::Unavailable,
            Type::Array {
                element: 0,
                length: 0,
            },
        ],
        BTreeMap::from([(code.into(), plan)]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(CompileTimeEvaluationLimits::default());
    assert_eq!(
        Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(code)
            .unwrap(),
        Value::aggregate(Vec::new())
    );
    assert_eq!(budget.storage_used(), 0);
}

#[test]
fn an_explicit_bounds_failure_needs_no_result_payload_and_releases_execution_storage() {
    let (code, _) = code_ids();
    let mut plan = literal();
    plan.slots = Vec::new().into();
    plan.points = plan.points.map(|_, _| Point {
        actions: vec![Action {
            operation: Operation::BoundsTrap,
            alternatives: vec![Alternative {
                owned: BTreeMap::new(),
                before: Box::new([]),
                after: Box::new([]),
            }]
            .into(),
        }]
        .into(),
        terminator: Terminator::Unreachable,
    });
    let program = Program::new(
        [Type::Unavailable],
        BTreeMap::from([(code.into(), plan)]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(CompileTimeEvaluationLimits::default());
    for _ in 0..2 {
        let error = Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(code)
            .unwrap_err();
        assert_eq!(error.rule, Rule::IndexOutOfBounds);
        assert!(error.site.is_some());
        assert_eq!(budget.storage_used(), 0);
    }
}
