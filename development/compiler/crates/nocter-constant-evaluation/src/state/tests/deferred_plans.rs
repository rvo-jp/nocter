//! Deferred bodies retain fully linked and immutable invocation boundaries.
use super::*;

#[test]
fn deferred_call_boundaries_validate_before_bodies_and_extend_atomically() {
    let (root, target) = code_ids();
    let mut root_plan = literal();
    root_plan.points = root_plan.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Call {
            destination: 0,
            target: target.into(),
            inputs: Box::new([]),
        };
        point
    });
    let mut target_plan = literal();
    target_plan.initializer = false;
    let contracts = BTreeMap::from([
        (root.into(), CallContract::from_plan(&root_plan).unwrap()),
        (
            target.into(),
            CallContract::from_plan(&target_plan).unwrap(),
        ),
    ]);
    let mut program = Program::linked(
        [Type::Integer(nocter_model::BuiltinType::I32)],
        contracts,
        BTreeMap::from([(root.into(), root_plan)]),
        BTreeMap::new(),
    )
    .unwrap();
    assert!(!program.plans.contains_key(&target.into()));
    let original = program.plans[&root.into()].clone();
    let mut wrong = target_plan.clone();
    wrong.result = 1;
    wrong.slots = vec![Slot {
        data: 1,
        storage: Storage::Owned,
    }]
    .into();
    assert_eq!(
        program.extend(ProgramExtension {
            types: vec![Type::Bool],
            plans: BTreeMap::from([(target.into(), wrong)]),
            ..ProgramExtension::default()
        }),
        Err(Rule::InvalidPlan)
    );
    assert_eq!(program.types.len(), 1);
    assert!(!program.plans.contains_key(&target.into()));
    assert!(Arc::ptr_eq(&original, &program.plans[&root.into()]));
    assert_eq!(
        program.extend(ProgramExtension {
            contracts: BTreeMap::from([(
                target.into(),
                CallContract::from_plan(&target_plan).unwrap()
            )]),
            ..ProgramExtension::default()
        }),
        Err(Rule::InvalidPlan)
    );
    program
        .extend(ProgramExtension {
            plans: BTreeMap::from([(target.into(), target_plan)]),
            ..ProgramExtension::default()
        })
        .unwrap();
    assert!(Arc::ptr_eq(&original, &program.plans[&root.into()]));
    let mut budget = Budget::new(Limits::default());
    assert_eq!(
        Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(root)
            .unwrap(),
        Value::integer(42)
    );
    assert_eq!(budget.storage_used(), 0);
}
