//! Closed execution evidence; checking still owns source access and initialization proofs.
use super::*;
mod cleanup;
mod prefix;
mod remainder;

fn action(operation: Operation, owned: &[usize], after: Vec<Effect>) -> Action {
    Action {
        operation,
        alternatives: vec![Alternative {
            owned: owned
                .iter()
                .map(|&slot| (slot, Box::new([]) as Box<[Missing]>))
                .collect(),
            before: Box::new([]),
            after: after.into(),
        }]
        .into(),
    }
}
fn types(length: u64) -> Vec<Type> {
    vec![Type::I32, Type::Usize, Type::Array { element: 0, length }]
}
fn array(values: &[i32]) -> Value {
    Value::aggregate(
        values
            .iter()
            .copied()
            .map(Value::integer)
            .collect::<Box<[_]>>(),
    )
}
fn index(destination: usize) -> Operation {
    Operation::Index {
        destination,
        base: Place::root(0),
        index: Place::root(1),
    }
}
fn integer(destination: usize, value: impl Into<i128>) -> Operation {
    Operation::Literal {
        destination,
        value: Value::integer(value),
    }
}
fn copy(destination: usize, source: usize) -> Operation {
    Operation::Copy {
        destination: Place::root(destination),
        source: Place::root(source),
    }
}
fn add(destination: usize, left: usize, right: usize) -> Operation {
    Operation::Binary {
        destination: Place::root(destination),
        operation: crate::CompileTimeBinaryOperation::Add,
        left: Place::root(left),
        right: Place::root(right),
    }
}
fn finish(operation: Operation) -> Action {
    action(
        operation,
        &[0],
        vec![Effect::Dispose(Place::root(0)), Effect::Construct(4)],
    )
}
fn script(values: &[i32], operations: Vec<Operation>) -> Plan {
    let mut actions = vec![action(
        Operation::Literal {
            destination: 0,
            value: array(values),
        },
        &[],
        vec![Effect::Construct(0)],
    )];
    let last = operations.len() - 1;
    actions.extend(operations.into_iter().enumerate().map(|(i, operation)| {
        if i == last {
            finish(operation)
        } else {
            action(operation, &[0], vec![])
        }
    }));
    let mut points = ArenaBuilder::new();
    let entry = points.insert(Point {
        actions: actions.into(),
        terminator: Terminator::Return(Place::root(4)),
    });
    Plan {
        slots: vec![
            Slot {
                data: 2,
                storage: Storage::Owned,
            },
            Slot {
                data: 1,
                storage: Storage::Auxiliary,
            },
            Slot {
                data: 0,
                storage: Storage::Alias,
            },
            Slot {
                data: 0,
                storage: Storage::Alias,
            },
            Slot {
                data: 0,
                storage: Storage::Owned,
            },
            Slot {
                data: 0,
                storage: Storage::Auxiliary,
            },
            Slot {
                data: 2,
                storage: Storage::Auxiliary,
            },
        ]
        .into(),
        inputs: Box::new([]),
        statics: Box::new([]),
        result: 0,
        initializer: true,
        entry,
        points: points.finish(),
    }
}
fn read(position: u64, values: &[i32]) -> Plan {
    script(values, vec![integer(1, position), index(2), copy(4, 2)])
}
fn execute(plan: Plan, types: Vec<Type>) -> Value {
    let (code, _) = code_ids();
    let program = Program::new(
        types,
        BTreeMap::from([(code.into(), plan)]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(CompileTimeEvaluationLimits::default());
    let value = Executor::new(&program, &mut budget, 0)
        .evaluate_initializer(code)
        .unwrap();
    assert_eq!(Value::from_frozen(&value.freeze()).unwrap(), value);
    value
}

#[test]
fn saved_addresses_survive_index_mutation_and_edit_distinct_elements() {
    let plan = script(
        &[11, 22],
        vec![
            integer(1, 0),
            index(2),
            integer(1, 1),
            index(3),
            integer(5, 77),
            copy(2, 5),
            add(4, 2, 3),
        ],
    );
    assert_eq!(execute(plan, types(2)), Value::integer(99));
}

#[test]
fn indexed_copy_is_a_snapshot_with_a_data_result_instead_of_an_alias() {
    let mut plan = script(
        &[11, 22],
        vec![
            integer(1, 0),
            Operation::IndexCopy {
                destination: 5,
                base: Place::root(0),
                index: Place::root(1),
            },
            index(2),
            integer(1, 1),
            integer(3, 77),
            copy(2, 3),
            add(4, 5, 2),
        ],
    );
    let mut slots = plan.slots.to_vec();
    slots[3].storage = Storage::Auxiliary;
    plan.slots = slots.clone().into();
    assert_eq!(execute(plan.clone(), types(2)), Value::integer(88));

    slots[5].storage = Storage::Alias;
    plan.slots = slots.into();
    let (code, _) = code_ids();
    assert!(matches!(
        Program::new(
            types(2),
            BTreeMap::from([(code.into(), plan)]),
            BTreeMap::new(),
        ),
        Err(Rule::InvalidPlan)
    ));
}

#[test]
fn selected_nested_storage_keeps_independent_array_snapshots() {
    let mut plan = script(
        &[11, 22],
        vec![
            copy(6, 0),
            integer(1, 0),
            index(2),
            integer(1, 1),
            integer(5, 77),
            copy(2, 5),
            Operation::Binary {
                destination: Place::root(4),
                operation: crate::CompileTimeBinaryOperation::Add,
                left: Place {
                    slot: 6,
                    fields: vec![0, 0].into(),
                },
                right: Place::root(2),
            },
        ],
    );
    let mut slots = plan.slots.to_vec();
    slots[0].data = 3;
    slots[6].data = 3;
    plan.slots = slots.into();
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Literal {
            destination: 0,
            value: Value::aggregate(vec![array(&[11, 22])]),
        };
        for action in &mut point.actions {
            if let Operation::Index { base, .. } = &mut action.operation {
                base.fields = vec![0].into();
            }
        }
        point
    });
    let mut domain = types(2);
    domain.push(Type::Record(vec![2].into()));
    assert_eq!(execute(plan, domain), Value::integer(88));
}

#[test]
fn evaluated_bounds_fail_at_the_selection_and_release_frames_before_reuse() {
    let (good, bad) = code_ids();
    for (values, length, invalid) in [
        (vec![], 0, 0),
        (vec![11, 22], 2, 2),
        (vec![11, 22], 2, u64::MAX),
    ] {
        let program = Program::new(
            types(length),
            BTreeMap::from([
                (bad.into(), read(invalid, &values)),
                (good.into(), super::literal()),
            ]),
            BTreeMap::new(),
        )
        .unwrap();
        let mut budget = Budget::new(CompileTimeEvaluationLimits::new(
            NonZeroU64::new(100).unwrap(),
            NonZeroU32::new(1).unwrap(),
        ));
        let mut executor = Executor::new(&program, &mut budget, 0);
        for _ in 0..2 {
            let failure = executor.evaluate_initializer(bad).unwrap_err();
            assert_eq!(failure.rule, Rule::IndexOutOfBounds);
            assert_eq!(failure.site.unwrap().action, 2);
            assert_eq!(failure.code, bad.into());
            assert!(failure.trace.is_empty());
            assert_eq!(
                executor.evaluate_initializer(good).unwrap(),
                Value::integer(42)
            );
        }
        drop(executor);
        assert_eq!(budget.storage_used(), 0);
    }
}

#[test]
fn homogeneous_metadata_and_every_index_role_validate_before_execution() {
    let (code, _) = code_ids();
    let valid = read(0, &[11, 22]);
    for (slot, data, storage) in [
        (0, 0, Storage::Owned),
        (1, 0, Storage::Auxiliary),
        (2, 1, Storage::Alias),
        (2, 0, Storage::Owned),
        (2, 0, Storage::Auxiliary),
    ] {
        let mut invalid = valid.clone();
        let mut slots = invalid.slots.to_vec();
        slots[slot] = Slot { data, storage };
        invalid.slots = slots.into();
        assert!(matches!(
            Program::new(
                types(2),
                BTreeMap::from([(code.into(), invalid)]),
                BTreeMap::new()
            ),
            Err(Rule::InvalidPlan)
        ));
    }
    for domain in [
        types(1),
        vec![Type::I32, Type::Usize, Type::Record(vec![0, 0].into())],
        vec![
            Type::I32,
            Type::Usize,
            Type::Array {
                element: 1,
                length: 2,
            },
        ],
    ] {
        assert!(matches!(
            Program::new(
                domain,
                BTreeMap::from([(code.into(), valid.clone())]),
                BTreeMap::new()
            ),
            Err(Rule::InvalidPlan)
        ));
    }
    // Physical aggregate storage never identifies unit with an empty array or erases element data.
    for (domain, value) in [
        (types(0), Value::unit()),
        (
            types(2),
            Value::repeated(Value::integer(11), NonZeroU64::new(1).unwrap()),
        ),
        (
            types(2),
            Value::repeated(Value::boolean(true), NonZeroU64::new(2).unwrap()),
        ),
        (
            vec![Type::I32, Type::Usize, Type::Record(vec![0, 0].into())],
            Value::repeated(Value::integer(11), NonZeroU64::new(2).unwrap()),
        ),
        (
            types(2),
            Value::aggregate(vec![Value::integer(11), Value::boolean(true)]),
        ),
    ] {
        let mut invalid = read(0, &[]);
        invalid.points = invalid.points.map(|_, point| {
            let mut point = point.clone();
            point.actions[0].operation = Operation::Literal {
                destination: 0,
                value: value.clone(),
            };
            point
        });
        assert!(matches!(
            Program::new(
                domain,
                BTreeMap::from([(code.into(), invalid)]),
                BTreeMap::new()
            ),
            Err(Rule::InvalidPlan)
        ));
    }
}

#[test]
fn array_metadata_preserves_finite_containment_without_expanding_counts() {
    for domain in [
        vec![Type::Array {
            element: 0,
            length: 1,
        }],
        vec![Type::Array {
            element: 1,
            length: 0,
        }],
    ] {
        assert!(matches!(
            Program::new(domain, BTreeMap::new(), BTreeMap::new()),
            Err(Rule::InvalidPlan)
        ));
    }
    Program::new(
        vec![
            Type::I32,
            Type::Array {
                element: 0,
                length: u64::MAX,
            },
        ],
        BTreeMap::new(),
        BTreeMap::new(),
    )
    .unwrap();
    Program::new(
        vec![Type::Array {
            element: 0,
            length: 0,
        }],
        BTreeMap::new(),
        BTreeMap::new(),
    )
    .unwrap();
}

#[test]
fn listed_array_assembly_and_publication_share_the_aggregate_storage_path() {
    let (code, _) = code_ids();
    for values in [vec![], vec![11], vec![11, 22]] {
        let count = values.len();
        let mut actions = Vec::new();
        for (slot, &value) in values.iter().enumerate() {
            actions.push(action(
                integer(slot, value),
                &(0..slot).collect::<Vec<_>>(),
                vec![Effect::Construct(slot)],
            ));
        }
        actions.push(action(
            Operation::None,
            &(0..count).collect::<Vec<_>>(),
            vec![Effect::Assemble {
                destination: count,
                fields: (0..count)
                    .rev()
                    .map(|index| (index, Place::root(index)))
                    .collect(),
            }],
        ));
        let mut slots = vec![
            Slot {
                data: 0,
                storage: Storage::Owned
            };
            count
        ];
        slots.push(Slot {
            data: 2,
            storage: Storage::Owned,
        });
        let mut points = ArenaBuilder::new();
        let entry = points.insert(Point {
            actions: actions.into(),
            terminator: Terminator::Return(Place::root(count)),
        });
        let plan = Plan {
            slots: slots.into(),
            result: 2,
            initializer: true,
            inputs: Box::new([]),
            statics: Box::new([]),
            entry,
            points: points.finish(),
        };
        for length in [u64::try_from(count + 1).unwrap(), u64::MAX] {
            assert!(matches!(
                Program::new(
                    types(length),
                    BTreeMap::from([(code.into(), plan.clone())]),
                    BTreeMap::new()
                ),
                Err(Rule::InvalidPlan)
            ));
        }
        let result = execute(plan, types(u64::try_from(count).unwrap()));
        assert_eq!(result, array(&values));
        assert_ne!(result, Value::unit());
    }
}

#[test]
fn failed_array_imports_and_captured_paths_cannot_leak_live_storage() {
    let (code, _) = code_ids();
    let program = Program::new(
        types(2),
        BTreeMap::from([(code.into(), read(1, &[11, 22]))]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut failures = 0;
    let mut successes = 0;
    for limit in 1..=64 {
        let mut budget = Budget::new(Limits::new(
            CompileTimeEvaluationLimits::default(),
            NonZeroU64::new(limit).unwrap(),
        ));
        match Executor::new(&program, &mut budget, 0).evaluate_initializer(code) {
            Ok(value) => {
                assert_eq!(value, Value::integer(22));
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

#[test]
fn repeated_selection_on_a_backedge_uses_new_values_and_preserves_older_addresses() {
    let mut plan = read(0, &[11, 22]);
    let mut slots = plan.slots.to_vec();
    slots.extend([
        Slot {
            data: 3,
            storage: Storage::Auxiliary,
        },
        Slot {
            data: 1,
            storage: Storage::Auxiliary,
        },
        Slot {
            data: 1,
            storage: Storage::Auxiliary,
        },
    ]);
    plan.slots = slots.into();
    let base = plan.points.get(plan.entry).unwrap().actions[0].clone();
    let mut points = ArenaBuilder::new();
    let setup = points.insert(Point {
        actions: Box::new([]),
        terminator: Terminator::Unreachable,
    });
    let body = points.insert(Point {
        actions: Box::new([]),
        terminator: Terminator::Unreachable,
    });
    let exit = points.insert(Point {
        actions: Box::new([]),
        terminator: Terminator::Unreachable,
    });
    plan.entry = setup;
    plan.points = points.finish().map(|point, _| {
        if point == setup {
            Point {
                actions: vec![
                    base.clone(),
                    action(integer(1, 0), &[0], vec![]),
                    action(integer(5, 0), &[0], vec![]),
                    action(integer(8, 1), &[0], vec![]),
                    action(integer(9, 2), &[0], vec![]),
                    action(index(3), &[0], vec![]),
                ]
                .into(),
                terminator: Terminator::Jump(body),
            }
        } else if point == body {
            Point {
                actions: vec![
                    action(index(2), &[0], vec![]),
                    action(add(5, 5, 2), &[0], vec![]),
                    action(add(1, 1, 8), &[0], vec![]),
                    action(
                        Operation::Compare {
                            destination: 7,
                            operation: crate::CompileTimeComparisonOperation::Less,
                            left: Place::root(1),
                            right: Place::root(9),
                        },
                        &[0],
                        vec![],
                    ),
                ]
                .into(),
                terminator: Terminator::Branch {
                    condition: Place::root(7),
                    yes: body,
                    no: exit,
                },
            }
        } else {
            Point {
                actions: vec![finish(add(4, 5, 3))].into(),
                terminator: Terminator::Return(Place::root(4)),
            }
        }
    });
    let mut domain = types(2);
    domain.push(Type::Bool);
    // Sum both elements through one repeated recipe, then read the original saved element.
    assert_eq!(execute(plan, domain), Value::integer(44));
}

#[test]
fn selected_addresses_cross_call_frames_without_reloading_the_index_variable() {
    let (code, helper) = code_ids();
    let initializer = script(
        &[11, 22],
        vec![
            integer(1, 0),
            index(2),
            integer(1, 1),
            Operation::Call {
                destination: 5,
                target: helper.into(),
                inputs: vec![Input {
                    operation: nocter_model::ValueInputOperation::Edit,
                    place: Place::root(2),
                }]
                .into(),
            },
            Operation::Binary {
                destination: Place::root(4),
                operation: crate::CompileTimeBinaryOperation::Add,
                left: Place::root(2),
                right: Place {
                    slot: 0,
                    fields: vec![1].into(),
                },
            },
        ],
    );
    let mut points = ArenaBuilder::new();
    let entry = points.insert(Point {
        actions: vec![
            action(integer(1, 77), &[], vec![]),
            action(copy(0, 1), &[], vec![]),
            action(copy(2, 0), &[], vec![Effect::Construct(2)]),
        ]
        .into(),
        terminator: Terminator::Return(Place::root(2)),
    });
    let function = Plan {
        slots: vec![
            Slot {
                data: 0,
                storage: Storage::Alias,
            },
            Slot {
                data: 0,
                storage: Storage::Auxiliary,
            },
            Slot {
                data: 0,
                storage: Storage::Owned,
            },
        ]
        .into(),
        inputs: vec![(0, nocter_model::ValueInputOperation::Edit)].into(),
        statics: Box::new([]),
        result: 0,
        initializer: false,
        entry,
        points: points.finish(),
    };
    let program = Program::new(
        types(2),
        BTreeMap::from([(code.into(), initializer), (helper.into(), function)]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(CompileTimeEvaluationLimits::default());
    assert_eq!(
        Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(code)
            .unwrap(),
        Value::integer(99)
    );
    assert_eq!(budget.storage_used(), 0);
}

#[test]
fn zero_sized_elements_retain_their_count_and_bounds() {
    let (code, _) = code_ids();
    let mut domain = types(2);
    domain[0] = Type::Unit;
    for position in 0..=2 {
        let mut plan = read(position, &[11, 22]);
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            point.actions[0].operation = Operation::Literal {
                destination: 0,
                value: Value::aggregate(vec![Value::unit(); 2]),
            };
            point
        });
        let program = Program::new(
            domain.clone(),
            BTreeMap::from([(code.into(), plan)]),
            BTreeMap::new(),
        )
        .unwrap();
        let mut budget = Budget::new(CompileTimeEvaluationLimits::default());
        let result = Executor::new(&program, &mut budget, 0).evaluate_initializer(code);
        if position < 2 {
            assert_eq!(result.unwrap(), Value::unit());
        } else {
            assert_eq!(result.unwrap_err().rule, Rule::IndexOutOfBounds);
        }
        assert_eq!(budget.storage_used(), 0);
    }
}

#[test]
fn maximum_repeated_arrays_keep_canonical_indices_edits_and_snapshots_compact() {
    let mut plan = script(
        &[],
        vec![
            copy(6, 0),
            integer(1, u64::MAX - 1),
            index(2),
            integer(5, 77),
            copy(2, 5),
            Operation::Binary {
                destination: Place::root(4),
                operation: crate::CompileTimeBinaryOperation::Add,
                left: Place::root(2),
                right: Place {
                    slot: 6,
                    fields: vec![0].into(),
                },
            },
        ],
    );
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Literal {
            destination: 0,
            value: Value::repeated(Value::integer(11), NonZeroU64::new(u64::MAX).unwrap()),
        };
        point
    });
    assert_eq!(execute(plan, types(u64::MAX)), Value::integer(88));
}

#[test]
fn repetition_operations_validate_the_result_shape_role_count_and_element_identity() {
    let plan = script(
        &[1, 2],
        vec![
            integer(5, 42),
            Operation::Repeat {
                destination: 6,
                source: Place::root(5),
            },
            integer(4, 42),
        ],
    );
    assert_eq!(execute(plan.clone(), types(2)), Value::integer(42));
    let (code, _) = code_ids();
    for (domain, seed_data, result_storage) in [
        (types(0), 0, Storage::Auxiliary),
        (types(1), 0, Storage::Auxiliary),
        (types(2), 1, Storage::Auxiliary),
        (types(2), 0, Storage::Alias),
        (
            vec![Type::I32, Type::Usize, Type::Record(vec![0, 0].into())],
            0,
            Storage::Auxiliary,
        ),
    ] {
        let mut invalid = plan.clone();
        // Keep the initial literal valid for the changed schema; the Repeat operation itself
        // must reject zero/one counts rather than relying on an unrelated literal mismatch.
        if let Type::Array { length, .. } = domain[2] {
            invalid.points = invalid.points.map(|_, point| {
                let mut point = point.clone();
                point.actions[0].operation = Operation::Literal {
                    destination: 0,
                    value: array(&vec![1; usize::try_from(length).unwrap()]),
                };
                point
            });
        }
        let mut slots = invalid.slots.to_vec();
        slots[5].data = seed_data;
        slots[6].storage = result_storage;
        invalid.slots = slots.into();
        assert!(matches!(
            Program::new(
                domain,
                BTreeMap::from([(code.into(), invalid)]),
                BTreeMap::new()
            ),
            Err(Rule::InvalidPlan)
        ));
    }
}
