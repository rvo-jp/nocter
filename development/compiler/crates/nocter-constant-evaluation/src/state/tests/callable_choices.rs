//! Complete callable choice values retain one selector without exposing unguarded components.
use super::*;

fn types() -> Vec<Type> {
    let (first, second) = code_ids();
    vec![
        Type::NamedFunction(first),
        Type::NamedFunction(second),
        Type::CallableAlternatives(vec![0, 1].into()),
    ]
}
fn literal_choice(data: usize, value: &Value) -> Plan {
    let mut plan = literal();
    plan.slots = plan
        .slots
        .iter()
        .map(|slot| Slot { data, ..*slot })
        .collect();
    plan.result = data;
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Literal {
            destination: 0,
            value: value.clone(),
        };
        point
    });
    plan
}
#[test]
fn choices_validate_the_selected_payload_and_retain_ordinary_shared_value_storage() {
    let (code, _) = code_ids();
    for case in [0, 1] {
        let expected = Value::callable(case, Value::unit());
        let program = Program::new(
            types(),
            BTreeMap::from([(code.into(), literal_choice(2, &expected))]),
            BTreeMap::new(),
        )
        .unwrap();
        let mut budget = Budget::new(Limits::default());
        let actual = Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(code)
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual.data().callable(), Some((case, &Value::unit())));
        assert_eq!(program.types[2].component(0), None);
        assert_eq!(program.types[2].component(1), None);
        drop(actual);
        assert_eq!(budget.storage_used(), 0);
    }
}
#[test]
fn malformed_selectors_and_unmatched_payloads_reject_before_execution() {
    let (code, _) = code_ids();
    for value in [
        Value::unit(),
        Value::callable(2, Value::unit()),
        Value::callable(u64::MAX, Value::unit()),
        Value::callable(0, Value::integer(42)),
        Value::aggregate(vec![Value::integer(-1), Value::unit()]),
        Value::aggregate(vec![Value::boolean(false), Value::unit()]),
    ] {
        assert!(matches!(
            Program::new(
                types(),
                BTreeMap::from([(code.into(), literal_choice(2, &value))]),
                BTreeMap::new()
            ),
            Err(Rule::InvalidPlan)
        ));
    }
}
#[test]
fn choice_schemas_reject_missing_duplicate_noncallable_and_nested_cases() {
    for cases in [vec![], vec![0], vec![0, 0], vec![0, 3], vec![0, 2]] {
        let mut types = types();
        types[2] = Type::CallableAlternatives(cases.into());
        assert!(matches!(
            Program::new(types, BTreeMap::new(), BTreeMap::new()),
            Err(Rule::InvalidPlan)
        ));
    }
    let mut noncallable = types();
    noncallable[0] = Type::Unit;
    assert!(matches!(
        Program::new(noncallable, BTreeMap::new(), BTreeMap::new()),
        Err(Rule::InvalidPlan)
    ));
    let mut types = types();
    types.push(Type::CallableAlternatives(vec![0, 2].into()));
    assert!(matches!(
        Program::new(types, BTreeMap::new(), BTreeMap::new()),
        Err(Rule::InvalidPlan)
    ));
    let (first, _) = code_ids();
    let types = vec![
        Type::NamedFunction(first),
        Type::NamedFunction(first),
        Type::CallableAlternatives(vec![0, 1].into()),
    ];
    assert!(matches!(
        Program::new(types, BTreeMap::new(), BTreeMap::new()),
        Err(Rule::InvalidPlan)
    ));
}

#[test]
fn same_lexical_code_keeps_each_complete_selected_environment() {
    let (code, _) = code_ids();
    let closure = nocter_model::ValueClosure {
        owner: code.into(),
        slot: 0,
    };
    let types = vec![
        Type::Integer(nocter_model::BuiltinType::I32),
        Type::Bool,
        Type::Record(vec![0].into()),
        Type::Record(vec![1].into()),
        Type::CapturedFunction {
            code: closure,
            environment: 2,
        },
        Type::CapturedFunction {
            code: closure,
            environment: 3,
        },
        Type::CallableAlternatives(vec![4, 5].into()),
    ];
    for (case, environment) in [(0, Value::integer(42)), (1, Value::boolean(true))] {
        let payload = Value::aggregate(vec![Value::aggregate(vec![environment])]);
        let value = Value::callable(case, payload);
        let program = Program::new(
            types.clone(),
            BTreeMap::from([(code.into(), literal_choice(6, &value))]),
            BTreeMap::new(),
        )
        .unwrap();
        let mut budget = Budget::new(Limits::default());
        let actual = Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(code)
            .unwrap();
        assert_eq!(actual, value);
        let wrong = Value::callable(1 - case, value.data().callable().unwrap().1.clone());
        assert!(!validation::value(&program, 6, &wrong));
        drop(actual);
        assert_eq!(budget.storage_used(), 0);
    }
}

#[test]
fn selected_maximum_array_environments_share_bounded_storage_and_release_failed_imports() {
    let (code, other) = code_ids();
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
        Type::Record(vec![1].into()),
        Type::CapturedFunction {
            code: closure,
            environment: 2,
        },
        Type::NamedFunction(other),
        Type::CallableAlternatives(vec![3, 4].into()),
    ];
    let payload = Value::aggregate(vec![Value::aggregate(vec![Value::repeated(
        Value::integer(42),
        NonZeroU64::new(u64::MAX).unwrap(),
    )])]);
    let expected = Value::callable(0, payload);
    let program = Program::new(
        types,
        BTreeMap::from([(code.into(), literal_choice(5, &expected))]),
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
                assert_eq!(value, expected);
                assert!(budget.storage_used() < 32);
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
}
