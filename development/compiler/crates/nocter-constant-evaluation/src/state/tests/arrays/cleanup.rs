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
                range: nocter_model::ArrayRange::Constant { start, end },
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
                range: nocter_model::ArrayRange::Constant { start: 1, end: 2 },
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

fn retained_remainder(length: u64, position: u64) -> Plan {
    let mut plan = script(&[11], vec![integer(4, 42)]);
    let mut slots = plan.slots.to_vec();
    slots.extend([
        Slot {
            data: 1,
            storage: Storage::Owned,
        }, // retained usize snapshot
        Slot {
            data: 0,
            storage: Storage::Owned,
        }, // moved element
    ]);
    plan.slots = slots.into();
    let selected = Place {
        slot: 0,
        fields: vec![usize::try_from(position).unwrap()].into(),
    };
    let mut moved = action(
        Operation::None,
        &[0, 7],
        vec![Effect::Move {
            source: selected.clone(),
            destination: Place::root(8),
        }],
    );
    let holes: Box<[Box<[usize]>]> = vec![selected.fields.clone()].into();
    let condition = BTreeMap::from([(0, holes), (7, Box::new([]) as _), (8, Box::new([]) as _)]);
    let mut changed = action(integer(1, u64::MAX), &[], vec![]);
    changed.alternatives[0].owned = condition.clone();
    let mut done = action(
        integer(4, 42),
        &[],
        vec![
            Effect::DisposeArrayElements {
                source: Place::root(0),
                range: nocter_model::ArrayRange::After(7),
            },
            Effect::DisposeArrayElements {
                source: Place::root(0),
                range: nocter_model::ArrayRange::Before(7),
            },
            Effect::Release(Place::root(0)),
            Effect::Dispose(Place::root(7)),
            Effect::Dispose(Place::root(8)),
            Effect::Construct(4),
        ],
    );
    done.alternatives[0].owned = condition;
    // Source bounds complete before capture and transfer. The old variable changes only after
    // the snapshot is owned. Numeric cleanup consumes that snapshot, never the old variable.
    let actions = vec![
        action(
            Operation::Literal {
                destination: 0,
                value: Value::repeated(Value::integer(11), NonZeroU64::new(length).unwrap()),
            },
            &[],
            vec![Effect::Construct(0)],
        ),
        action(integer(1, position), &[0], vec![]),
        action(index(2), &[0], vec![]),
        action(copy(7, 1), &[0], vec![Effect::Construct(7)]),
        {
            moved.alternatives[0].owned =
                BTreeMap::from([(0, Box::new([]) as _), (7, Box::new([]) as _)]);
            moved
        },
        changed,
        done,
    ];
    plan.points = plan.points.map(|_, point| Point {
        actions: actions.clone().into(),
        terminator: point.terminator.clone(),
    });
    plan
}

#[test]
fn retained_cleanup_uses_saved_numbers_and_skips_empty_sides_with_bounded_storage() {
    let (code, _) = code_ids();
    let mut minima = Vec::new();
    for length in [2, u64::MAX] {
        for position in [0, length - 1] {
            let program = Program::new(
                types(length),
                BTreeMap::from([(code.into(), retained_remainder(length, position))]),
                BTreeMap::new(),
            )
            .unwrap();
            let mut success = None;
            for limit in 1..=96 {
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
            minima.push(success.unwrap());
        }
    }
    assert!(minima.iter().all(|&minimum| minimum == minima[0]));
}

#[test]
fn retiring_the_snapshot_before_its_last_range_use_is_an_integrity_failure() {
    let (code, _) = code_ids();
    let mut plan = retained_remainder(2, 0);
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        let effects = point.actions.last_mut().unwrap().alternatives[0]
            .after
            .as_mut();
        effects.swap(0, 3); // dispose snapshot before the first range; bytes may still exist
        point
    });
    let program = Program::new(
        types(2),
        BTreeMap::from([(code.into(), plan)]),
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
