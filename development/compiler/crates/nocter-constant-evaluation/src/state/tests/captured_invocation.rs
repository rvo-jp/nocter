//! Taking invocation views reuse one body's ordered inputs and frame-local adapter storage.
use super::*;
use nocter_model::{ValueClosure, ValueInputOperation as Op};

fn action(
    owned: &[usize],
    before: Vec<Effect>,
    operation: Operation,
    after: Vec<Effect>,
) -> Action {
    Action {
        operation,
        alternatives: vec![Alternative {
            owned: owned
                .iter()
                .map(|&slot| (slot, Box::new([]) as Box<[Missing]>))
                .collect(),
            before: before.into(),
            after: after.into(),
        }]
        .into(),
    }
}
fn plan(
    slots: &[(usize, Storage)],
    input: Option<Op>,
    result: usize,
    returned: usize,
    backing: Option<usize>,
    actions: Vec<Action>,
) -> Plan {
    let mut points = ArenaBuilder::new();
    let entry = points.insert(Point {
        actions: actions.into(),
        terminator: Terminator::Return(Place::root(returned)),
    });
    Plan {
        slots: slots
            .iter()
            .map(|&(data, storage)| Slot { data, storage })
            .collect::<Vec<_>>()
            .into(),
        inputs: input.map(|operation| (0, operation)).into_iter().collect(),
        environment_backing: backing,
        statics: Box::new([]),
        result,
        initializer: input.is_none(),
        entry,
        points: points.finish(),
    }
}
fn body(operation: Op) -> Plan {
    let owned = if operation == Op::Take { &[0][..] } else { &[] };
    let mut actions = Vec::new();
    if operation != Op::Read {
        actions.push(action(
            owned,
            vec![],
            Operation::Literal {
                destination: 1,
                value: Value::integer(1),
            },
            vec![],
        ));
        actions.push(action(
            owned,
            vec![],
            Operation::Binary {
                destination: Place::root(0),
                operation: crate::CompileTimeBinaryOperation::Add,
                left: Place::root(0),
                right: Place::root(1),
            },
            vec![],
        ));
    }
    actions.push(if operation == Op::Take {
        action(
            owned,
            vec![],
            Operation::None,
            vec![Effect::Move {
                source: Place::root(0),
                destination: Place::root(2),
            }],
        )
    } else {
        action(
            owned,
            vec![],
            Operation::Copy {
                source: Place::root(0),
                destination: Place::root(2),
            },
            vec![Effect::Construct(2)],
        )
    });
    plan(
        &[
            (
                0,
                if operation == Op::Take {
                    Storage::Owned
                } else {
                    Storage::Alias
                },
            ),
            (0, Storage::Auxiliary),
            (0, Storage::Owned),
            (0, Storage::Auxiliary),
        ],
        Some(operation),
        0,
        2,
        (operation != Op::Take).then_some(3),
        actions,
    )
}
fn construct_function() -> Action {
    action(
        &[],
        vec![],
        Operation::Literal {
            destination: 0,
            value: Value::integer(40),
        },
        vec![
            Effect::Construct(0),
            Effect::Assemble {
                destination: 1,
                fields: Box::new([(0, Place::root(0))]),
            },
        ],
    )
}
fn caller(target: nocter_model::ValueCode, offered: Op) -> Plan {
    let mut actions = vec![construct_function()];
    let mut owners = vec![1];
    for destination in [3, 4] {
        let source = if offered == Op::Take {
            actions.push(action(
                &owners,
                vec![],
                Operation::Copy {
                    source: Place::root(1),
                    destination: Place::root(2),
                },
                vec![Effect::Construct(2)],
            ));
            owners.push(2);
            owners.sort_unstable();
            2
        } else {
            1
        };
        let before = if offered == Op::Take {
            vec![Effect::Invoke(Place::root(source))]
        } else {
            vec![]
        };
        actions.push(action(
            &owners,
            before,
            Operation::Call {
                destination,
                target,
                inputs: vec![Input {
                    operation: offered,
                    place: Place {
                        slot: source,
                        fields: vec![0].into(),
                    },
                }]
                .into(),
            },
            vec![Effect::Construct(destination)],
        ));
        owners.retain(|&slot| slot != 2);
        owners.push(destination);
    }
    actions.push(action(
        &owners,
        vec![],
        Operation::Copy {
            destination: Place::root(5),
            source: Place {
                slot: 1,
                fields: vec![0].into(),
            },
        },
        vec![Effect::Construct(5)],
    ));
    owners.push(5);
    actions.push(action(
        &owners,
        vec![],
        Operation::None,
        vec![
            Effect::Assemble {
                destination: 6,
                fields: Box::new([
                    (0, Place::root(3)),
                    (1, Place::root(4)),
                    (2, Place::root(5)),
                ]),
            },
            Effect::Dispose(Place::root(1)),
        ],
    ));
    plan(
        &[
            (0, Storage::Owned),
            (1, Storage::Owned),
            (1, Storage::Owned),
            (0, Storage::Owned),
            (0, Storage::Owned),
            (0, Storage::Owned),
            (2, Storage::Owned),
        ],
        None,
        2,
        6,
        None,
        actions,
    )
}
fn program(operation: Op, offered: Op) -> Result<Program, Rule> {
    let (owner, _) = code_ids();
    let code = ValueClosure {
        owner: owner.into(),
        slot: 0,
    };
    Program::new(
        vec![
            Type::I32,
            Type::CapturedFunction {
                code,
                environment: 0,
            },
            Type::Record(vec![0, 0, 0].into()),
        ],
        BTreeMap::from([
            (
                owner.into(),
                caller(nocter_model::ValueCode::Closure(code), offered),
            ),
            (nocter_model::ValueCode::Closure(code), body(operation)),
        ]),
        BTreeMap::new(),
    )
}
#[test]
fn read_edit_and_consuming_bodies_keep_their_own_contract_and_selected_environment() {
    let (root, _) = code_ids();
    for (operation, offered, expected) in [
        (Op::Read, Op::Read, [40, 40, 40]),
        (Op::Read, Op::Edit, [40, 40, 40]),
        (Op::Edit, Op::Edit, [41, 42, 42]),
        (Op::Read, Op::Take, [40, 40, 40]),
        (Op::Edit, Op::Take, [41, 41, 40]),
        (Op::Take, Op::Take, [41, 41, 40]),
    ] {
        let program = program(operation, offered).unwrap();
        let mut budget = Budget::new(CompileTimeEvaluationLimits::new(
            NonZeroU64::new(1000).unwrap(),
            NonZeroU32::new(2).unwrap(),
        ));
        let value = Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(root)
            .unwrap();
        assert_eq!(
            value,
            Value::aggregate(expected.map(Value::integer).to_vec()),
            "{operation:?}/{offered:?}"
        );
        drop(value);
        assert_eq!(budget.storage_used(), 0);
    }
}
#[test]
fn weaker_handoffs_and_invalid_adapter_backing_reject_before_execution() {
    for (operation, offered) in [
        (Op::Edit, Op::Read),
        (Op::Take, Op::Read),
        (Op::Take, Op::Edit),
    ] {
        assert!(matches!(
            program(operation, offered),
            Err(Rule::InvalidPlan)
        ));
    }
    let (root, _) = code_ids();
    let code = nocter_model::ValueCode::Closure(ValueClosure {
        owner: root.into(),
        slot: 0,
    });
    for backing in [None, Some(0), Some(1), Some(2), Some(99)] {
        let mut valid = program(Op::Edit, Op::Take).unwrap();
        valid.plans.get_mut(&code).unwrap().environment_backing = backing;
        assert!(matches!(
            Program::new(valid.types, valid.plans, valid.statics),
            Err(Rule::InvalidPlan)
        ));
    }
}
#[test]
fn taking_adapter_storage_failures_release_received_values_and_frames() {
    let (root, _) = code_ids();
    let program = program(Op::Edit, Op::Take).unwrap();
    let mut succeeded = false;
    for limit in 1..=256 {
        let mut budget = Budget::new(Limits::new(
            CompileTimeEvaluationLimits::default(),
            NonZeroU64::new(limit).unwrap(),
        ));
        match Executor::new(&program, &mut budget, 0).evaluate_initializer(root) {
            Err(error) => assert_eq!(error.rule, Rule::StorageLimit),
            Ok(value) => {
                drop(value);
                succeeded = true;
            }
        }
        assert_eq!(budget.storage_used(), 0, "limit {limit}");
        if succeeded {
            break;
        }
    }
    assert!(
        succeeded,
        "this fixed two-frame graph fits below 256 logical items"
    );
}
