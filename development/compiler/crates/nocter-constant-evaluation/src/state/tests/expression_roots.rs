use super::*;
use nocter_model::{ValueCodeRoot, ValueExpression};

fn number(value: impl Into<i128>) -> Plan {
    let value = value.into();
    let mut plan = literal();
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

#[test]
fn expression_value_domain_comes_from_the_validated_plan_not_its_code_identity() {
    let (owner, _) = code_ids();
    let root = ValueCodeRoot::Expression(ValueExpression { owner, slot: 0 });
    assert!(
        Program::new(
            [Type::Integer(nocter_model::BuiltinType::I32)],
            BTreeMap::from([(root.code(), number(u64::MAX))]),
            BTreeMap::new()
        )
        .is_err()
    );
    let program = Program::new(
        [Type::Integer(nocter_model::BuiltinType::Usize)],
        BTreeMap::from([(root.code(), number(u64::MAX))]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(CompileTimeEvaluationLimits::default());
    assert_eq!(
        Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(root)
            .unwrap(),
        Value::integer(u64::MAX)
    );
}

fn call(target: AccessCodeId) -> Plan {
    let mut plan = literal();
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Call {
            destination: 0,
            target: target.into(),
            inputs: Box::new([]),
        };
        point
    });
    plan
}

#[test]
fn expression_roots_execute_independently_of_their_source_owner_and_siblings() {
    let (owner, _) = code_ids();
    let first = ValueCodeRoot::Expression(ValueExpression { owner, slot: 0 });
    let second = ValueCodeRoot::Expression(ValueExpression { owner, slot: 1 });
    let program = Program::new(
        [Type::Integer(nocter_model::BuiltinType::I32)],
        BTreeMap::from([
            (owner.into(), number(3)),
            (first.code(), number(42)),
            (second.code(), number(7)),
        ]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(CompileTimeEvaluationLimits::default());
    let mut executor = Executor::new(&program, &mut budget, 0);
    for (root, expected) in [(first, 42), (second, 7), (owner.into(), 3)] {
        assert_eq!(
            executor.evaluate_initializer(root).unwrap(),
            Value::integer(expected)
        );
    }
    drop(executor);
    assert_eq!(budget.storage_used(), 0);
    let without_parent = Program::new(
        [Type::Integer(nocter_model::BuiltinType::I32)],
        BTreeMap::from([(first.code(), number(42))]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut executor = Executor::new(&without_parent, &mut budget, 0);
    assert_eq!(
        executor.evaluate_initializer(first).unwrap(),
        Value::integer(42)
    );
    assert_eq!(
        executor.evaluate_initializer(owner).unwrap_err().rule,
        Rule::InvalidPlan
    );
}

#[test]
fn expression_failure_retains_its_root_and_real_helper_call_sites_without_a_parent_plan() {
    let (owner, helper) = code_ids();
    let root = ValueCodeRoot::Expression(ValueExpression { owner, slot: 7 });
    let mut points = ArenaBuilder::new();
    let entry = points.insert(Point {
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
        terminator: Terminator::Unreachable,
    });
    let helper_plan = Plan {
        slots: Vec::new().into(),
        inputs: Box::new([]),
        statics: Box::new([]),
        environment_backing: None,
        result: 0,
        initializer: false,
        entry,
        points: points.finish().map(|_, point| Point {
            terminator: Terminator::Jump(entry),
            ..point.clone()
        }),
    };
    let program = Program::new(
        [Type::Integer(nocter_model::BuiltinType::I32)],
        BTreeMap::from([(root.code(), call(helper)), (helper.into(), helper_plan)]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(CompileTimeEvaluationLimits::new(
        NonZeroU64::new(20).unwrap(),
        NonZeroU32::new(8).unwrap(),
    ));
    let error = Executor::new(&program, &mut budget, 0)
        .evaluate_initializer(root)
        .unwrap_err();
    assert_eq!(error.rule, Rule::StepLimit);
    assert_eq!(error.code, root);
    assert_eq!(error.site.unwrap().code, helper.into());
    assert_eq!(error.trace.len(), 1);
    assert_eq!(error.trace[0].code, root.code());
    assert_eq!(budget.storage_used(), 0);
}

#[test]
fn expression_roots_are_required_initializers_and_never_declared_call_targets() {
    let (owner, _) = code_ids();
    let root = ValueCodeRoot::Expression(ValueExpression { owner, slot: 0 });
    let mut callable = number(42);
    callable.initializer = false;
    assert!(matches!(
        Program::new(
            [Type::Integer(nocter_model::BuiltinType::I32)],
            BTreeMap::from([(root.code(), callable)]),
            BTreeMap::new()
        ),
        Err(Rule::InvalidPlan)
    ));
    let mut with_input = number(42);
    with_input.inputs = vec![(0, nocter_model::ValueInputOperation::Take)].into();
    assert!(matches!(
        Program::new(
            [Type::Integer(nocter_model::BuiltinType::I32)],
            BTreeMap::from([(root.code(), with_input)]),
            BTreeMap::new()
        ),
        Err(Rule::InvalidPlan)
    ));
    assert!(matches!(
        Program::new(
            [Type::Integer(nocter_model::BuiltinType::I32)],
            BTreeMap::from([(root.code(), call(owner))]),
            BTreeMap::new()
        ),
        Err(Rule::InvalidPlan)
    ));
}
