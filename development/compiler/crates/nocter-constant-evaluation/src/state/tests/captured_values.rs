//! Captured payloads reuse aggregate construction, selected views and ordinary metered storage.
use super::*;
use nocter_model::ValueClosure;

fn code(owner: AccessCodeId, slot: u32) -> ValueClosure {
    ValueClosure {
        owner: owner.into(),
        slot,
    }
}
fn action(operation: Operation, owned: &[usize], after: Vec<Effect>) -> Action {
    Action {
        operation,
        alternatives: vec![Alternative {
            owned: owned
                .iter()
                .copied()
                .map(|slot| (slot, Box::new([]) as Box<[Missing]>))
                .collect(),
            before: Box::new([]),
            after: after.into(),
        }]
        .into(),
    }
}
fn capture(destination: usize, source: usize) -> Effect {
    Effect::Assemble {
        destination,
        fields: Box::new([(0, Place::root(source))]),
    }
}
fn independent_copies(owner: AccessCodeId) -> Program {
    let types = [
        Type::Integer(nocter_model::BuiltinType::I32),
        Type::CapturedFunction {
            code: code(owner, 0),
            environment: 0,
        },
        Type::CapturedFunction {
            code: code(owner, 0),
            environment: 0,
        },
        Type::Record(vec![2, 2].into()),
    ];
    let mut points = ArenaBuilder::new();
    let entry = points.insert(Point {
        actions: vec![
            action(
                Operation::Literal {
                    destination: 0,
                    value: Value::integer(40),
                },
                &[],
                vec![
                    Effect::Construct(0),
                    capture(1, 0),
                    Effect::AdaptFunction {
                        source: Place::root(1),
                        destination: Place::root(2),
                    },
                ],
            ),
            action(
                Operation::Copy {
                    destination: Place::root(3),
                    source: Place::root(2),
                },
                &[2],
                vec![Effect::Construct(3)],
            ),
            action(
                Operation::Literal {
                    destination: 4,
                    value: Value::integer(42),
                },
                &[2, 3],
                vec![],
            ),
            action(
                Operation::Copy {
                    destination: Place {
                        slot: 3,
                        fields: vec![0].into(),
                    },
                    source: Place::root(4),
                },
                &[2, 3],
                vec![],
            ),
            action(
                Operation::None,
                &[2, 3],
                vec![Effect::Assemble {
                    destination: 5,
                    fields: Box::new([(0, Place::root(2)), (1, Place::root(3))]),
                }],
            ),
        ]
        .into(),
        terminator: Terminator::Return(Place::root(5)),
    });
    let slots = [
        (0, Storage::Owned),
        (1, Storage::Owned),
        (2, Storage::Owned),
        (2, Storage::Owned),
        (0, Storage::Auxiliary),
        (3, Storage::Owned),
    ]
    .map(|(data, storage)| Slot { data, storage });
    Program::new(
        types,
        BTreeMap::from([(
            owner.into(),
            Plan {
                slots: slots.into(),
                inputs: Box::new([]),
                statics: Box::new([]),
                conditions: Box::new([]),
                environment_backing: None,
                result: 3,
                initializer: true,
                entry,
                points: points.finish(),
            },
        )]),
        BTreeMap::new(),
    )
    .unwrap()
}
#[test]
fn captured_views_share_code_but_copied_environment_updates_are_independent() {
    let (root, _) = code_ids();
    let program = independent_copies(root);
    let mut budget = Budget::new(Limits::default());
    let value = Executor::new(&program, &mut budget, 0)
        .evaluate_initializer(root)
        .unwrap();
    assert_eq!(
        value,
        Value::aggregate(vec![
            Value::aggregate(vec![Value::integer(40)]),
            Value::aggregate(vec![Value::integer(42)]),
        ])
    );
    assert!(budget.storage_used() > 0);
    drop(value);
    assert_eq!(budget.storage_used(), 0);
}
#[test]
fn every_capture_copy_and_update_storage_failure_refunds_its_partial_work() {
    let (root, _) = code_ids();
    let program = independent_copies(root);
    let mut recovered = false;
    for limit in 1..=128 {
        let mut budget = Budget::new(Limits::new(
            CompileTimeEvaluationLimits::default(),
            NonZeroU64::new(limit).unwrap(),
        ));
        let result = Executor::new(&program, &mut budget, 0).evaluate_initializer(root);
        match result {
            Err(error) => assert_eq!(error.rule, Rule::StorageLimit),
            Ok(value) => {
                drop(value);
                recovered = true;
            }
        }
        assert_eq!(budget.storage_used(), 0, "limit {limit}");
        if recovered {
            break;
        }
    }
    assert!(
        recovered,
        "this fixed six-slot graph fits well below 128 logical items"
    );
}
#[test]
fn captured_schema_and_adapter_edges_preserve_code_and_environment_identity() {
    let (root, _) = code_ids();
    for types in [
        vec![Type::CapturedFunction {
            code: code(root, 0),
            environment: 0,
        }],
        vec![
            Type::Integer(nocter_model::BuiltinType::I32),
            Type::CapturedFunction {
                code: code(root, 0),
                environment: 2,
            },
        ],
    ] {
        assert!(matches!(
            Program::new(types, BTreeMap::new(), BTreeMap::new()),
            Err(Rule::InvalidPlan)
        ));
    }
    for destination in [
        Type::CapturedFunction {
            code: code(root, 1),
            environment: 0,
        },
        Type::CapturedFunction {
            code: code(root, 0),
            environment: 1,
        },
    ] {
        let mut plan = literal();
        plan.slots = vec![
            Slot {
                data: 2,
                storage: Storage::Owned,
            },
            Slot {
                data: 3,
                storage: Storage::Owned,
            },
        ]
        .into();
        plan.result = 3;
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            point.actions[0].operation = Operation::Literal {
                destination: 0,
                value: Value::aggregate(vec![Value::integer(42)]),
            };
            point.actions[0].alternatives[0].after = vec![
                Effect::Construct(0),
                Effect::AdaptFunction {
                    source: Place::root(0),
                    destination: Place::root(1),
                },
            ]
            .into();
            point
        });
        assert!(matches!(
            Program::new(
                vec![
                    Type::Integer(nocter_model::BuiltinType::I32),
                    Type::Integer(nocter_model::BuiltinType::Usize),
                    Type::CapturedFunction {
                        code: code(root, 0),
                        environment: 0
                    },
                    destination
                ],
                BTreeMap::from([(root.into(), plan)]),
                BTreeMap::new()
            ),
            Err(Rule::InvalidPlan)
        ));
    }
}
