//! Selected read environments share ordinary frame backing and checked body identities.
use super::*;

fn program(case: u64) -> (Program, AccessCodeId) {
    let (root, named) = code_ids();
    let closure = ValueClosure {
        owner: root.into(),
        slot: 0,
    };
    let target: CodeInstance = nocter_model::ValueCode::Closure(closure).into();
    let cases = vec![
        CallCase {
            data: 1,
            target: named.into(),
        },
        CallCase { data: 2, target },
    ];
    let payload = if case == 0 {
        Value::unit()
    } else {
        Value::aggregate(vec![Value::integer(41)])
    };
    let caller = plan(
        &[(3, Storage::Owned), (0, Storage::Owned)],
        None,
        0,
        1,
        None,
        vec![
            action(
                &[],
                vec![],
                Operation::Literal {
                    destination: 0,
                    value: Value::callable(case, payload),
                },
                vec![Effect::Construct(0)],
            ),
            action(
                &[0],
                vec![],
                Operation::SelectedCall {
                    destination: 1,
                    callee: Place::root(0),
                    cases: cases.into(),
                    inputs: Box::new([]),
                },
                vec![Effect::Construct(1), Effect::Dispose(Place::root(0))],
            ),
        ],
    );
    let mut named_body = literal();
    named_body.initializer = false;
    let program = Program::new(
        vec![
            Type::Integer(nocter_model::BuiltinType::I32),
            Type::NamedFunction(named),
            Type::CapturedFunction {
                code: closure,
                environment: 0,
            },
            Type::CallableAlternatives(vec![1, 2].into()),
        ],
        BTreeMap::from([
            (root.into(), caller),
            (named.into(), named_body),
            (target, body(Op::Read)),
        ]),
        BTreeMap::new(),
    )
    .unwrap();
    (program, root)
}

#[test]
fn stored_cases_select_only_the_matching_named_or_captured_body() {
    for (case, expected) in [(0, 42), (1, 41)] {
        let (program, root) = program(case);
        let mut budget = Budget::new(Limits::default());
        let actual = Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(root)
            .unwrap();
        assert_eq!(actual, Value::integer(expected));
        drop(actual);
        assert_eq!(budget.storage_used(), 0);
    }
}

#[test]
fn selected_contracts_reject_wrong_case_code_coverage_and_non_read_environments() {
    let (original, root) = program(1);
    for corruption in 0..5 {
        let mut changed = original.clone();
        match corruption {
            0..=2 => {
                let plan = Arc::make_mut(changed.plans.get_mut(&root.into()).unwrap());
                plan.points = plan.points.map(|_, point| {
                    let mut point = point.clone();
                    let Operation::SelectedCall { cases, .. } = &mut point.actions[1].operation
                    else {
                        unreachable!();
                    };
                    match corruption {
                        0 => cases.swap(0, 1),
                        1 => *cases = cases[..1].into(),
                        2 => cases[0].target = cases[1].target,
                        _ => unreachable!(),
                    }
                    point
                });
            }
            3..=4 => {
                let target = *changed
                    .contracts
                    .iter()
                    .find(|(_, contract)| contract.environment_backing)
                    .unwrap()
                    .0;
                let mut body = body(if corruption == 3 { Op::Edit } else { Op::Read });
                if corruption == 4 {
                    body.environment_backing = None;
                }
                changed
                    .contracts
                    .insert(target, CallContract::from_plan(&body).unwrap());
                changed.plans.insert(target, Arc::new(body));
            }
            _ => unreachable!(),
        }
        assert!(
            validation::validate(&changed).is_err(),
            "corruption {corruption}"
        );
    }
}

fn explicit_body(operation: Op) -> Plan {
    let mut result = body(Op::Read);
    let mut slots = result.slots.to_vec();
    slots.push(Slot {
        data: 0,
        storage: if operation == Op::Take {
            Storage::Owned
        } else {
            Storage::Alias
        },
    });
    result.slots = slots.into();
    result.inputs = vec![(0, Op::Read), (4, operation)].into();
    result.points = result.points.map(|_, point| {
        let mut point = point.clone();
        let owners = if operation == Op::Take { &[4][..] } else { &[] };
        let mut actions = Vec::new();
        if operation == Op::Edit {
            actions.push(action(
                owners,
                vec![],
                Operation::Binary {
                    destination: Place::root(4),
                    operation: crate::CompileTimeBinaryOperation::Add,
                    left: Place::root(4),
                    right: Place::root(0),
                },
                vec![],
            ));
        }
        let operation = if operation == Op::Edit {
            Operation::Copy {
                source: Place::root(4),
                destination: Place::root(2),
            }
        } else {
            Operation::Binary {
                destination: Place::root(2),
                operation: crate::CompileTimeBinaryOperation::Add,
                left: Place::root(4),
                right: Place::root(0),
            }
        };
        let mut after = vec![Effect::Construct(2)];
        if !owners.is_empty() {
            after.push(Effect::Dispose(Place::root(4)));
        }
        actions.push(action(owners, vec![], operation, after));
        point.actions = actions.into();
        point
    });
    result
}

fn captured_choice(tag: u64) -> Action {
    action(
        &[],
        vec![],
        Operation::Literal {
            destination: 0,
            value: Value::callable(
                tag,
                Value::aggregate(vec![Value::integer(i128::from(tag) + 1)]),
            ),
        },
        vec![Effect::Construct(0)],
    )
}

fn explicit_program(operation: Op, tag: u64) -> (Program, AccessCodeId) {
    let (root, _) = code_ids();
    let first = ValueClosure {
        owner: root.into(),
        slot: 0,
    };
    let second = ValueClosure {
        owner: root.into(),
        slot: 1,
    };
    let targets: [CodeInstance; 2] =
        [first, second].map(|closure| nocter_model::ValueCode::Closure(closure).into());
    let before = if operation == Op::Take {
        vec![Effect::Invoke {
            source: Place::root(2),
            inputs: vec![(0, Place::root(2))].into(),
        }]
    } else {
        vec![]
    };
    let mut after = vec![Effect::Construct(1), Effect::Dispose(Place::root(0))];
    if operation != Op::Take {
        after.push(Effect::Dispose(Place::root(2)));
    }
    let input = match operation {
        Op::Read => Input::Read(Place::root(2)),
        Op::Edit => Input::Edit(Place::root(2)),
        Op::Take => Input::Take(0),
    };
    let caller = plan(
        &[
            (3, Storage::Owned),
            (0, Storage::Owned),
            (0, Storage::Owned),
        ],
        None,
        0,
        1,
        None,
        vec![
            captured_choice(tag),
            action(
                &[0],
                vec![],
                Operation::Literal {
                    destination: 2,
                    value: Value::integer(40),
                },
                vec![Effect::Construct(2)],
            ),
            action(
                &[0, 2],
                before,
                Operation::SelectedCall {
                    destination: 1,
                    callee: Place::root(0),
                    cases: vec![
                        CallCase {
                            data: 1,
                            target: targets[0],
                        },
                        CallCase {
                            data: 2,
                            target: targets[1],
                        },
                    ]
                    .into(),
                    inputs: vec![input].into(),
                },
                after,
            ),
        ],
    );
    let program = Program::new(
        vec![
            Type::Integer(nocter_model::BuiltinType::I32),
            Type::CapturedFunction {
                code: first,
                environment: 0,
            },
            Type::CapturedFunction {
                code: second,
                environment: 0,
            },
            Type::CallableAlternatives(vec![1, 2].into()),
        ],
        BTreeMap::from([
            (root.into(), caller),
            (targets[0], explicit_body(operation)),
            (targets[1], explicit_body(operation)),
        ]),
        BTreeMap::new(),
    )
    .unwrap();
    (program, root)
}

#[test]
fn selected_explicit_channels_share_read_edit_and_protected_take_handoffs() {
    for operation in [Op::Read, Op::Edit, Op::Take] {
        for tag in [0, 1] {
            let (program, root) = explicit_program(operation, tag);
            let mut budget = Budget::new(Limits::default());
            let actual = Executor::new(&program, &mut budget, 0)
                .evaluate_initializer(root)
                .unwrap();
            assert_eq!(actual, Value::integer(i128::from(tag) + 41));
            drop(actual);
            assert_eq!(budget.storage_used(), 0);
        }
    }
    let (mut missing, root) = explicit_program(Op::Take, 0);
    let caller = Arc::make_mut(missing.plans.get_mut(&root.into()).unwrap());
    caller.points = caller.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[2].alternatives[0].before = Box::new([]);
        point
    });
    assert!(validation::validate(&missing).is_err());
}

#[test]
fn inactive_candidate_bodies_need_no_executable_plan() {
    let (mut program, root) = program(1);
    let inactive = *program
        .contracts
        .keys()
        .find(|code| {
            code.source != root.into()
                && matches!(code.source, nocter_model::ValueCode::Declaration(_))
        })
        .unwrap();
    program.plans.remove(&inactive);
    validation::validate(&program).unwrap();
    let mut budget = Budget::new(Limits::default());
    let result = Executor::new(&program, &mut budget, 0)
        .evaluate_initializer(root)
        .unwrap();
    assert_eq!(result, Value::integer(41));
    drop(result);
    assert_eq!(budget.storage_used(), 0);
}

#[test]
fn selected_environment_backing_and_protected_inputs_release_partial_failures() {
    let (program, root) = explicit_program(Op::Take, 1);
    for work in [false, true] {
        let (mut failed, mut succeeded) = (false, false);
        for limit in 1..128 {
            let limits = if work {
                Limits::new(
                    CompileTimeEvaluationLimits::new(
                        NonZeroU64::new(limit).unwrap(),
                        NonZeroU32::new(64).unwrap(),
                    ),
                    NonZeroU64::new(256).unwrap(),
                )
            } else {
                Limits::new(
                    CompileTimeEvaluationLimits::default(),
                    NonZeroU64::new(limit).unwrap(),
                )
            };
            let mut budget = Budget::new(limits);
            match Executor::new(&program, &mut budget, 0).evaluate_initializer(root) {
                Ok(value) => {
                    assert_eq!(value, Value::integer(42));
                    drop(value);
                    succeeded = true;
                }
                Err(error) => {
                    assert_eq!(
                        error.rule,
                        if work {
                            Rule::StepLimit
                        } else {
                            Rule::StorageLimit
                        }
                    );
                    failed = true;
                }
            }
            assert_eq!(budget.storage_used(), 0);
        }
        assert!(failed && succeeded);
    }
}
