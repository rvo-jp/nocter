//! Context changes preserve owners and actual references, never merely relabel function code.
use super::*;

#[derive(Clone, Copy)]
enum Selection {
    Root,
    Preserved,
    Stale,
}
fn action(operation: Operation, owned: &[usize], effects: Vec<Effect>) -> Action {
    Action {
        operation,
        alternatives: Box::new([Alternative {
            owned: owned
                .iter()
                .map(|&slot| (slot, Box::new([]) as Box<[Missing]>))
                .collect(),
            before: Box::new([]),
            after: effects.into(),
        }]),
    }
}
fn setup() -> Vec<Action> {
    vec![
        action(
            Operation::Literal {
                destination: 0,
                value: Value::aggregate(vec![Value::unit(), Value::integer(40)]),
            },
            &[],
            vec![Effect::Construct(0)],
        ),
        action(
            Operation::Alias {
                destination: 2,
                source: Place::root(0),
            },
            &[0],
            vec![],
        ),
        action(
            Operation::Literal {
                destination: 1,
                value: Value::aggregate(vec![Value::unit(), Value::integer(42)]),
            },
            &[0],
            vec![Effect::Construct(1)],
        ),
        action(
            Operation::None,
            &[0, 1],
            vec![Effect::ReplaceRoot {
                source: Place::root(1),
                destination: 0,
                data: 4,
            }],
        ),
    ]
}
fn fixture(selection: Selection, split: bool) -> (AccessCodeId, Vec<Type>, Plan) {
    let (root, other) = code_ids();
    let types = vec![
        Type::Integer(nocter_model::BuiltinType::I32),
        Type::NamedFunction(root),
        Type::NamedFunction(other),
        Type::Record(Box::new([1, 0])),
        Type::Record(Box::new([2, 0])),
    ];
    let (result, tail, output) = match selection {
        Selection::Root => (4, vec![], Place::root(0)),
        Selection::Preserved | Selection::Stale => {
            let field = usize::from(matches!(selection, Selection::Preserved));
            let data = usize::from(field != 1);
            (
                data,
                vec![action(
                    Operation::Copy {
                        destination: Place::root(3),
                        source: Place {
                            slot: 2,
                            fields: vec![field].into(),
                        },
                    },
                    &[0],
                    vec![Effect::Construct(3), Effect::Dispose(Place::root(0))],
                )],
                Place::root(3),
            )
        }
    };
    let mut points = ArenaBuilder::new();
    let mut initial = setup();
    let entry = if split {
        let end = points.insert(Point {
            actions: tail.into(),
            terminator: Terminator::Return(output),
        });
        points.insert(Point {
            actions: initial.into(),
            terminator: Terminator::Jump(end),
        })
    } else {
        initial.extend(tail);
        points.insert(Point {
            actions: initial.into(),
            terminator: Terminator::Return(output),
        })
    };
    let plan = Plan {
        slots: [
            (3, Storage::Owned),
            (4, Storage::Owned),
            (3, Storage::Alias),
            (result, Storage::Owned),
        ]
        .map(|(data, storage)| Slot { data, storage })
        .into(),
        inputs: Box::new([]),
        statics: Box::new([]),
        environment_backing: None,
        result,
        initializer: true,
        entry,
        points: points.finish(),
    };
    (root, types, plan)
}
fn program(selection: Selection, split: bool) -> (AccessCodeId, Program) {
    let (root, types, plan) = fixture(selection, split);
    (
        root,
        Program::new(
            types,
            BTreeMap::from([(root.into(), plan)]),
            BTreeMap::new(),
        )
        .unwrap(),
    )
}
#[test]
fn root_contexts_flow_through_operations_and_edges_without_invalidating_unchanged_fields() {
    for split in [false, true] {
        for selection in [Selection::Root, Selection::Preserved] {
            let (root, program) = program(selection, split);
            let mut budget = Budget::new(Limits::default());
            let value = Executor::new(&program, &mut budget, 0)
                .evaluate_initializer(root)
                .unwrap();
            let expected = match selection {
                Selection::Root => Value::aggregate(vec![Value::unit(), Value::integer(42)]),
                _ => Value::integer(42),
            };
            assert_eq!(value, expected);
            drop(value);
            assert_eq!(budget.storage_used(), 0);
        }
    }
}
#[test]
fn stale_function_views_fail_instead_of_inheriting_equal_payload_geometry() {
    let (root, invalid) = program(Selection::Stale, true);
    let mut budget = Budget::new(Limits::default());
    assert_eq!(
        Executor::new(&invalid, &mut budget, 0)
            .evaluate_initializer(root)
            .unwrap_err()
            .rule,
        Rule::InvalidPlan
    );
    assert_eq!(budget.storage_used(), 0);
    let (root, healthy) = program(Selection::Preserved, true);
    let value = Executor::new(&healthy, &mut budget, 0)
        .evaluate_initializer(root)
        .unwrap();
    assert_eq!(value, Value::integer(42));
    drop(value);
    assert_eq!(budget.storage_used(), 0);
}
#[test]
fn retired_backing_keeps_its_actual_function_identity() {
    // This independent kernel rejection grants no source-level lifetime admission.
    let (root, types, mut plan) = fixture(Selection::Stale, false);
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        let actions = &mut point.actions;
        let mut effects = actions[3].alternatives[0].after.to_vec();
        effects.push(Effect::Dispose(Place::root(0)));
        actions[3].alternatives[0].after = effects.into();
        actions[4].alternatives[0].owned.remove(&0);
        actions[4].alternatives[0].after = Box::new([Effect::Construct(3)]);
        point
    });
    let program = Program::new(
        types,
        BTreeMap::from([(root.into(), plan)]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(Limits::default());
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
fn root_context_changes_require_actual_suppliers_and_updated_result_contracts() {
    for case in 0..4 {
        let (root, types, mut plan) = fixture(Selection::Root, true);
        if case == 0 {
            plan.result = 3;
        } else {
            plan.points = plan.points.map(|_, point| {
                let mut point = point.clone();
                for action in &mut point.actions {
                    for alternative in &mut action.alternatives {
                        for effect in &mut alternative.after {
                            if let Effect::ReplaceRoot {
                                source,
                                destination,
                                data,
                            } = effect
                            {
                                match case {
                                    1 => *data = 3,
                                    2 => *source = Place::root(*destination),
                                    3 => *destination = usize::MAX,
                                    _ => unreachable!(),
                                }
                            }
                        }
                    }
                }
                point
            });
        }
        assert!(
            matches!(
                Program::new(
                    types,
                    BTreeMap::from([(root.into(), plan)]),
                    BTreeMap::new()
                ),
                Err(Rule::InvalidPlan)
            ),
            "case {case}"
        );
    }
}
#[test]
fn metered_contexts_release_after_each_failed_request_and_healthy_completion() {
    for (root, program) in [program(Selection::Preserved, true), moving_program(true)] {
        let mut succeeded = false;
        for units in 1..256 {
            let mut budget = Budget::new(Limits::new(
                CompileTimeEvaluationLimits::default(),
                NonZeroU64::new(units).unwrap(),
            ));
            match Executor::new(&program, &mut budget, 0).evaluate_initializer(root) {
                Ok(value) => {
                    assert_eq!(value, Value::integer(42));
                    drop(value);
                    succeeded = true;
                }
                Err(error) => assert_eq!(error.rule, Rule::StorageLimit),
            }
            assert_eq!(budget.storage_used(), 0, "storage {units}");
            if succeeded {
                break;
            }
        }
        assert!(succeeded);
    }
}

#[test]
fn restoring_a_saved_value_restores_its_original_context_and_payload() {
    let (root, types, mut plan) = fixture(Selection::Root, false);
    let mut slots = plan.slots.to_vec();
    slots.push(Slot {
        data: 3,
        storage: Storage::Owned,
    });
    plan.slots = slots.into();
    plan.result = 3;
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        let mut actions = point.actions.to_vec();
        actions.insert(
            3,
            action(
                Operation::Copy {
                    source: Place::root(0),
                    destination: Place::root(4),
                },
                &[0, 1],
                vec![Effect::Construct(4)],
            ),
        );
        actions[4].alternatives[0].owned.insert(4, Box::new([]));
        actions.push(action(
            Operation::None,
            &[0, 4],
            vec![Effect::ReplaceRoot {
                source: Place::root(4),
                destination: 0,
                data: 3,
            }],
        ));
        point.actions = actions.into();
        point
    });
    let program = Program::new(
        types,
        BTreeMap::from([(root.into(), plan)]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(Limits::default());
    let value = Executor::new(&program, &mut budget, 0)
        .evaluate_initializer(root)
        .unwrap();
    assert_eq!(
        value,
        Value::aggregate(vec![Value::unit(), Value::integer(40)])
    );
    drop(value);
    assert_eq!(budget.storage_used(), 0);
}

#[test]
fn retired_roots_receive_new_contexts_only_after_their_original_disposal() {
    for retire_first in [true, false] {
        let (root, program) = moving_program(retire_first);
        let mut budget = Budget::new(Limits::default());
        let result = Executor::new(&program, &mut budget, 0).evaluate_initializer(root);
        if retire_first {
            let value = result.unwrap();
            assert_eq!(value, Value::integer(42));
            drop(value);
        } else {
            assert_eq!(result.unwrap_err().rule, Rule::InvalidPlan);
        }
        assert_eq!(budget.storage_used(), 0);
    }
}

fn constrained_plan(valid: bool) -> (AccessCodeId, Vec<Type>, Plan) {
    let (root, types, mut plan) = fixture(Selection::Preserved, false);
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        let alternative = &mut point.actions[3].alternatives[0];
        alternative.before = Box::new([
            Effect::Context {
                place: Place::root(0),
                data: 3,
            },
            Effect::Context {
                place: Place::root(1),
                data: 4,
            },
        ]);
        let mut after = alternative.after.to_vec();
        after.push(Effect::Context {
            place: Place::root(0),
            data: if valid { 4 } else { 3 },
        });
        alternative.after = after.into();
        point
    });
    (root, types, plan)
}
#[test]
fn contextual_constraints_follow_their_exact_phase_and_do_not_consume_execution_steps() {
    let (root, baseline) = program(Selection::Preserved, false);
    let steps = (1..128)
        .find(|&units| {
            let mut budget = Budget::new(Limits::new(
                CompileTimeEvaluationLimits::new(
                    NonZeroU64::new(units).unwrap(),
                    NonZeroU32::new(4).unwrap(),
                ),
                NonZeroU64::new(256).unwrap(),
            ));
            Executor::new(&baseline, &mut budget, 0)
                .evaluate_initializer(root)
                .is_ok()
        })
        .unwrap();
    let (root, types, plan) = constrained_plan(true);
    let program = Program::new(
        types,
        BTreeMap::from([(root.into(), plan)]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(Limits::new(
        CompileTimeEvaluationLimits::new(
            NonZeroU64::new(steps).unwrap(),
            NonZeroU32::new(4).unwrap(),
        ),
        NonZeroU64::new(256).unwrap(),
    ));
    let value = Executor::new(&program, &mut budget, 0)
        .evaluate_initializer(root)
        .unwrap();
    assert_eq!(value, Value::integer(42));
    drop(value);
    assert_eq!(budget.storage_used(), 0);
    let (root, types, plan) = constrained_plan(false);
    assert!(matches!(
        Program::new(
            types,
            BTreeMap::from([(root.into(), plan)]),
            BTreeMap::new()
        ),
        Err(Rule::InvalidPlan)
    ));
}

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
                destination: 0,
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

#[test]
fn differing_guarded_contexts_need_a_fresh_receipt_before_a_typed_use() {
    for marked in [false, true] {
        for reverse in [false, true] {
            for receive in [false, true] {
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
                    if receive {
                        actions.push(action(
                            Operation::Literal {
                                destination: 1,
                                value: Value::aggregate(vec![Value::unit(), Value::integer(42)]),
                            },
                            &[0],
                            vec![Effect::Construct(1)],
                        ));
                        actions.push(point.actions[3].clone());
                    }
                    point.actions = actions.into();
                    point
                });
                let result = Program::new(
                    types,
                    BTreeMap::from([(root.into(), plan)]),
                    BTreeMap::new(),
                );
                if !receive {
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

fn consuming_plan() -> Plan {
    let mut points = ArenaBuilder::new();
    let entry = points.insert(Point {
        actions: Box::new([action(
            Operation::Copy {
                destination: Place::root(1),
                source: Place {
                    slot: 0,
                    fields: vec![1].into(),
                },
            },
            &[0],
            vec![Effect::Construct(1), Effect::Dispose(Place::root(0))],
        )]),
        terminator: Terminator::Return(Place::root(1)),
    });
    Plan {
        slots: [
            Slot {
                data: 4,
                storage: Storage::Owned,
            },
            Slot {
                data: 0,
                storage: Storage::Owned,
            },
        ]
        .into(),
        inputs: Box::new([(0, nocter_model::ValueInputOperation::Take)]),
        statics: Box::new([]),
        environment_backing: None,
        result: 0,
        initializer: false,
        entry,
        points: points.finish(),
    }
}
#[test]
fn taking_protection_uses_the_context_at_the_exact_before_effect() {
    for supplier_first in [true, false] {
        let (root, types, mut plan) = fixture(Selection::Root, false);
        let Type::NamedFunction(callee) = types[2] else {
            unreachable!()
        };
        Arc::make_mut(&mut plan.slots)[3].data = 0;
        plan.result = 0;
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            let last = point.actions.last_mut().unwrap();
            last.operation = Operation::Call {
                destination: 3,
                target: callee.into(),
                inputs: Box::new([Input::Take(4)]),
            };
            let alternative = &mut last.alternatives[0];
            let replacement = alternative.after[0].clone();
            let protection = Effect::Invoke {
                source: Place::root(0),
                inputs: Box::new([(0, Place::root(0))]),
            };
            alternative.before = if supplier_first {
                vec![replacement, protection]
            } else {
                vec![protection, replacement]
            }
            .into();
            alternative.after = Box::new([Effect::Construct(3)]);
            point.terminator = Terminator::Return(Place::root(3));
            point
        });
        let result = Program::new(
            types,
            BTreeMap::from([(root.into(), plan), (callee.into(), consuming_plan())]),
            BTreeMap::new(),
        );
        if supplier_first {
            let program = result.unwrap();
            let mut budget = Budget::new(Limits::default());
            let value = Executor::new(&program, &mut budget, 0)
                .evaluate_initializer(root)
                .unwrap();
            assert_eq!(value, Value::integer(42));
            drop(value);
            assert_eq!(budget.storage_used(), 0);
        } else {
            assert!(matches!(result, Err(Rule::InvalidPlan)));
        }
    }
}

fn moving_program(retire_first: bool) -> (AccessCodeId, Program) {
    let (root, types, mut plan) = fixture(Selection::Preserved, true);
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        for action in &mut point.actions {
            for alternative in &mut action.alternatives {
                for effect in &mut alternative.after {
                    if let Effect::ReplaceRoot {
                        source,
                        destination,
                        data,
                    } = effect
                    {
                        *effect = Effect::MoveRoot {
                            source: source.clone(),
                            destination: *destination,
                            data: *data,
                        };
                        if retire_first {
                            alternative.before = Box::new([Effect::Dispose(Place::root(0))]);
                        }
                    }
                }
            }
        }
        point
    });
    let program = Program::new(
        types,
        BTreeMap::from([(root.into(), plan)]),
        BTreeMap::new(),
    )
    .unwrap();
    (root, program)
}
