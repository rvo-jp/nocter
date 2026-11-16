//! Independent lexical code keys survive dispatch, failure sites and recovery.
use super::*;
use nocter_model::{ValueClosure, ValueCode, ValueCodeRoot, ValueExpression};

fn closure(owner: ValueCodeRoot, slot: u32) -> ValueCode {
    ValueCode::Closure(ValueClosure { owner, slot })
}
fn number(value: i32, initializer: bool) -> Plan {
    let mut plan = literal();
    plan.initializer = initializer;
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Literal {
            destination: 0,
            value: Value::integer(value),
        };
        point
    });
    plan
}
fn call(target: CodeInstance, initializer: bool) -> Plan {
    let mut plan = number(0, initializer);
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Call {
            destination: 0,
            target,
            inputs: Box::new([]),
        };
        point
    });
    plan
}
#[test]
fn lexical_plans_do_not_alias_their_owner_or_siblings() {
    let (owner, _) = code_ids();
    let first = ValueCodeRoot::Expression(ValueExpression { owner, slot: 0 });
    let second = ValueCodeRoot::Expression(ValueExpression { owner, slot: 1 });
    let alpha = closure(first, 0);
    let beta = closure(owner.into(), 1);
    let program = Program::new(
        [Type::Integer(nocter_model::BuiltinType::I32)],
        BTreeMap::from([
            (owner.into(), number(3, true)),
            (first.code().into(), call(alpha.into(), true)),
            (second.code().into(), call(beta.into(), true)),
            (alpha.into(), number(42, false)),
            (beta.into(), number(7, false)),
        ]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(Limits::default());
    for (root, expected) in [(first, 42), (second, 7), (owner.into(), 3)] {
        assert_eq!(
            Executor::new(&program, &mut budget, 0)
                .evaluate_initializer(root)
                .unwrap(),
            Value::integer(expected)
        );
        assert_eq!(budget.storage_used(), 0);
    }
}
#[test]
fn lexical_failure_sites_keep_the_real_call_path_and_refund_failed_frames() {
    let (owner, _) = code_ids();
    let alpha = closure(owner.into(), 0);
    let beta = closure(owner.into(), 1);
    let recovery = ValueCodeRoot::Expression(ValueExpression { owner, slot: 0 });
    let mut forever = number(0, false);
    let entry = forever.entry;
    forever.points = forever.points.map(|_, _| Point {
        actions: vec![Action {
            operation: Operation::None,
            alternatives: vec![Alternative {
                owned: BTreeMap::new(),
                before: Box::new([]),
                after: Box::new([]),
            }]
            .into(),
        }]
        .into(),
        terminator: Terminator::Jump(entry),
    });
    let program = Program::new(
        [Type::Integer(nocter_model::BuiltinType::I32)],
        BTreeMap::from([
            (owner.into(), call(alpha.into(), true)),
            (alpha.into(), call(beta.into(), false)),
            (beta.into(), forever),
            (recovery.code().into(), number(42, true)),
        ]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(CompileTimeEvaluationLimits::new(
        NonZeroU64::new(20).unwrap(),
        NonZeroU32::new(8).unwrap(),
    ));
    let error = Executor::new(&program, &mut budget, 0)
        .evaluate_initializer(owner)
        .unwrap_err();
    assert_eq!(error.rule, Rule::StepLimit);
    assert_eq!(error.code, owner.into());
    assert_eq!(error.site.unwrap().code, beta);
    assert_eq!(
        error.trace.iter().map(|site| site.code).collect::<Vec<_>>(),
        [alpha, owner.into()]
    );
    assert_eq!(budget.storage_used(), 0);
    budget.restart_work();
    assert_eq!(
        Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(recovery)
            .unwrap(),
        Value::integer(42)
    );
    assert_eq!(budget.storage_used(), 0);
}
#[test]
fn lexical_calls_cannot_fall_back_to_roots_siblings_or_initializers() {
    let (owner, _) = code_ids();
    let alpha = closure(owner.into(), 0);
    let beta = closure(owner.into(), 1);
    let expression = ValueCodeRoot::Expression(ValueExpression { owner, slot: 0 });
    for plans in [
        BTreeMap::from([(alpha.into(), number(42, true))]),
        BTreeMap::from([
            (owner.into(), call(beta.into(), true)),
            (alpha.into(), number(42, false)),
        ]),
        BTreeMap::from([
            (owner.into(), call(expression.code().into(), true)),
            (expression.code().into(), number(42, true)),
        ]),
    ] {
        assert!(matches!(
            Program::new(
                [Type::Integer(nocter_model::BuiltinType::I32)],
                plans,
                BTreeMap::new()
            ),
            Err(Rule::InvalidPlan)
        ));
    }
}

#[test]
fn selected_instances_of_one_source_do_not_alias_or_fall_back() {
    let (owner, _) = code_ids();
    let source = closure(owner.into(), 0);
    let alpha = CodeInstance {
        source,
        instance: 1,
    };
    let beta = CodeInstance {
        source,
        instance: 2,
    };
    let second = ValueCodeRoot::Expression(ValueExpression { owner, slot: 0 });
    let program = Program::new(
        [Type::Integer(nocter_model::BuiltinType::I32)],
        BTreeMap::from([
            (owner.into(), call(alpha, true)),
            (second.code().into(), call(beta, true)),
            (source.into(), number(3, false)),
            (alpha, number(42, false)),
            (beta, number(7, false)),
        ]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(Limits::default());
    for (root, expected) in [(owner.into(), 42), (second, 7)] {
        assert_eq!(
            Executor::new(&program, &mut budget, 0)
                .evaluate_initializer(root)
                .unwrap(),
            Value::integer(expected)
        );
        assert_eq!(budget.storage_used(), 0);
    }
    let mut missing = program;
    missing.plans.remove(&beta);
    assert!(matches!(
        Program::new(
            missing.types,
            missing
                .plans
                .into_iter()
                .map(|(key, plan)| (key, (*plan).clone()))
                .collect(),
            missing.statics
        ),
        Err(Rule::InvalidPlan)
    ));
}

#[test]
fn recursive_instance_failures_keep_semantic_sites_and_refund_frames() {
    let (owner, helper) = code_ids();
    let source = helper.into();
    let first = CodeInstance {
        source,
        instance: 1,
    };
    let recursive = CodeInstance {
        source,
        instance: 2,
    };
    let recovery = ValueCodeRoot::Expression(ValueExpression { owner, slot: 0 });
    let program = Program::new(
        [Type::Integer(nocter_model::BuiltinType::I32)],
        BTreeMap::from([
            (owner.into(), call(first, true)),
            (first, call(recursive, false)),
            (recursive, call(recursive, false)),
            (recovery.code().into(), number(42, true)),
        ]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(CompileTimeEvaluationLimits::new(
        NonZeroU64::new(100).unwrap(),
        NonZeroU32::new(4).unwrap(),
    ));
    let error = Executor::new(&program, &mut budget, 0)
        .evaluate_initializer(owner)
        .unwrap_err();
    assert_eq!(error.rule, Rule::CallDepthLimit);
    assert_eq!(error.code, owner.into());
    assert_eq!(error.site.unwrap().code, source);
    assert_eq!(
        error.trace.iter().map(|site| site.code).collect::<Vec<_>>(),
        [source, source, owner.into()]
    );
    assert_eq!(budget.storage_used(), 0);
    budget.restart_work();
    assert_eq!(
        Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(recovery)
            .unwrap(),
        Value::integer(42)
    );
    assert_eq!(budget.storage_used(), 0);
}

#[test]
fn deep_calls_resume_and_unwind_on_a_small_host_stack() {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let (owner, helper) = code_ids();
            let target = |instance| CodeInstance {
                source: helper.into(),
                instance,
            };
            let depth = 4096;
            let mut plans = BTreeMap::from([(owner.into(), call(target(1), true))]);
            for index in 1..depth {
                plans.insert(target(index), call(target(index + 1), false));
            }
            plans.insert(target(depth), number(42, false));
            let program = Program::new(
                [Type::Integer(nocter_model::BuiltinType::I32)],
                plans,
                BTreeMap::new(),
            )
            .unwrap();
            let work = CompileTimeEvaluationLimits::new(
                NonZeroU64::new(100_000).unwrap(),
                NonZeroU32::new(depth + 1).unwrap(),
            );
            let mut budget = Budget::new(work);
            let value = Executor::new(&program, &mut budget, 0)
                .evaluate_initializer(owner)
                .unwrap();
            assert_eq!(value, Value::integer(42));
            drop(value);
            assert_eq!(budget.storage_used(), 0);
            for suspended in [0, 17] {
                let mut budget = Budget::new(Limits::default());
                let error = Executor::new(&program, &mut budget, suspended)
                    .evaluate_initializer(owner)
                    .unwrap_err();
                assert_eq!(error.rule, Rule::CallDepthLimit);
                assert_eq!(
                    error.instance.unwrap().instance,
                    255 - u32::try_from(suspended).unwrap()
                );
                assert_eq!(error.trace.len(), 255 - suspended);
                assert_eq!(error.trace.last().unwrap().code, owner.into());
                assert_eq!(budget.storage_used(), 0);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
