//! Typed variant validity, deep immutable DAGs and failed-plan isolation.
use super::*;
#[test]
fn optional_values_validate_the_selected_tag_and_payload_type_before_execution() {
    let (id, _) = code_ids();
    for (value, valid) in [
        (Value::variant(false, None), true),
        (Value::variant(true, Some(Value::integer(42))), true),
        (Value::variant(true, None), false),
        (Value::variant(false, Some(Value::integer(42))), false),
        (Value::variant(true, Some(Value::boolean(true))), false),
    ] {
        let mut plan = literal();
        for slot in std::sync::Arc::make_mut(&mut plan.slots) {
            slot.data = 1;
        }
        plan.result = 1;
        plan.points = plan.points.map(|_, original| {
            let mut point = original.clone();
            point.actions[0].operation = Operation::Literal {
                destination: 0,
                value: value.clone(),
            };
            point
        });
        assert_eq!(
            Program::new(
                [
                    Type::Integer(nocter_model::BuiltinType::I32),
                    Type::Optional(0)
                ],
                BTreeMap::from([(id.into(), plan)]),
                BTreeMap::new()
            )
            .is_ok(),
            valid
        );
    }
}
#[test]
fn deep_variant_values_freeze_compare_restore_and_drop_iteratively() {
    let mut value = Value::integer(42);
    for _ in 0..20_000 {
        value = Value::variant(true, Some(value));
    }
    let frozen = value.freeze();
    assert_eq!(frozen.nodes().len(), 20_001);
    let restored = Value::from_frozen(&frozen).unwrap();
    assert!(value == restored);
}

#[test]
#[allow(clippy::too_many_lines)]
fn taking_inputs_are_protected_at_their_frozen_owner_handoff_after_wrapper_materialization() {
    use nocter_model::ValueInputOperation as Op;
    let (root, helper) = code_ids();
    let mut consumer = literal();
    consumer.initializer = false;
    consumer.slots = vec![
        Slot {
            data: 0,
            storage: Storage::Owned,
        },
        Slot {
            data: 0,
            storage: Storage::Owned,
        },
        Slot {
            data: 1,
            storage: Storage::Owned,
        },
    ]
    .into();
    consumer.inputs = vec![(2, Op::Take)].into();
    consumer.points = consumer.points.map(|_, original| {
        let mut point = original.clone();
        point.actions[0].operation = Operation::Copy {
            source: Place {
                slot: 2,
                fields: vec![0].into(),
            },
            destination: Place::root(0),
        };
        point.actions[0].alternatives[0]
            .owned
            .insert(2, Box::new([]));
        let mut after = point.actions[0].alternatives[0].after.to_vec();
        after.push(Effect::Dispose(Place::root(2)));
        point.actions[0].alternatives[0].after = after.into();
        point
    });
    let mut initializer = literal();
    initializer.slots = vec![
        Slot {
            data: 0,
            storage: Storage::Owned,
        },
        Slot {
            data: 1,
            storage: Storage::Owned,
        },
        Slot {
            data: 0,
            storage: Storage::Owned,
        },
    ]
    .into();
    initializer.points = initializer.points.map(|_, original| {
        let mut point = original.clone();
        point.actions[0].alternatives[0].after = vec![Effect::Construct(0)].into();
        let mut actions = point.actions.to_vec();
        actions.push(Action {
            operation: Operation::Call {
                destination: 2,
                target: helper.into(),
                inputs: vec![Input::Take(1)].into(),
            },
            alternatives: vec![Alternative {
                owned: BTreeMap::from([(0, Box::new([]) as Box<[Missing]>)]),
                before: vec![
                    Effect::Assemble {
                        destination: 1,
                        fields: vec![(0, Place::root(0))].into(),
                    },
                    Effect::Invoke {
                        source: Place::root(1),
                        inputs: vec![(0, Place::root(1))].into(),
                    },
                ]
                .into(),
                after: vec![Effect::Construct(2)].into(),
            }]
            .into(),
        });
        point.actions = actions.into();
        point.terminator = Terminator::Return(Place::root(2));
        point
    });
    let definitions = BTreeMap::from([(root.into(), initializer), (helper.into(), consumer)]);
    let program = Program::new(
        [
            Type::Integer(nocter_model::BuiltinType::I32),
            Type::Optional(0),
        ],
        definitions.clone(),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(Limits::default());
    let value = Executor::new(&program, &mut budget, 0)
        .evaluate_initializer(root)
        .unwrap();
    assert_eq!(value, Value::integer(42));
    drop(value);
    assert_eq!(budget.storage_used(), 0);
    // Fail at successive import/frame/wrapper/protection allocations, then reuse the meter.
    // Failed execution must release the partially assembled wrapper and protected payload too.
    let mut failures = 0;
    let mut successes = 0;
    for capacity in 1..128 {
        let mut budget = Budget::new(Limits::new(
            crate::CompileTimeEvaluationLimits::default(),
            std::num::NonZeroU64::new(capacity).unwrap(),
        ));
        for _ in 0..2 {
            budget.restart_work();
            match Executor::new(&program, &mut budget, 0).evaluate_initializer(root) {
                Ok(value) => {
                    assert_eq!(value, Value::integer(42));
                    successes += 1;
                }
                Err(error) => {
                    assert_eq!(error.rule, Rule::StorageLimit);
                    failures += 1;
                }
            }
            assert_eq!(budget.storage_used(), 0, "capacity {capacity}");
        }
    }
    assert!(failures > 0 && successes > 0);
    for protected in [
        vec![],
        vec![(0, Place::root(1)), (0, Place::root(1))],
        vec![(1, Place::root(1))],
        vec![(0, Place::root(0))],
    ] {
        let mut altered = definitions.clone();
        let entry = altered.get_mut(&root.into()).unwrap();
        entry.points = entry.points.map(|_, original| {
            let mut point = original.clone();
            point.actions[1].alternatives[0].before[1] = Effect::Invoke {
                source: Place::root(1),
                inputs: protected.clone().into(),
            };
            point
        });
        assert!(
            Program::new(
                [
                    Type::Integer(nocter_model::BuiltinType::I32),
                    Type::Optional(0)
                ],
                altered,
                BTreeMap::new()
            )
            .is_err()
        );
    }
}

#[test]
fn primary_conditions_observe_only_the_tag_and_reject_incompatible_operand_roles() {
    let (root, _) = code_ids();
    for primary in [false, true] {
        let mut plan = literal();
        plan.slots = vec![
            Slot {
                data: 1,
                storage: Storage::Owned,
            },
            Slot {
                data: 2,
                storage: Storage::Owned,
            },
        ]
        .into();
        plan.result = 2;
        plan.points = plan.points.map(|_, original| {
            let mut point = original.clone();
            point.actions[0].operation = Operation::Literal {
                destination: 0,
                value: Value::variant(primary, primary.then(|| Value::integer(42))),
            };
            point.actions[0].alternatives[0].after = vec![Effect::Construct(0)].into();
            let mut actions = point.actions.to_vec();
            actions.push(Action {
                operation: Operation::PrimaryCondition {
                    source: Place::root(0),
                    destination: 1,
                },
                alternatives: vec![Alternative {
                    owned: BTreeMap::from([(0, Box::new([]) as Box<[Missing]>)]),
                    before: Box::new([]),
                    after: vec![Effect::Construct(1), Effect::Dispose(Place::root(0))].into(),
                }]
                .into(),
            });
            point.actions = actions.into();
            point
        });
        let types = [
            Type::Integer(nocter_model::BuiltinType::I32),
            Type::Optional(0),
            Type::Bool,
        ];
        let program = Program::new(
            types.clone(),
            BTreeMap::from([(root.into(), plan.clone())]),
            BTreeMap::new(),
        )
        .unwrap();
        let mut budget = Budget::new(Limits::default());
        assert_eq!(
            Executor::new(&program, &mut budget, 0)
                .evaluate_initializer(root)
                .unwrap(),
            Value::boolean(primary)
        );
        assert_eq!(budget.storage_used(), 0);
        for (source, destination) in [(Place::root(1), 1), (Place::root(0), 0)] {
            let mut invalid = plan.clone();
            invalid.points = invalid.points.map(|_, original| {
                let mut point = original.clone();
                point.actions[1].operation = Operation::PrimaryCondition {
                    source: source.clone(),
                    destination,
                };
                point
            });
            assert!(
                Program::new(
                    types.clone(),
                    BTreeMap::from([(root.into(), invalid)]),
                    BTreeMap::new()
                )
                .is_err()
            );
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn guarded_optional_aliases_bind_only_matching_variants_and_release_partial_work_on_failure() {
    let (root, _) = code_ids();
    for (primary, expected) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut plan = literal();
        plan.slots = vec![
            Slot {
                data: 1,
                storage: Storage::Owned,
            },
            Slot {
                data: 1,
                storage: Storage::Alias,
            },
            Slot {
                data: 0,
                storage: Storage::Owned,
            },
        ]
        .into();
        plan.points = plan.points.map(|_, original| {
            let mut point = original.clone();
            point.actions[0].operation = Operation::Literal {
                destination: 0,
                value: Value::variant(primary, primary.then(|| Value::integer(42))),
            };
            point.actions[0].alternatives[0].after = vec![Effect::Construct(0)].into();
            let mut actions = point.actions.to_vec();
            let owned = BTreeMap::from([(0, Box::new([]) as Box<[Missing]>)]);
            actions.push(Action {
                operation: Operation::RequireVariant {
                    primary: expected,
                    source: Place::root(0),
                    view: Some(1),
                },
                alternatives: vec![Alternative {
                    owned: owned.clone(),
                    before: Box::new([]),
                    after: Box::new([]),
                }]
                .into(),
            });
            actions.push(Action {
                operation: if expected {
                    Operation::Copy {
                        destination: Place::root(2),
                        source: Place {
                            slot: 1,
                            fields: vec![0].into(),
                        },
                    }
                } else {
                    Operation::Literal {
                        destination: 2,
                        value: Value::integer(42),
                    }
                },
                alternatives: vec![Alternative {
                    owned,
                    before: Box::new([]),
                    after: vec![Effect::Construct(2), Effect::Dispose(Place::root(0))].into(),
                }]
                .into(),
            });
            point.actions = actions.into();
            point.terminator = Terminator::Return(Place::root(2));
            point
        });
        let types = [
            Type::Integer(nocter_model::BuiltinType::I32),
            Type::Optional(0),
        ];
        let program = Program::new(
            types.clone(),
            BTreeMap::from([(root.into(), plan.clone())]),
            BTreeMap::new(),
        )
        .unwrap();
        let mut reached_guard = false;
        let mut exhausted = false;
        for capacity in 1..128 {
            let mut budget = Budget::new(Limits::new(
                crate::CompileTimeEvaluationLimits::default(),
                std::num::NonZeroU64::new(capacity).unwrap(),
            ));
            for _ in 0..2 {
                budget.restart_work();
                match Executor::new(&program, &mut budget, 0).evaluate_initializer(root) {
                    Ok(value) => {
                        assert_eq!(primary, expected);
                        assert_eq!(value, Value::integer(42));
                        reached_guard = true;
                    }
                    Err(error) if error.rule == Rule::StorageLimit => exhausted = true,
                    Err(error) => {
                        assert_ne!(primary, expected);
                        assert_eq!(error.rule, Rule::InactiveVariant);
                        reached_guard = true;
                    }
                }
                assert_eq!(budget.storage_used(), 0);
            }
        }
        assert!(reached_guard && exhausted);
        for view in [0, 2, 3] {
            let mut invalid = plan.clone();
            invalid.points = invalid.points.map(|_, original| {
                let mut point = original.clone();
                point.actions[1].operation = Operation::RequireVariant {
                    primary: expected,
                    source: Place::root(0),
                    view: Some(view),
                };
                point
            });
            assert!(
                Program::new(
                    types.clone(),
                    BTreeMap::from([(root.into(), invalid)]),
                    BTreeMap::new()
                )
                .is_err()
            );
        }
    }
}
