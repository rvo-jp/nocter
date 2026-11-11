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
        fields: vec![Projection::Index {
            slot: 7,
            bound: length,
        }]
        .into(),
    };
    let mut moved = action(
        Operation::None,
        &[0, 7],
        vec![Effect::Move {
            source: selected.clone(),
            destination: Place::root(8),
        }],
    );
    let holes: Box<[ProjectionPath]> = vec![selected.fields.clone()].into();
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

#[test]
fn another_routes_uninitialized_snapshot_is_never_read_while_selecting_a_guard() {
    let mut plan = retained_remainder(2, 0);
    let mut slots = plan.slots.to_vec();
    slots.push(Slot {
        data: 1,
        storage: Storage::Owned,
    }); // slot 9 belongs to another route
    plan.slots = slots.into();
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        let action = point.actions.last_mut().unwrap();
        let mut other = action.alternatives[0].clone();
        other.owned.remove(&7);
        other.owned.insert(9, Box::new([]));
        other.owned.insert(
            0,
            vec![vec![Projection::Index { slot: 9, bound: 2 }].into()].into(),
        );
        for effect in &mut other.after {
            match effect {
                Effect::DisposeArrayElements { range, .. } => {
                    *range = range
                        .try_map_index::<_, ()>(|slot| Ok(if slot == 7 { 9 } else { slot }))
                        .unwrap();
                }
                Effect::Dispose(place) if place.slot == 7 => place.slot = 9,
                _ => {}
            }
        }
        action.alternatives = vec![other, action.alternatives[0].clone()].into();
        point
    });
    assert_eq!(execute(plan, types(2)), Value::integer(42));
}

#[test]
fn retained_projection_requires_an_owned_usize_and_its_exact_array_bound() {
    let (code, _) = code_ids();
    for projection in [
        Projection::Index { slot: 1, bound: 2 },
        Projection::Index { slot: 7, bound: 3 },
    ] {
        let mut plan = retained_remainder(2, 0);
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            let Effect::Move { source, .. } = &mut point.actions[4].alternatives[0].after[0] else {
                unreachable!()
            };
            source.fields = vec![projection].into();
            point
        });
        assert!(matches!(
            Program::new(
                types(2),
                BTreeMap::from([(code.into(), plan)]),
                BTreeMap::new()
            ),
            Err(Rule::InvalidPlan)
        ));
    }
}

#[test]
fn guard_holes_are_compared_by_numbers_rather_than_snapshot_slot_order() {
    let mut plan = retained_remainder(2, 1);
    let mut slots = plan.slots.to_vec();
    slots.extend([
        Slot {
            data: 1,
            storage: Storage::Owned,
        },
        Slot {
            data: 0,
            storage: Storage::Owned,
        },
    ]);
    plan.slots = slots.into();
    let path = |slot| ProjectionPath::from(vec![Projection::Index { slot, bound: 2 }]);
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        let old = &point.actions;
        let mut first = old[4].clone();
        first.alternatives[0].owned.insert(9, Box::new([]));
        let mut second = action(
            Operation::None,
            &[0, 7, 8, 9],
            vec![Effect::Move {
                source: Place {
                    slot: 0,
                    fields: path(9),
                },
                destination: Place::root(10),
            }],
        );
        second.alternatives[0].owned.insert(0, vec![path(7)].into());
        let mut changed = old[5].clone();
        let mut done = old[6].clone();
        for action in [&mut changed, &mut done] {
            action.alternatives[0]
                .owned
                .insert(0, vec![path(7), path(9)].into());
            action.alternatives[0].owned.insert(9, Box::new([]));
            action.alternatives[0].owned.insert(10, Box::new([]));
        }
        done.alternatives[0].after = vec![
            Effect::Release(Place::root(0)),
            Effect::Dispose(Place::root(7)),
            Effect::Dispose(Place::root(8)),
            Effect::Dispose(Place::root(9)),
            Effect::Dispose(Place::root(10)),
            Effect::Construct(4),
        ]
        .into();
        let mut actions = old[..4].to_vec();
        actions.extend([
            action(integer(1, 0), &[0, 7], vec![]),
            action(index(3), &[0, 7], vec![]),
            action(copy(9, 1), &[0, 7], vec![Effect::Construct(9)]),
            first,
            second,
            changed,
            done,
        ]);
        point.actions = actions.into();
        point
    });
    // Slot 7 holds index 1, slot 9 holds index 0. The plan's canonical symbolic guard order
    // is intentionally the opposite of the actual ownership frontier's numeric order.
    assert_eq!(execute(plan, types(2)), Value::integer(42));
}
