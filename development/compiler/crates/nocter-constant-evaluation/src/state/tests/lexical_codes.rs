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
fn call(target: ValueCode, initializer: bool) -> Plan {
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
            (first.code(), call(alpha, true)),
            (second.code(), call(beta, true)),
            (alpha, number(42, false)),
            (beta, number(7, false)),
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
            (owner.into(), call(alpha, true)),
            (alpha, call(beta, false)),
            (beta, forever),
            (recovery.code(), number(42, true)),
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
        BTreeMap::from([(alpha, number(42, true))]),
        BTreeMap::from([(owner.into(), call(beta, true)), (alpha, number(42, false))]),
        BTreeMap::from([
            (owner.into(), call(expression.code(), true)),
            (expression.code(), number(42, true)),
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
