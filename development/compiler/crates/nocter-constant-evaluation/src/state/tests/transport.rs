//! Guarded closed-plan movement, independently of source admission and physical ABI.
use super::*;

fn plan(source: usize, target: usize, value: &Value) -> Plan {
    let mut plan = literal();
    plan.slots = vec![
        Slot {
            data: source,
            storage: Storage::Owned,
        },
        Slot {
            data: target,
            storage: Storage::Owned,
        },
    ]
    .into();
    plan.result = target;
    plan.points = plan.points.map(|_, original| {
        let mut point = original.clone();
        point.actions[0].operation = Operation::Literal {
            destination: 0,
            value: value.clone(),
        };
        point.actions[0].alternatives[0].after = vec![
            Effect::Construct(0),
            Effect::Transport {
                source: Place::root(0),
                destination: Place::root(1),
            },
        ]
        .into();
        point
    });
    plan
}

#[test]
fn guarded_transport_preserves_absence_and_active_alternate_graphs() {
    let (code, callable) = code_ids();
    let types = vec![
        Type::Uninhabited,
        Type::Integer(nocter_model::BuiltinType::I32),
        Type::Record(vec![1, 1].into()),
        Type::NamedFunction(callable),
        Type::Variant {
            primary: 0,
            alternate: None,
        },
        Type::Variant {
            primary: 3,
            alternate: None,
        },
        Type::Variant {
            primary: 0,
            alternate: Some(2),
        },
        Type::Variant {
            primary: 3,
            alternate: Some(2),
        },
    ];
    for (source, target, value) in [
        (4, 5, Value::variant(false, None)),
        (
            6,
            7,
            Value::variant(
                false,
                Some(Value::aggregate(vec![
                    Value::integer(40),
                    Value::integer(2),
                ])),
            ),
        ),
    ] {
        let program = Program::new(
            types.clone(),
            BTreeMap::from([(code.into(), plan(source, target, &value))]),
            BTreeMap::new(),
        )
        .unwrap();
        let mut budget = Budget::new(Limits::default());
        let answer = Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(code)
            .unwrap();
        assert_eq!(answer, value);
        drop(answer);
        assert_eq!(budget.storage_used(), 0);
    }
}

#[test]
fn closed_transport_rejects_narrowing_unknown_domains_and_changed_callable_contexts() {
    let (a, b) = code_ids();
    let closure = nocter_model::ValueClosure {
        owner: a.into(),
        slot: 0,
    };
    let types = vec![
        Type::Uninhabited,
        Type::Unavailable,
        Type::Unit,
        Type::Integer(nocter_model::BuiltinType::I32),
        Type::NamedFunction(a),
        Type::NamedFunction(b),
        Type::CapturedFunction {
            code: closure,
            environment: 2,
        },
        Type::CapturedFunction {
            code: closure,
            environment: 3,
        },
        Type::Variant {
            primary: 0,
            alternate: None,
        },
        Type::Variant {
            primary: 4,
            alternate: None,
        },
        Type::Variant {
            primary: 1,
            alternate: None,
        },
        Type::Variant {
            primary: 5,
            alternate: None,
        },
        Type::Variant {
            primary: 6,
            alternate: None,
        },
        Type::Variant {
            primary: 7,
            alternate: None,
        },
    ];
    let program = Program::new(types, BTreeMap::new(), BTreeMap::new()).unwrap();
    for (source, target) in [(9, 8), (10, 9), (8, 10), (9, 11), (12, 13), (0, 2), (1, 1)] {
        assert!(
            !super::super::transport::permits(&program, source, target, || Ok(())).unwrap(),
            "{source}->{target}"
        );
    }
    assert!(super::super::transport::permits(&program, 8, 9, || Ok(())).unwrap());
    let invalid = Program::new(
        program.types.clone(),
        BTreeMap::from([(a.into(), plan(9, 8, &Value::variant(false, None)))]),
        BTreeMap::new(),
    );
    assert!(matches!(invalid, Err(Rule::InvalidPlan)));
}

#[test]
fn shared_max_array_transport_is_bounded_and_failed_execution_releases_its_owners() {
    let (code, _) = code_ids();
    let mut types = vec![
        Type::Uninhabited,
        Type::Integer(nocter_model::BuiltinType::I32),
        Type::Variant {
            primary: 0,
            alternate: None,
        },
        Type::Variant {
            primary: 1,
            alternate: None,
        },
    ];
    let (mut source, mut target) = (2, 3);
    let mut value = Value::variant(false, None);
    for _ in 0..48 {
        types.push(Type::Record(vec![source, source].into()));
        source = types.len() - 1;
        types.push(Type::Record(vec![target, target].into()));
        target = types.len() - 1;
        value = Value::aggregate(vec![value.clone(), value]);
    }
    types.push(Type::Array {
        element: source,
        length: u64::MAX,
    });
    source = types.len() - 1;
    types.push(Type::Array {
        element: target,
        length: u64::MAX,
    });
    target = types.len() - 1;
    let value = Value::repeated(value, NonZeroU64::new(u64::MAX).unwrap());
    let program = Program::new(
        types,
        BTreeMap::from([(code.into(), plan(source, target, &value))]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut constrained = Budget::new(CompileTimeEvaluationLimits::new(
        NonZeroU64::new(12).unwrap(),
        NonZeroU32::new(64).unwrap(),
    ));
    for _ in 0..2 {
        constrained.restart_work();
        assert_eq!(
            Executor::new(&program, &mut constrained, 0)
                .evaluate_initializer(code)
                .unwrap_err()
                .rule,
            Rule::StepLimit
        );
        assert_eq!(constrained.storage_used(), 0);
    }
    let mut healthy = Budget::new(Limits::default());
    let answer = Executor::new(&program, &mut healthy, 0)
        .evaluate_initializer(code)
        .unwrap();
    assert_eq!(answer, value);
    drop(answer);
    assert_eq!(healthy.storage_used(), 0);
}

#[test]
fn guarded_transport_cannot_consume_the_same_owner_twice() {
    let (code, _) = code_ids();
    let types = vec![
        Type::Uninhabited,
        Type::Unit,
        Type::Variant {
            primary: 0,
            alternate: None,
        },
        Type::Variant {
            primary: 1,
            alternate: None,
        },
    ];
    let mut invalid = plan(2, 3, &Value::variant(false, None));
    invalid.slots = invalid
        .slots
        .iter()
        .copied()
        .chain([Slot {
            data: 3,
            storage: Storage::Owned,
        }])
        .collect();
    invalid.points = invalid.points.map(|_, original| {
        let mut point = original.clone();
        let mut effects = point.actions[0].alternatives[0].after.to_vec();
        effects.push(Effect::Transport {
            source: Place::root(0),
            destination: Place::root(2),
        });
        point.actions[0].alternatives[0].after = effects.into();
        point
    });
    let program = Program::new(
        types,
        BTreeMap::from([(code.into(), invalid)]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(Limits::default());
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
fn transport_retains_an_adjacent_captured_environment_across_storage_failures() {
    let (code, _) = code_ids();
    let closure = nocter_model::ValueClosure {
        owner: code.into(),
        slot: 0,
    };
    let types = vec![
        Type::Uninhabited,
        Type::Integer(nocter_model::BuiltinType::I32),
        Type::CapturedFunction {
            code: closure,
            environment: 1,
        },
        Type::Variant {
            primary: 0,
            alternate: None,
        },
        Type::Variant {
            primary: 2,
            alternate: None,
        },
        Type::Record(vec![3, 2].into()),
        Type::Record(vec![4, 2].into()),
    ];
    let value = Value::aggregate(vec![
        Value::variant(false, None),
        Value::aggregate(vec![Value::integer(42)]),
    ]);
    let program = Program::new(
        types,
        BTreeMap::from([(code.into(), plan(5, 6, &value))]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut failures = 0;
    let mut successes = 0;
    for storage in 1..64 {
        let limits = Limits::new(
            CompileTimeEvaluationLimits::default(),
            NonZeroU64::new(storage).unwrap(),
        );
        let mut budget = Budget::new(limits);
        match Executor::new(&program, &mut budget, 0).evaluate_initializer(code) {
            Ok(answer) => {
                assert_eq!(answer, value);
                successes += 1;
            }
            Err(error) => {
                assert_eq!(error.rule, Rule::StorageLimit);
                failures += 1;
            }
        }
        assert_eq!(budget.storage_used(), 0);
    }
    assert!(failures > 0 && successes > 0);
}

fn callable_types() -> Vec<Type> {
    let mut codes = ArenaBuilder::new();
    let first = codes.insert(());
    let second = codes.insert(());
    let third = codes.insert(());
    vec![
        Type::NamedFunction(first),
        Type::NamedFunction(second),
        Type::NamedFunction(third),
        Type::CallableAlternatives(vec![1, 0].into()),
        Type::CallableAlternatives(vec![0, 2, 1].into()),
    ]
}

#[test]
fn whole_callable_transport_encodes_injection_and_reindexes_original_cases() {
    let (code, _) = code_ids();
    for (source, input, expected) in [
        (0, Value::unit(), Value::callable(0, Value::unit())),
        (1, Value::unit(), Value::callable(2, Value::unit())),
        (2, Value::unit(), Value::callable(1, Value::unit())),
        (
            3,
            Value::callable(0, Value::unit()),
            Value::callable(2, Value::unit()),
        ),
        (
            3,
            Value::callable(1, Value::unit()),
            Value::callable(0, Value::unit()),
        ),
    ] {
        let program = Program::new(
            callable_types(),
            BTreeMap::from([(code.into(), plan(source, 4, &input))]),
            BTreeMap::new(),
        )
        .unwrap();
        let mut budget = Budget::new(Limits::default());
        let actual = Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(code)
            .unwrap();
        assert_eq!(actual, expected);
        drop(actual);
        assert_eq!(budget.storage_used(), 0);
    }
}

#[test]
fn callable_transport_rejects_missing_cases_narrowing_and_unqualified_nested_encoding() {
    let types = callable_types();
    let program = Program::new(types.clone(), BTreeMap::new(), BTreeMap::new()).unwrap();
    for (source, target) in [(2, 3), (4, 3), (3, 0), (4, 0)] {
        assert!(!super::super::transport::permits(&program, source, target, || Ok(())).unwrap());
    }
    let mut nested = types;
    nested.push(Type::Record(vec![0].into()));
    nested.push(Type::Record(vec![4].into()));
    let program = Program::new(nested, BTreeMap::new(), BTreeMap::new()).unwrap();
    assert!(!super::super::transport::permits(&program, 5, 6, || Ok(())).unwrap());
}

#[test]
fn callable_encoding_releases_failed_work_and_storage_before_a_healthy_retry() {
    let (code, _) = code_ids();
    let input = Value::callable(0, Value::unit());
    let program = Program::new(
        callable_types(),
        BTreeMap::from([(code.into(), plan(3, 4, &input))]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut failed = false;
    let mut succeeded = false;
    for limit in 1..64 {
        let mut budget = Budget::new(Limits::new(
            CompileTimeEvaluationLimits::default(),
            NonZeroU64::new(limit).unwrap(),
        ));
        match Executor::new(&program, &mut budget, 0).evaluate_initializer(code) {
            Ok(value) => {
                assert_eq!(value, Value::callable(2, Value::unit()));
                drop(value);
                succeeded = true;
            }
            Err(error) => {
                assert_eq!(error.rule, Rule::StorageLimit);
                failed = true;
            }
        }
        assert_eq!(budget.storage_used(), 0);
    }
    assert!(failed && succeeded);
    let mut budget = Budget::new(CompileTimeEvaluationLimits::new(
        NonZeroU64::new(4).unwrap(),
        NonZeroU32::new(64).unwrap(),
    ));
    assert_eq!(
        Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(code)
            .unwrap_err()
            .rule,
        Rule::StepLimit
    );
    assert_eq!(budget.storage_used(), 0);
    let mut healthy = Budget::new(Limits::default());
    let answer = Executor::new(&program, &mut healthy, 0)
        .evaluate_initializer(code)
        .unwrap();
    assert_eq!(answer, Value::callable(2, Value::unit()));
    drop(answer);
    assert_eq!(healthy.storage_used(), 0);
}

#[test]
fn whole_callable_injection_preserves_complete_maximum_length_environments() {
    let (code, named) = code_ids();
    let closure = nocter_model::ValueClosure {
        owner: code.into(),
        slot: 0,
    };
    let types = vec![
        Type::Integer(nocter_model::BuiltinType::I32),
        Type::Array {
            element: 0,
            length: u64::MAX,
        },
        Type::CapturedFunction {
            code: closure,
            environment: 1,
        },
        Type::NamedFunction(named),
        Type::CallableAlternatives(vec![3, 2].into()),
    ];
    let payload = Value::aggregate(vec![Value::repeated(
        Value::integer(42),
        NonZeroU64::new(u64::MAX).unwrap(),
    )]);
    let program = Program::new(
        types,
        BTreeMap::from([(code.into(), plan(2, 4, &payload))]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(Limits::new(
        CompileTimeEvaluationLimits::default(),
        NonZeroU64::new(64).unwrap(),
    ));
    let answer = Executor::new(&program, &mut budget, 0)
        .evaluate_initializer(code)
        .unwrap();
    assert_eq!(answer, Value::callable(1, payload));
    assert!(budget.storage_used() < 32);
    drop(answer);
    assert_eq!(budget.storage_used(), 0);
}
