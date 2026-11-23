//! Typed selected-case assembly and tag dispatch through the ordinary evaluator plan.
use super::*;

fn types() -> [Type; 4] {
    let nominal = ArenaBuilder::<nocter_model::NominalTypeId, ()>::new().insert(());
    [
        Type::Integer(nocter_model::BuiltinType::I32),
        Type::Bool,
        Type::Unavailable,
        Type::Enum {
            nominal,
            cases: Box::new([Box::new([]), Box::new([0, 1]), Box::new([2])]),
        },
    ]
}

fn action(operation: Operation, owned: &[usize], after: Vec<Effect>) -> Action {
    Action {
        operation,
        alternatives: Box::new([Alternative {
            owned: owned.iter().map(|&slot| (slot, Box::default())).collect(),
            before: Box::new([]),
            after: after.into(),
        }]),
    }
}

fn owned_slots(data: impl IntoIterator<Item = usize>) -> Arc<[Slot]> {
    data.into_iter()
        .map(|data| Slot {
            data,
            storage: Storage::Owned,
        })
        .collect()
}

fn assembly(tag: Option<u64>, order: &[(usize, usize)]) -> Plan {
    let mut plan = literal();
    plan.slots = owned_slots([0, 1, 3]);
    plan.result = 3;
    plan.points = plan.points.map(|_, original| {
        let mut point = original.clone();
        point.actions = Box::new([
            action(
                Operation::Literal {
                    destination: 0,
                    value: Value::integer(42),
                },
                &[],
                vec![Effect::Construct(0)],
            ),
            action(
                Operation::Literal {
                    destination: 1,
                    value: Value::boolean(true),
                },
                &[0],
                vec![Effect::Construct(1)],
            ),
            action(
                Operation::None,
                &[0, 1],
                vec![Effect::Assemble {
                    destination: 2,
                    enum_case: tag,
                    fields: order
                        .iter()
                        .map(|&(field, source)| (field, Place::root(source)))
                        .collect(),
                }],
            ),
        ]);
        point.terminator = Terminator::Return(Place::root(2));
        point
    });
    plan
}

#[test]
fn enum_assembly_checks_the_selected_types_and_consumes_each_staged_field() {
    let (id, _) = code_ids();
    let build = |tag, fields| {
        Program::new(
            types(),
            BTreeMap::from([(id.into(), assembly(tag, fields))]),
            BTreeMap::new(),
        )
    };
    let fields = &[(1, 1), (0, 0)];
    let program = build(Some(1), fields).unwrap();
    let mut budget = Budget::new(Limits::default());
    let result = Executor::new(&program, &mut budget, 0)
        .evaluate_initializer(id)
        .unwrap();
    let expected = Value::enumeration(1, vec![Value::integer(42), Value::boolean(true)]);
    assert_eq!(result, expected);
    drop(result);
    assert_eq!(budget.storage_used(), 0);
    for bad_tag in [None, Some(0), Some(2), Some(3), Some(u64::MAX)] {
        assert!(build(bad_tag, fields).is_err());
    }
    for invalid in [
        &[(0, 0)][..],
        &[(0, 1), (1, 0)],
        &[(0, 0), (0, 0)],
        &[(0, 0), (2, 1)],
    ] {
        assert!(build(Some(1), invalid).is_err());
    }
}

fn dispatch(value: Value, tested_tag: u64) -> Plan {
    let mut plan = literal();
    plan.slots = owned_slots([3, 1]);
    plan.result = 1;
    let mut points = ArenaBuilder::new();
    let selected = points.insert(Point {
        actions: Box::new([action(
            Operation::EnumCondition {
                source: Place::root(0),
                tag: 1,
                destination: 1,
            },
            &[0],
            vec![Effect::Construct(1), Effect::Dispose(Place::root(0))],
        )]),
        terminator: Terminator::Return(Place::root(1)),
    });
    let fallback = points.insert(Point {
        actions: Box::new([action(
            Operation::Literal {
                destination: 1,
                value: Value::boolean(false),
            },
            &[0],
            vec![Effect::Construct(1), Effect::Dispose(Place::root(0))],
        )]),
        terminator: Terminator::Return(Place::root(1)),
    });
    plan.entry = points.insert(Point {
        actions: Box::new([action(
            Operation::Literal {
                destination: 0,
                value,
            },
            &[],
            vec![Effect::Construct(0)],
        )]),
        terminator: Terminator::EnumSwitch {
            source: Place::root(0),
            cases: Arc::new([(tested_tag, selected)]),
            fallback,
        },
    });
    plan.points = points.finish();
    plan
}

#[test]
fn enum_dispatch_validates_the_active_payload_without_qualifying_other_cases() {
    let (id, _) = code_ids();
    let empty = Value::enumeration(0, Vec::new());
    let payload = Value::enumeration(1, vec![Value::integer(42), Value::boolean(true)]);
    let mut budget = Budget::new(Limits::default());
    for (value, tag, expected) in [
        (empty.clone(), 1, Some(false)),
        (payload.clone(), 1, Some(true)),
        (payload, 3, None),
        (empty, 0, Some(false)),
        (Value::enumeration(2, vec![Value::unit()]), 2, None),
        (
            Value::enumeration(1, vec![Value::integer(42), Value::integer(0)]),
            1,
            None,
        ),
    ] {
        let input = BTreeMap::from([(id.into(), dispatch(value, tag))]);
        let program = Program::new(types(), input, BTreeMap::new());
        match expected {
            Some(expected) => {
                let program = program.unwrap();
                let actual = Executor::new(&program, &mut budget, 0)
                    .evaluate_initializer(id)
                    .unwrap();
                assert_eq!(actual, Value::boolean(expected));
            }
            None => assert!(program.is_err()),
        }
        assert_eq!(budget.storage_used(), 0);
    }
}

#[test]
fn enum_leaf_stays_intact_when_a_neighboring_outcome_widens() {
    use super::super::transport;
    let mut table = types().to_vec();
    let enumeration = table[3].clone();
    let Type::Enum { cases, .. } = enumeration else {
        unreachable!()
    };
    let mut identities = ArenaBuilder::<nocter_model::NominalTypeId, ()>::new();
    identities.insert(());
    let distinct = identities.insert(());

    let mut append = |ty| {
        let id = table.len();
        table.push(ty);
        id
    };
    let excluded = append(Type::Uninhabited);
    let narrow = append(Type::Variant {
        primary: excluded,
        alternate: Some(1),
    });
    let wide = append(Type::Variant {
        primary: 0,
        alternate: Some(1),
    });
    let from = append(Type::Record(Box::new([3, narrow])));
    let into = append(Type::Record(Box::new([3, wide])));
    let other = append(Type::Enum {
        nominal: distinct,
        cases,
    });
    let incompatible = append(Type::Record(Box::new([other, wide])));
    let program = Program::new(table, BTreeMap::new(), BTreeMap::new()).unwrap();
    assert!(transport::permits(&program, from, into, || Ok(())).unwrap());
    assert!(!transport::permits(&program, from, incompatible, || Ok(())).unwrap());
    let budget = Budget::new(Limits::default());
    let value = Value::aggregate(vec![
        Value::enumeration(1, vec![Value::integer(42), Value::boolean(true)]),
        Value::variant(false, Some(Value::boolean(false))),
    ]);
    let original = budget.storage.import(&value).unwrap();
    let received =
        transport::encode(&program, from, into, &original, &budget.storage, || Ok(())).unwrap();
    assert_eq!(received.freeze(), value.freeze());
    assert_eq!(received.identity(), original.identity());
}
