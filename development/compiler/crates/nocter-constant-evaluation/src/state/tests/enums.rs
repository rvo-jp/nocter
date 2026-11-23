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

fn nested_payload_types() -> [Type; 6] {
    let nominal = ArenaBuilder::<nocter_model::NominalTypeId, ()>::new().insert(());
    // Both variants have the same field shape and ordinal.
    [
        Type::Integer(nocter_model::BuiltinType::I32),
        Type::Bool,
        Type::Integer(nocter_model::BuiltinType::Usize),
        Type::Array {
            element: 0,
            length: 4,
        },
        Type::Record(Box::new([1, 3])),
        Type::Enum {
            nominal,
            cases: Box::new([Box::new([4]), Box::new([4])]),
        },
    ]
}

fn payload_paths(tag: u64, field: usize) -> Plan {
    let mut plan = literal();
    let mut slots = owned_slots([5, 5, 2, 0, 0]).to_vec();
    slots.extend([
        Slot {
            data: 4,
            storage: Storage::Alias,
        },
        Slot {
            data: 0,
            storage: Storage::Owned,
        },
    ]);
    plan.slots = slots.into();
    let index = Projection::Index { slot: 2, bound: 4 };
    let case = Projection::EnumField { tag, field };
    let selected = Place {
        slot: 5,
        fields: vec![Projection::Field(1), index].into(),
    };
    let array_pattern = [case, Projection::Field(1), index];
    let projected = |slot| Place {
        slot,
        fields: array_pattern.to_vec().into(),
    };
    let copy = |source, slot| Operation::Copy {
        source,
        destination: Place::root(slot),
    };
    let mut dispose = action(
        copy(projected(1), 4),
        &[0, 1, 2, 3],
        vec![
            Effect::Construct(4),
            Effect::Release(Place::root(0)),
            Effect::Dispose(Place::root(1)),
            Effect::Dispose(Place::root(2)),
        ],
    );
    dispose.alternatives[0]
        .owned
        .insert(0, Box::new([projected(0).fields.into()]));
    let mut points = ArenaBuilder::new();
    plan.entry = points.insert(Point {
        actions: Box::new([
            acquire_literal(
                0,
                Value::enumeration(
                    1,
                    vec![Value::aggregate(vec![
                        Value::boolean(true),
                        Value::aggregate([10, 20, 30, 40].map(Value::integer)),
                    ])],
                ),
                &[],
            ),
            action(copy(Place::root(0), 1), &[0], vec![Effect::Construct(1)]),
            acquire_literal(2, Value::integer(2), &[0, 1]),
            acquire_literal(3, Value::integer(12), &[0, 1, 2]),
            action(
                Operation::Alias {
                    destination: 5,
                    source: Place {
                        slot: 0,
                        fields: vec![case].into(),
                    },
                },
                &[0, 1, 2, 3],
                vec![Effect::Replace {
                    source: Place::root(3),
                    destination: selected.clone(),
                }],
            ),
            action(
                Operation::None,
                &[0, 1, 2],
                vec![Effect::Move {
                    source: projected(0),
                    destination: Place::root(3),
                }],
            ),
            dispose,
            action(
                Operation::Binary {
                    destination: Place::root(6),
                    operation: crate::CompileTimeBinaryOperation::Add,
                    left: Place::root(3),
                    right: Place::root(4),
                },
                &[3, 4],
                vec![
                    Effect::Construct(6),
                    Effect::Dispose(Place::root(3)),
                    Effect::Dispose(Place::root(4)),
                ],
            ),
        ]),
        terminator: Terminator::Return(Place::root(6)),
    });
    plan.points = points.finish();
    plan
}

fn acquire_literal(slot: usize, value: Value, owned: &[usize]) -> Action {
    action(
        Operation::Literal {
            destination: slot,
            value,
        },
        owned,
        vec![Effect::Construct(slot)],
    )
}

#[test]
fn enum_coordinates_keep_aliases_copies_and_retired_array_elements_exact() {
    let (id, _) = code_ids();
    let build = |tag, field| {
        Program::new(
            nested_payload_types(),
            BTreeMap::from([(id.into(), payload_paths(tag, field))]),
            BTreeMap::new(),
        )
    };
    let mut budget = Budget::new(Limits::default());
    let program = build(1, 0).unwrap();
    let actual = Executor::new(&program, &mut budget, 0)
        .evaluate_initializer(id)
        .unwrap();
    assert_eq!(actual, Value::integer(42));
    drop(actual);

    let wrong_case = build(0, 0).unwrap();
    let error = Executor::new(&wrong_case, &mut budget, 0)
        .evaluate_initializer(id)
        .unwrap_err();
    assert_eq!(error.rule, Rule::InvalidPlan);
    assert!(build(1, 1).is_err());
    assert!(build(2, 0).is_err());
    assert_eq!(budget.storage_used(), 0);
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
