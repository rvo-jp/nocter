//! Representation joins retain exact contracts without selecting a runtime route.
use super::*;

#[test]
fn changed_contexts_cross_a_backedge_only_when_the_entry_context_is_preserved() {
    for stable in [false, true] {
        let (root, types, mut plan) = fixture(Selection::Root, true);
        let entry = plan.entry;
        plan.points = plan.points.map(|id, point| {
            let mut point = point.clone();
            if id != entry {
                point.terminator = Terminator::Jump(if stable { id } else { entry });
            }
            point
        });
        let result = Program::new(
            types,
            BTreeMap::from([(root.into(), plan)]),
            BTreeMap::new(),
        );
        if stable {
            let program = result.unwrap();
            let mut budget = Budget::new(Limits::new(
                CompileTimeEvaluationLimits::new(
                    NonZeroU64::new(64).unwrap(),
                    NonZeroU32::new(4).unwrap(),
                ),
                NonZeroU64::new(256).unwrap(),
            ));
            assert_eq!(
                Executor::new(&program, &mut budget, 0)
                    .evaluate_initializer(root)
                    .unwrap_err()
                    .rule,
                Rule::StepLimit
            );
            assert_eq!(budget.storage_used(), 0);
        } else {
            assert!(matches!(result, Err(Rule::InvalidPlan)));
        }
    }
}

#[test]
fn fresh_receipts_resolve_retained_contexts_on_loop_reentry() {
    let (root, types, mut plan) = fixture(Selection::Root, false);
    let mut slots = plan.slots.to_vec();
    slots.push(Slot {
        data: 3,
        storage: Storage::Owned,
    });
    plan.slots = slots.into();
    let entry = plan.entry;
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Literal {
            destination: 4,
            value: Value::aggregate(vec![Value::unit(), Value::integer(40)]),
        };
        point.actions[0].alternatives[0].after = Box::new([
            Effect::Construct(4),
            Effect::MoveRoot {
                source: Place::root(4),
                destination: Place::root(0),
                data: 3,
            },
        ]);
        let alternative = &mut point.actions.last_mut().unwrap().alternatives[0];
        let mut after = alternative.after.to_vec();
        after.push(Effect::Dispose(Place::root(0)));
        alternative.after = after.into();
        point.terminator = Terminator::Jump(entry);
        point
    });
    let program = Program::new(
        types,
        BTreeMap::from([(root.into(), plan)]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(Limits::new(
        CompileTimeEvaluationLimits::new(
            NonZeroU64::new(256).unwrap(),
            NonZeroU32::new(4).unwrap(),
        ),
        NonZeroU64::new(128).unwrap(),
    ));
    assert_eq!(
        Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(root)
            .unwrap_err()
            .rule,
        Rule::StepLimit
    );
    assert_eq!(budget.storage_used(), 0);
}

fn guarded_plan(marked: bool, reverse: bool) -> (AccessCodeId, Vec<Type>, Plan) {
    let (root, types, mut plan) = fixture(Selection::Root, false);
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        let mut actions = point.actions[..3].to_vec();
        if marked {
            actions.push(action(
                Operation::Literal {
                    destination: 3,
                    value: Value::aggregate(vec![Value::unit(), Value::integer(0)]),
                },
                &[0, 1],
                vec![Effect::Construct(3)],
            ));
        }
        let mut alternatives = vec![
            point.actions[3].alternatives[0].clone(),
            action(
                Operation::None,
                &[0, 1, 3],
                vec![
                    Effect::Dispose(Place::root(1)),
                    Effect::Dispose(Place::root(3)),
                ],
            )
            .alternatives[0]
                .clone(),
        ];
        if reverse {
            alternatives.reverse();
        }
        actions.push(Action {
            operation: Operation::None,
            alternatives: alternatives.into(),
        });
        point.actions = actions.into();
        point
    });
    (root, types, plan)
}
fn receive(owners: &[usize]) -> [Action; 2] {
    let mut supplied = owners.to_vec();
    supplied.push(1);
    [
        action(
            Operation::Literal {
                destination: 1,
                value: Value::aggregate(vec![Value::unit(), Value::integer(42)]),
            },
            owners,
            vec![Effect::Construct(1)],
        ),
        action(
            Operation::None,
            &supplied,
            vec![Effect::ReplaceRoot {
                source: Place::root(1),
                destination: 0,
                data: 4,
            }],
        ),
    ]
}
#[test]
fn differing_guarded_contexts_need_a_fresh_receipt_before_a_typed_use() {
    for marked in [false, true] {
        for reverse in [false, true] {
            for receiving in [false, true] {
                let (root, types, mut plan) = guarded_plan(marked, reverse);
                if receiving {
                    plan.points = plan.points.map(|_, point| {
                        let mut point = point.clone();
                        let mut actions = point.actions.to_vec();
                        actions.extend(receive(&[0]));
                        point.actions = actions.into();
                        point
                    });
                }
                let result = Program::new(
                    types,
                    BTreeMap::from([(root.into(), plan)]),
                    BTreeMap::new(),
                );
                if !receiving {
                    assert!(matches!(result, Err(Rule::InvalidPlan)));
                    continue;
                }
                let program = result.unwrap();
                let mut budget = Budget::new(Limits::default());
                let value = Executor::new(&program, &mut budget, 0)
                    .evaluate_initializer(root)
                    .unwrap();
                assert_eq!(
                    value,
                    Value::aggregate(vec![Value::unit(), Value::integer(42)])
                );
                drop(value);
                assert_eq!(budget.storage_used(), 0);
            }
        }
    }
}

#[test]
fn a_shared_component_can_close_without_choosing_its_containing_representation() {
    for marked in [false, true] {
        for reverse in [false, true] {
            for field in [0, 1, 2] {
                let (root, types, mut plan) = guarded_plan(marked, reverse);
                let mut slots = plan.slots.to_vec();
                slots.push(Slot {
                    data: usize::from(field == 0),
                    storage: Storage::Owned,
                });
                plan.slots = slots.into();
                plan.result = usize::from(field == 0);
                plan.points = plan.points.map(|_, point| {
                    let mut point = point.clone();
                    let mut actions = point.actions.to_vec();
                    actions.push(action(
                        Operation::Copy {
                            destination: Place::root(4),
                            source: Place {
                                slot: 0,
                                fields: vec![field].into(),
                            },
                        },
                        &[0],
                        vec![Effect::Construct(4)],
                    ));
                    actions.extend(receive(&[0, 4]));
                    actions.push(action(
                        Operation::None,
                        &[0, 4],
                        vec![Effect::Dispose(Place::root(0))],
                    ));
                    point.actions = actions.into();
                    point.terminator = Terminator::Return(Place::root(4));
                    point
                });
                let result = Program::new(
                    types,
                    BTreeMap::from([(root.into(), plan)]),
                    BTreeMap::new(),
                );
                if field != 1 {
                    assert!(matches!(result, Err(Rule::InvalidPlan)), "field {field}");
                    continue;
                }
                let program = result.unwrap();
                let mut budget = Budget::new(Limits::default());
                let value = Executor::new(&program, &mut budget, 0)
                    .evaluate_initializer(root)
                    .unwrap();
                assert_eq!(value, Value::integer(if marked { 40 } else { 42 }));
                drop(value);
                assert_eq!(budget.storage_used(), 0);
            }
        }
    }
}

fn indexed_plan(marked: bool, reverse: bool, length: u64) -> (AccessCodeId, Vec<Type>, Plan) {
    let (root, mut types, mut plan) = guarded_plan(marked, reverse);
    types.truncate(3);
    types.extend([
        Type::Array {
            element: 0,
            length: 2,
        },
        Type::Array { element: 0, length },
        Type::Integer(nocter_model::BuiltinType::Usize),
        Type::Record(Box::new([1, 3])),
        Type::Record(Box::new([2, if length == 2 { 3 } else { 4 }])),
    ]);
    let mut slots = plan.slots.to_vec();
    for slot in &mut slots {
        slot.data = match slot.data {
            3 => 6,
            4 => 7,
            data => data,
        };
    }
    slots.extend([
        Slot {
            data: 5,
            storage: Storage::Owned,
        },
        Slot {
            data: 0,
            storage: Storage::Owned,
        },
    ]);
    plan.slots = slots.into();
    plan.result = 0;
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        let mut actions = point.actions.to_vec();
        actions.push(action(
            Operation::Literal {
                destination: 4,
                value: Value::integer(0),
            },
            &[0],
            vec![Effect::Construct(4)],
        ));
        actions.push(action(
            Operation::Copy {
                destination: Place::root(5),
                source: Place {
                    slot: 0,
                    fields: vec![
                        Projection::Field(1),
                        Projection::Index { slot: 4, bound: 2 },
                    ]
                    .into(),
                },
            },
            &[0, 4],
            vec![Effect::Construct(5)],
        ));
        actions.extend(receive(&[0, 4, 5]));
        actions.push(action(
            Operation::None,
            &[0, 4, 5],
            vec![
                Effect::Dispose(Place::root(0)),
                Effect::Dispose(Place::root(4)),
            ],
        ));
        for action in &mut actions {
            for alternative in &mut action.alternatives {
                for effect in &mut alternative.after {
                    if let Effect::ReplaceRoot { data, .. } = effect {
                        *data = 7;
                    }
                }
            }
            if let Operation::Literal { destination, value } = &mut action.operation
                && *destination != 4
            {
                let (count, base) = if *destination == 0 {
                    (2, 40)
                } else {
                    (length, 42)
                };
                *value = Value::aggregate(vec![
                    Value::unit(),
                    Value::aggregate((0..count).map(|_| Value::integer(base)).collect::<Vec<_>>()),
                ]);
            }
        }
        point.actions = actions.into();
        point.terminator = Terminator::Return(Place::root(5));
        point
    });
    (root, types, plan)
}

#[test]
fn common_indexed_components_keep_every_candidate_bound_and_saved_index_contract() {
    for marked in [false, true] {
        for reverse in [false, true] {
            for length in [0, 2, 3] {
                let (root, types, plan) = indexed_plan(marked, reverse, length);
                let result = Program::new(
                    types,
                    BTreeMap::from([(root.into(), plan)]),
                    BTreeMap::new(),
                );
                if length != 2 {
                    assert!(matches!(result, Err(Rule::InvalidPlan)), "length {length}");
                    continue;
                }
                let program = result.unwrap();
                let mut budget = Budget::new(Limits::default());
                let value = Executor::new(&program, &mut budget, 0)
                    .evaluate_initializer(root)
                    .unwrap();
                assert_eq!(value, Value::integer(if marked { 40 } else { 42 }));
                drop(value);
                assert_eq!(budget.storage_used(), 0);
            }
        }
    }
}

#[test]
fn candidate_agreement_never_proves_a_saved_index_type_or_number() {
    for wrong_type in [false, true] {
        let (root, mut types, mut plan) = indexed_plan(false, false, 2);
        if wrong_type {
            types[5] = Type::Bool;
        }
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            for action in &mut point.actions {
                if let Operation::Literal {
                    destination: 4,
                    value,
                } = &mut action.operation
                {
                    *value = if wrong_type {
                        Value::boolean(false)
                    } else {
                        Value::integer(2)
                    };
                }
            }
            point
        });
        let result = Program::new(
            types,
            BTreeMap::from([(root.into(), plan)]),
            BTreeMap::new(),
        );
        if wrong_type {
            assert!(matches!(result, Err(Rule::InvalidPlan)));
            continue;
        }
        let program = result.unwrap();
        let mut budget = Budget::new(Limits::default());
        assert_eq!(
            Executor::new(&program, &mut budget, 0)
                .evaluate_initializer(root)
                .unwrap_err()
                .rule,
            Rule::InvalidPlan
        );
        assert_eq!(budget.storage_used(), 0);
        let (root, types, plan) = indexed_plan(true, true, 2);
        let healthy = Program::new(
            types,
            BTreeMap::from([(root.into(), plan)]),
            BTreeMap::new(),
        )
        .unwrap();
        let value = Executor::new(&healthy, &mut budget, 0)
            .evaluate_initializer(root)
            .unwrap();
        assert_eq!(value, Value::integer(40));
        drop(value);
        assert_eq!(budget.storage_used(), 0);
    }
}
