//! Sparse cleanup state must stay finite and reject overlapping retirement.
use super::*;
fn intervals(length: u64, ranges: &[(u64, u64)]) -> Plan {
    let mut plan = script(&[11, 22], vec![integer(4, 42)]);
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Literal {
            destination: 0,
            value: Value::repeated(Value::integer(11), NonZeroU64::new(length).unwrap()),
        };
        let mut effects: Vec<_> = ranges
            .iter()
            .map(|&(start, end)| Effect::DisposeArrayElements {
                source: Place::root(0),
                start,
                end,
            })
            .collect();
        effects.extend([Effect::Release(Place::root(0)), Effect::Construct(4)]);
        point.actions[1].alternatives[0].after = effects.into();
        point
    });
    plan
}
#[test]
fn maximum_interval_cleanup_has_the_same_live_storage_requirement_as_a_small_interval() {
    let (code, _) = code_ids();
    let mut first_success = Vec::new();
    for length in [2, u64::MAX] {
        let program = Program::new(
            types(length),
            BTreeMap::from([(code.into(), intervals(length, &[(0, length)]))]),
            BTreeMap::new(),
        )
        .unwrap();
        let mut success = None;
        for limit in 1..=64 {
            let mut budget = Budget::new(Limits::new(
                CompileTimeEvaluationLimits::default(),
                NonZeroU64::new(limit).unwrap(),
            ));
            match Executor::new(&program, &mut budget, 0).evaluate_initializer(code) {
                Ok(value) => {
                    assert_eq!(value, Value::integer(42));
                    success.get_or_insert(limit);
                }
                Err(error) => assert_eq!(error.rule, Rule::StorageLimit),
            }
            assert_eq!(budget.storage_used(), 0);
        }
        first_success.push(success.unwrap());
    }
    assert_eq!(first_success[0], first_success[1]);
}
#[test]
fn disjoint_intervals_retire_once_and_overlap_fails_without_leaking_state() {
    assert_eq!(
        execute(intervals(3, &[(2, 3), (0, 2)]), types(3)),
        Value::integer(42)
    );
    let (code, _) = code_ids();
    for ranges in [&[(0, 2), (1, 3)][..], &[(0, 3), (0, 3)][..]] {
        let program = Program::new(
            types(3),
            BTreeMap::from([(code.into(), intervals(3, ranges))]),
            BTreeMap::new(),
        )
        .unwrap();
        let mut budget = Budget::new(CompileTimeEvaluationLimits::default());
        for _ in 0..2 {
            assert_eq!(
                Executor::new(&program, &mut budget, 0)
                    .evaluate_initializer(code)
                    .unwrap_err()
                    .rule,
                Rule::InvalidPlan
            );
            assert_eq!(budget.storage_used(), 0);
        }
    }
}
#[test]
fn interval_bounds_are_validated_before_execution() {
    let (code, _) = code_ids();
    for range in [(0, 0), (2, 1), (0, 4)] {
        assert!(matches!(
            Program::new(
                types(3),
                BTreeMap::from([(code.into(), intervals(3, &[range]))]),
                BTreeMap::new()
            ),
            Err(Rule::InvalidPlan)
        ));
    }
}
#[test]
fn nested_partial_elements_and_intact_intervals_share_one_retirement_frontier() {
    let mut plan = script(&[11, 22], vec![integer(4, 42)]);
    let mut slots = plan.slots.to_vec();
    slots[0].data = 3;
    slots[6].data = 3;
    plan.slots = slots.into();
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Literal {
            destination: 0,
            value: Value::aggregate(vec![array(&[1, 2]), array(&[3, 4])]),
        };
        let place = |fields: &[usize]| Place {
            slot: 0,
            fields: fields.into(),
        };
        point.actions[1].alternatives[0].after = vec![
            Effect::Dispose(place(&[0, 0])),
            Effect::DisposeArrayElements {
                source: place(&[]),
                start: 1,
                end: 2,
            },
            Effect::Dispose(place(&[0, 1])),
            Effect::Release(place(&[0])),
            Effect::Release(place(&[])),
            Effect::Construct(4),
        ]
        .into();
        point
    });
    assert_eq!(
        execute(
            plan,
            vec![
                Type::I32,
                Type::Usize,
                Type::Record(vec![0, 0].into()),
                Type::Array {
                    element: 2,
                    length: 2
                }
            ]
        ),
        Value::integer(42)
    );
}
