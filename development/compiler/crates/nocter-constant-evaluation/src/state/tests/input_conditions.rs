//! Input conditions read bound values, preserve original sites and refund failed frames.
use super::*;
use nocter_model::{BuiltinType, ValueInputOperation as Op};

fn guarded(callee: AccessCodeId, operation: Op) -> Plan {
    let mut plan = literal();
    let mut slots = plan.slots.to_vec();
    slots.push(Slot {
        data: 2,
        storage: if operation == Op::Take {
            Storage::Owned
        } else {
            Storage::Alias
        },
    });
    plan.slots = slots.into();
    plan.inputs = vec![(2, operation)].into();
    if operation == Op::Take {
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            point.actions[0].alternatives[0]
                .owned
                .insert(2, Box::new([]));
            let after = &mut point.actions[0].alternatives[0].after;
            *after = after
                .iter()
                .cloned()
                .chain([Effect::Dispose(Place::root(2))])
                .collect();
            point
        });
    }
    plan.initializer = false;
    plan.conditions = vec![InputCondition {
        input: 0,
        fields: Box::new([]),
        cause: Site {
            code: callee.into(),
            point: plan.entry,
            action: 0,
        },
    }]
    .into();
    plan
}
fn caller(callee: CodeInstance, present: bool, operation: Op) -> Plan {
    let mut plan = literal();
    let mut slots = plan.slots.to_vec();
    slots.push(Slot {
        data: 2,
        storage: Storage::Owned,
    });
    plan.slots = slots.into();
    plan.points = plan.points.map(|_, original| {
        let mut point = original.clone();
        let mut invoke = point.actions[0].clone();
        invoke.operation = Operation::Call {
            destination: 0,
            target: callee,
            inputs: vec![match operation {
                Op::Read => Input::Read(Place::root(2)),
                Op::Edit => Input::Edit(Place::root(2)),
                Op::Take => Input::Take(2),
            }]
            .into(),
        };
        invoke.alternatives[0].owned.insert(2, Box::new([]));
        if operation == Op::Take {
            invoke.alternatives[0].before = vec![Effect::Invoke {
                source: Place::root(2),
                inputs: vec![(0, Place::root(2))].into(),
            }]
            .into();
        } else {
            let after = &mut invoke.alternatives[0].after;
            *after = after
                .iter()
                .cloned()
                .chain([Effect::Dispose(Place::root(2))])
                .collect();
        }
        let prepare = Action {
            operation: Operation::Literal {
                destination: 2,
                value: Value::variant(present, present.then(Value::unit)),
            },
            alternatives: vec![Alternative {
                owned: BTreeMap::new(),
                before: Box::new([]),
                after: vec![Effect::Construct(2)].into(),
            }]
            .into(),
        };
        point.actions = vec![prepare, invoke].into();
        point
    });
    plan
}
fn program(present: bool, plan: Plan) -> Program {
    let (root, target) = code_ids();
    let frame = CodeInstance {
        source: target.into(),
        instance: 7,
    };
    Program::new(
        [
            Type::Integer(BuiltinType::I32),
            Type::NamedFunction(target),
            Type::Variant {
                primary: 1,
                alternate: None,
            },
        ],
        BTreeMap::from([
            (root.into(), caller(frame, present, plan.inputs[0].1)),
            (frame, plan),
        ]),
        BTreeMap::new(),
    )
    .unwrap()
}

fn compound_program(
    other: bool,
    present: bool,
    operation: Op,
    fields: &[usize],
) -> Result<Program, Rule> {
    let (root, target) = code_ids();
    let frame = CodeInstance {
        source: target.into(),
        instance: 7,
    };
    let mut body = guarded(target, operation);
    Arc::make_mut(&mut body.slots)[2].data = 4;
    body.conditions[0].fields = fields.into();
    let mut caller = caller(frame, present, operation);
    Arc::make_mut(&mut caller.slots)[2].data = 4;
    caller.points = caller.points.map(|_, point| {
        let mut point = point.clone();
        if operation == Op::Take {
            let Operation::Call { inputs, .. } = &mut point.actions[1].operation else {
                unreachable!()
            };
            inputs[0] = Input::Take(4);
        }
        point.actions[0].operation = Operation::Literal {
            destination: 2,
            value: Value::aggregate(vec![
                Value::integer(0),
                Value::aggregate(vec![
                    Value::variant(other, other.then(Value::unit)),
                    Value::variant(present, present.then(Value::unit)),
                    Value::integer(42),
                ]),
            ]),
        };
        point
    });
    Program::new(
        [
            Type::Integer(BuiltinType::I32),
            Type::NamedFunction(target),
            Type::Variant {
                primary: 1,
                alternate: None,
            },
            Type::Record(vec![2, 2, 0].into()),
            Type::Record(vec![0, 3].into()),
        ],
        BTreeMap::from([(root.into(), caller), (frame, body)]),
        BTreeMap::new(),
    )
}

fn nested_program(depth: usize, outer: bool, present: bool, operation: Op) -> Program {
    let (root, _) = code_ids();
    let base = compound_program(true, present, operation, &[1, 1]).unwrap();
    let mut types = base.types;
    let mut data = 4;
    for _ in 0..depth {
        types.push(Type::Variant {
            primary: data,
            alternate: None,
        });
        data = types.len() - 1;
    }
    let plans = base
        .plans
        .into_iter()
        .map(|(code, plan)| {
            let mut plan = (*plan).clone();
            Arc::make_mut(&mut plan.slots)[2].data = data;
            if code.source == root.into() {
                plan.points = plan.points.map(|_, point| {
                    let mut point = point.clone();
                    let Operation::Literal { value, .. } = &mut point.actions[0].operation else {
                        unreachable!()
                    };
                    if outer {
                        for _ in 0..depth {
                            *value = Value::variant(true, Some(value.clone()));
                        }
                    } else {
                        *value = Value::variant(false, None);
                    }
                    if operation == Op::Take {
                        let Operation::Call { inputs, .. } = &mut point.actions[1].operation else {
                            unreachable!()
                        };
                        inputs[0] = Input::Take(data);
                    }
                    point
                });
            } else {
                plan.conditions[0].fields = std::iter::repeat_n(0, depth).chain([1, 1]).collect();
            }
            (code, plan)
        })
        .collect();
    Program::new(types, plans, BTreeMap::new()).unwrap()
}

#[test]
fn nested_conditions_short_circuit_outer_absence_and_recheck_inner_tags_for_each_invocation() {
    let (root, target) = code_ids();
    for operation in [Op::Read, Op::Edit, Op::Take] {
        let outer_absent = nested_program(2, false, true, operation);
        let inner_absent = nested_program(2, true, false, operation);
        let present = nested_program(2, true, true, operation);
        let mut budget = Budget::new(Limits::default());
        for program in [&outer_absent, &inner_absent] {
            assert_eq!(
                Executor::new(program, &mut budget, 0)
                    .evaluate_initializer(root)
                    .unwrap(),
                Value::integer(42)
            );
        }
        let error = Executor::new(&present, &mut budget, 0)
            .evaluate_initializer(root)
            .unwrap_err();
        assert_eq!(error.rule, Rule::InputConditionFailed);
        assert_eq!(error.site.unwrap().code, target.into());
        assert_eq!(budget.storage_used(), 0);
        assert_eq!(
            Executor::new(&inner_absent, &mut budget, 0)
                .evaluate_initializer(root)
                .unwrap(),
            Value::integer(42)
        );
    }
}

#[test]
fn an_absent_outer_tag_does_not_spend_work_or_read_unavailable_deep_payloads() {
    let (root, target) = code_ids();
    let limits =
        CompileTimeEvaluationLimits::new(NonZeroU64::new(16).unwrap(), NonZeroU32::new(8).unwrap());
    let mut budget = Budget::new(limits);
    let absent = nested_program(128, false, true, Op::Read);
    assert_eq!(
        Executor::new(&absent, &mut budget, 0)
            .evaluate_initializer(root)
            .unwrap(),
        Value::integer(42)
    );
    assert_eq!(budget.storage_used(), 0);
    let present = nested_program(128, true, true, Op::Read);
    let error = Executor::new(&present, &mut budget, 0)
        .evaluate_initializer(root)
        .unwrap_err();
    assert_eq!(error.rule, Rule::StepLimit);
    assert_eq!(error.site.unwrap().code, target.into());
    assert_eq!(budget.storage_used(), 0);
}

#[test]
fn compound_conditions_select_exact_nested_fields_through_owned_and_alias_inputs() {
    let (root, target) = code_ids();
    for operation in [Op::Read, Op::Edit, Op::Take] {
        for other in [false, true] {
            let absent = compound_program(other, false, operation, &[1, 1])
                .unwrap_or_else(|failure| panic!("{operation:?}, sibling {other}: {failure:?}"));
            let present = compound_program(other, true, operation, &[1, 1]).unwrap();
            let mut budget = Budget::new(Limits::default());
            assert_eq!(
                Executor::new(&absent, &mut budget, 0)
                    .evaluate_initializer(root)
                    .unwrap(),
                Value::integer(42)
            );
            let error = Executor::new(&present, &mut budget, 0)
                .evaluate_initializer(root)
                .unwrap_err();
            assert_eq!(error.rule, Rule::InputConditionFailed);
            assert_eq!(error.instance.as_deref().unwrap().source, target.into());
            assert_eq!(budget.storage_used(), 0);
            assert_eq!(
                Executor::new(&absent, &mut budget, 0)
                    .evaluate_initializer(root)
                    .unwrap(),
                Value::integer(42)
            );
        }
    }
}

#[test]
fn compound_condition_validation_rejects_unknown_fields_scalar_targets_and_tag_selection() {
    for fields in [vec![2], vec![0], vec![1, 1, 0]] {
        assert!(matches!(
            compound_program(false, false, Op::Read, &fields),
            Err(Rule::InvalidPlan)
        ));
    }
    let (root, target) = code_ids();
    let program = compound_program(false, false, Op::Read, &[1, 1]).unwrap();
    let limits =
        CompileTimeEvaluationLimits::new(NonZeroU64::new(4).unwrap(), NonZeroU32::new(8).unwrap());
    let mut budget = Budget::new(limits);
    let error = Executor::new(&program, &mut budget, 0)
        .evaluate_initializer(root)
        .unwrap_err();
    assert_eq!(error.rule, Rule::StepLimit);
    assert_eq!(error.site.unwrap().code, target.into());
    assert_eq!(budget.storage_used(), 0);
}

#[test]
fn actual_absence_passes_and_presence_fails_before_the_original_body() {
    let (root, target) = code_ids();
    for operation in [Op::Read, Op::Edit, Op::Take] {
        let body = guarded(target, operation);
        let expected = body.conditions[0].cause;
        let absent = program(false, body.clone());
        let present = program(true, body);
        let mut budget = Budget::new(Limits::default());
        assert_eq!(
            Executor::new(&absent, &mut budget, 0)
                .evaluate_initializer(root)
                .unwrap(),
            Value::integer(42)
        );
        let error = Executor::new(&present, &mut budget, 0)
            .evaluate_initializer(root)
            .unwrap_err();
        assert_eq!(error.rule, Rule::InputConditionFailed);
        assert_eq!(error.site, Some(expected));
        assert_eq!(
            error.instance.as_deref(),
            Some(&CodeInstance {
                source: target.into(),
                instance: 7
            })
        );
        assert_eq!(error.trace.len(), 1);
        assert_eq!(budget.storage_used(), 0);
        assert_eq!(
            Executor::new(&absent, &mut budget, 0)
                .evaluate_initializer(root)
                .unwrap(),
            Value::integer(42)
        );
    }
}

#[test]
fn invalid_input_or_cause_cannot_publish_a_conditioned_plan() {
    let (root, target) = code_ids();
    for invalid in 0..4 {
        let mut body = guarded(target, Op::Read);
        match invalid {
            0 => body.conditions[0].input = 1,
            1 => body.conditions[0].cause.code = root.into(),
            2 => body.conditions[0].cause.action = usize::MAX,
            _ => Arc::make_mut(&mut body.slots)[2].data = 0,
        }
        assert!(matches!(
            Program::new(
                [
                    Type::Integer(BuiltinType::I32),
                    Type::NamedFunction(target),
                    Type::Variant {
                        primary: 1,
                        alternate: None
                    }
                ],
                BTreeMap::from([(target.into(), body)]),
                BTreeMap::new()
            ),
            Err(Rule::InvalidPlan)
        ));
    }
}

#[test]
fn conditions_spend_execution_work_and_preserve_original_traps_after_success() {
    let (root, target) = code_ids();
    let body = guarded(target, Op::Read);
    let expected = body.conditions[0].cause;
    let ordinary = program(false, body.clone());
    let limits =
        CompileTimeEvaluationLimits::new(NonZeroU64::new(3).unwrap(), NonZeroU32::new(8).unwrap());
    let mut limited = Budget::new(limits);
    let error = Executor::new(&ordinary, &mut limited, 0)
        .evaluate_initializer(root)
        .unwrap_err();
    assert_eq!(error.rule, Rule::StepLimit);
    assert_eq!(error.site, Some(expected));
    assert_eq!(error.instance.as_deref().unwrap().instance, 7);
    assert_eq!(limited.storage_used(), 0);
    let mut trapped = body;
    trapped.points = trapped.points.map(|_, _| Point {
        actions: vec![Action {
            operation: Operation::BoundsTrap,
            alternatives: vec![Alternative {
                owned: BTreeMap::new(),
                before: Box::new([]),
                after: Box::new([]),
            }]
            .into(),
        }]
        .into(),
        terminator: Terminator::Unreachable,
    });
    let trapped = program(false, trapped);
    let mut healthy = Budget::new(Limits::default());
    let error = Executor::new(&trapped, &mut healthy, 0)
        .evaluate_initializer(root)
        .unwrap_err();
    assert_eq!(error.rule, Rule::IndexOutOfBounds);
    assert_eq!(error.site, Some(expected));
    assert_eq!(error.instance.as_deref().unwrap().instance, 7);
    assert_eq!(healthy.storage_used(), 0);
    assert_eq!(
        Executor::new(&ordinary, &mut healthy, 0)
            .evaluate_initializer(root)
            .unwrap(),
        Value::integer(42)
    );
}
