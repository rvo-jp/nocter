//! Execution coverage and backing, independently of the source numerical induction proof.
use super::*;

fn prefix(length: u64, endpoint: usize) -> Missing {
    Missing::ArrayPrefix {
        base: ProjectionPath::default(),
        endpoint,
        bound: length,
    }
}
fn condition(length: u64, extra: &[usize]) -> BTreeMap<usize, Box<[Missing]>> {
    let mut owned = BTreeMap::from([(0, vec![prefix(length, 7)].into()), (7, Box::new([]) as _)]);
    owned.extend(
        extra
            .iter()
            .map(|&slot| (slot, Box::new([]) as Box<[Missing]>)),
    );
    owned
}
fn step(operation: Operation, length: u64, extra: &[usize], after: Vec<Effect>) -> Action {
    let mut result = action(operation, &[], after);
    result.alternatives[0].owned = condition(length, extra);
    result
}
fn extraction(length: u64, first: u64, second: Option<u64>) -> Plan {
    let mut plan = cleanup::retained_remainder(length, first);
    let mut slots = plan.slots.to_vec();
    slots.push(Slot {
        data: 1,
        storage: Storage::Owned,
    });
    plan.slots = slots.into();
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        let mut actions = point.actions[..5].to_vec();
        let mut moved = actions[4].alternatives[0].after.to_vec();
        moved[0] = Effect::MoveArrayElement {
            array: Place::root(0),
            selected: Place::root(2),
            destination: 8,
        };
        moved.push(Effect::ConsolidateArrayPrefix {
            source: Place::root(0),
            endpoint: 7,
        });
        actions[4].alternatives[0].after = moved.into();
        if let Some(position) = second {
            actions.extend([
                step(
                    integer(1, position),
                    length,
                    &[8],
                    vec![Effect::Dispose(Place::root(8))],
                ),
                step(index(3), length, &[], vec![]),
                step(copy(9, 1), length, &[], vec![Effect::Construct(9)]),
                step(
                    integer(1, u64::MAX),
                    length,
                    &[9],
                    vec![
                        Effect::MoveArrayElement {
                            array: Place::root(0),
                            selected: Place::root(3),
                            destination: 8,
                        },
                        Effect::Dispose(Place::root(7)),
                        Effect::Move {
                            source: Place::root(9),
                            destination: Place::root(7),
                        },
                        Effect::ConsolidateArrayPrefix {
                            source: Place::root(0),
                            endpoint: 7,
                        },
                    ],
                ),
            ]);
        }
        // Cleanup follows the transferred endpoint, including an exit before any counter update.
        actions.extend([
            step(integer(1, u64::MAX), length, &[8], vec![]),
            step(
                integer(4, 42),
                length,
                &[8],
                vec![
                    Effect::DisposeArrayElements {
                        source: Place::root(0),
                        range: nocter_model::ArrayRange::After(7),
                    },
                    Effect::Release(Place::root(0)),
                    Effect::Dispose(Place::root(7)),
                    Effect::Dispose(Place::root(8)),
                    Effect::Construct(4),
                ],
            ),
        ]);
        point.actions = actions.into();
        point
    });
    plan
}
#[test]
fn a_moved_prefix_advances_under_its_own_endpoint_and_cleanup_survives_counter_mutation() {
    for length in [2, u64::MAX] {
        for second in [None, Some(1)] {
            assert_eq!(
                execute(extraction(length, 0, second), types(length)),
                Value::integer(42)
            );
        }
    }
}
#[test]
fn prefix_working_storage_is_independent_of_array_cardinality_and_all_failures_recover() {
    let (code, _) = code_ids();
    for second in [None, Some(1)] {
        let mut minima = Vec::new();
        for length in [2, u64::MAX] {
            let program = Program::new(
                types(length),
                BTreeMap::from([(code.into(), extraction(length, 0, second))]),
                BTreeMap::new(),
            )
            .unwrap();
            let mut first = None;
            for limit in 1..=128 {
                let mut budget = Budget::new(Limits::new(
                    CompileTimeEvaluationLimits::default(),
                    NonZeroU64::new(limit).unwrap(),
                ));
                match Executor::new(&program, &mut budget, 0).evaluate_initializer(code) {
                    Ok(value) => {
                        assert_eq!(value, Value::integer(42));
                        first.get_or_insert(limit);
                    }
                    Err(error) => assert_eq!(error.rule, Rule::StorageLimit),
                }
                assert_eq!(budget.storage_used(), 0);
            }
            minima.push(first.unwrap());
        }
        assert_eq!(minima[0], minima[1]);
    }
}
#[test]
fn a_gap_or_missing_first_prefix_is_not_silently_absorbed_and_bounds_still_fail_first() {
    let (code, _) = code_ids();
    for (first, second, rule) in [
        (1, None, Rule::InvalidPlan),
        (0, Some(2), Rule::InvalidPlan),
        (0, Some(3), Rule::IndexOutOfBounds),
    ] {
        let program = Program::new(
            types(3),
            BTreeMap::from([(code.into(), extraction(3, first, second))]),
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
                rule
            );
            assert_eq!(budget.storage_used(), 0);
        }
    }
}

#[test]
fn the_last_element_consolidates_a_maximum_prefix_without_successor_overflow() {
    for length in [2, u64::MAX] {
        let mut plan = extraction(length, length - 1, None);
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            // Seed the same physical missing coverage using intact interval retirement. This
            // qualifies execution algebra at MAX, not a source induction or N runtime moves.
            point.actions[4].alternatives[0].before = vec![Effect::DisposeArrayElements {
                source: Place::root(0),
                range: nocter_model::ArrayRange::Constant {
                    start: 0,
                    end: length - 1,
                },
            }]
            .into();
            point
        });
        assert_eq!(execute(plan, types(length)), Value::integer(42));
    }
}

#[test]
fn prefix_guards_validate_their_array_bound_and_complete_owned_usize_backing() {
    let (code, _) = code_ids();
    for fault in 0..4 {
        let mut plan = extraction(2, 0, None);
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            let guarded = &mut point.actions[5].alternatives[0];
            match fault {
                0 => {
                    guarded.owned.insert(
                        0,
                        vec![Missing::ArrayPrefix {
                            base: ProjectionPath::default(),
                            endpoint: 7,
                            bound: 3,
                        }]
                        .into(),
                    );
                }
                1 => {
                    guarded.owned.remove(&7);
                }
                2 => {
                    guarded.owned.insert(0, vec![prefix(2, 8)].into());
                }
                _ => {
                    let effects = &mut point.actions[4].alternatives[0].after;
                    *effects.last_mut().unwrap() = Effect::ConsolidateArrayPrefix {
                        source: Place::root(0),
                        endpoint: 1,
                    };
                }
            }
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
fn an_inactive_prefix_route_never_reads_its_uninitialized_endpoint() {
    for first in [false, true] {
        let mut plan = extraction(2, 0, None);
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            let done = point.actions.last_mut().unwrap();
            let mut inactive = done.alternatives[0].clone();
            inactive.owned.remove(&7);
            inactive.owned.insert(9, Box::new([]));
            inactive.owned.insert(0, vec![prefix(2, 9)].into());
            inactive.after = inactive
                .after
                .iter()
                .map(|effect| match effect {
                    Effect::DisposeArrayElements { source, .. } => Effect::DisposeArrayElements {
                        source: source.clone(),
                        range: nocter_model::ArrayRange::After(9),
                    },
                    Effect::Dispose(place) if place.slot == 7 => Effect::Dispose(Place::root(9)),
                    _ => effect.clone(),
                })
                .collect();
            done.alternatives = if first {
                vec![inactive, done.alternatives[0].clone()]
            } else {
                vec![done.alternatives[0].clone(), inactive]
            }
            .into();
            point
        });
        assert_eq!(execute(plan, types(2)), Value::integer(42));
    }
}

#[test]
fn ending_the_endpoint_owner_before_consolidation_cannot_reuse_its_readable_bytes() {
    let (code, _) = code_ids();
    let mut plan = extraction(2, 0, None);
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        let effects = &mut point.actions[4].alternatives[0].after;
        let mut ended = effects[..effects.len() - 1].to_vec();
        ended.extend([
            Effect::Dispose(Place::root(7)),
            Effect::ConsolidateArrayPrefix {
                source: Place::root(0),
                endpoint: 7,
            },
        ]);
        *effects = ended.into();
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
        let failure = Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(code)
            .unwrap_err();
        assert_eq!(failure.rule, Rule::InvalidPlan);
        assert_eq!(failure.site.unwrap().action, 4);
        assert_eq!(budget.storage_used(), 0);
    }
}

#[test]
fn a_prefix_under_a_record_field_uses_the_same_owner_and_projection_geometry() {
    let nested = |place: &Place| {
        if place.slot != 0 {
            return place.clone();
        }
        Place {
            slot: 0,
            fields: std::iter::once(Projection::Field(0))
                .chain(place.fields.iter().copied())
                .collect(),
        }
    };
    for length in [2, u64::MAX] {
        let mut plan = extraction(length, 0, Some(1));
        let mut slots = plan.slots.to_vec();
        slots[0].data = 3;
        plan.slots = slots.into();
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            for action in &mut point.actions {
                action.operation = match &action.operation {
                    Operation::Literal {
                        destination: 0,
                        value,
                    } => Operation::Literal {
                        destination: 0,
                        value: Value::aggregate(vec![value.clone()]),
                    },
                    Operation::Index {
                        destination,
                        base,
                        index,
                    } => Operation::Index {
                        destination: *destination,
                        base: nested(base),
                        index: index.clone(),
                    },
                    _ => action.operation.clone(),
                };
                for alternative in &mut action.alternatives {
                    if let Some(holes) = alternative.owned.get_mut(&0) {
                        for hole in holes.iter_mut() {
                            match hole {
                                Missing::Value(path) | Missing::ArrayPrefix { base: path, .. } => {
                                    *path = nested(&Place {
                                        slot: 0,
                                        fields: path.clone(),
                                    })
                                    .fields;
                                }
                            }
                        }
                    }
                    alternative.after = alternative
                        .after
                        .iter()
                        .flat_map(|effect| match effect {
                            Effect::MoveArrayElement {
                                array,
                                selected,
                                destination,
                            } => vec![Effect::MoveArrayElement {
                                array: nested(array),
                                selected: selected.clone(),
                                destination: *destination,
                            }],
                            Effect::Move {
                                source,
                                destination,
                            } => vec![Effect::Move {
                                source: nested(source),
                                destination: nested(destination),
                            }],
                            Effect::DisposeArrayElements { source, range } => {
                                vec![Effect::DisposeArrayElements {
                                    source: nested(source),
                                    range: *range,
                                }]
                            }
                            Effect::ConsolidateArrayPrefix { source, endpoint } => {
                                vec![Effect::ConsolidateArrayPrefix {
                                    source: nested(source),
                                    endpoint: *endpoint,
                                }]
                            }
                            Effect::Release(source) => vec![
                                Effect::Release(nested(source)),
                                Effect::Release(source.clone()),
                            ],
                            _ => vec![effect.clone()],
                        })
                        .collect();
                }
            }
            point
        });
        let mut data = types(length);
        data.push(Type::Record(vec![2].into()));
        assert_eq!(execute(plan, data), Value::integer(42));
    }
}

#[test]
fn a_saved_element_cannot_surrender_through_a_different_live_array_owner() {
    let (code, _) = code_ids();
    let mut plan = extraction(2, 0, None);
    let mut slots = plan.slots.to_vec();
    slots[6].storage = Storage::Owned;
    plan.slots = slots.into();
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        let old = &point.actions;
        let mut actions = vec![
            old[0].clone(),
            action(
                Operation::Literal {
                    destination: 6,
                    value: array(&[31, 32]),
                },
                &[0],
                vec![Effect::Construct(6)],
            ),
        ];
        for (index, item) in old.iter().enumerate().skip(1) {
            let mut item = item.clone();
            for alternative in &mut item.alternatives {
                alternative.owned.insert(6, Box::new([]));
            }
            if index == 4 {
                item.alternatives[0].after[0] = Effect::MoveArrayElement {
                    array: Place::root(6),
                    selected: Place::root(2),
                    destination: 8,
                };
            }
            actions.push(item);
        }
        point.actions = actions.into();
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
        let failure = Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(code)
            .unwrap_err();
        assert_eq!(failure.rule, Rule::InvalidPlan);
        assert_eq!(failure.site.unwrap().action, 5);
        assert_eq!(budget.storage_used(), 0);
    }
}

#[test]
fn a_frozen_address_and_an_owned_containing_array_are_both_required() {
    let (code, _) = code_ids();
    for (array, selected) in [(0, 8), (6, 2)] {
        let mut plan = extraction(2, 0, None);
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            point.actions[4].alternatives[0].after[0] = Effect::MoveArrayElement {
                array: Place::root(array),
                selected: Place::root(selected),
                destination: 8,
            };
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
