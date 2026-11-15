//! Explicit exclusion is metadata, distinct from an unresolved representation and actual storage.
use super::*;

#[test]
fn an_empty_alternative_preserves_absence_without_admitting_any_payload() {
    let (code, _) = code_ids();
    let types = [
        Type::Uninhabited,
        Type::Variant {
            primary: 0,
            alternate: None,
        },
    ];
    let mut plan = literal();
    for slot in Arc::make_mut(&mut plan.slots) {
        slot.data = 1;
    }
    plan.result = 1;
    plan.points = plan.points.map(|_, original| {
        let mut point = original.clone();
        point.actions[0].operation = Operation::Literal {
            destination: 0,
            value: Value::variant(false, None),
        };
        point
    });
    let mut invalid_cleanup = plan.clone();
    invalid_cleanup.points = invalid_cleanup.points.map(|_, original| {
        let mut point = original.clone();
        let mut effects = point.actions[0].alternatives[0].after.to_vec();
        effects.push(Effect::Dispose(Place {
            slot: 1,
            fields: vec![Projection::Field(0)].into(),
        }));
        point.actions[0].alternatives[0].after = effects.into();
        point
    });
    assert!(matches!(
        Program::new(
            types.clone(),
            BTreeMap::from([(code.into(), invalid_cleanup)]),
            BTreeMap::new()
        ),
        Err(Rule::InvalidPlan)
    ));
    let program = Program::new(
        types,
        BTreeMap::from([(code.into(), plan)]),
        BTreeMap::new(),
    )
    .unwrap();
    assert_ne!(Type::Uninhabited, Type::Unavailable);
    for value in [
        Value::unit(),
        Value::boolean(false),
        Value::integer(0),
        Value::aggregate(Vec::new()),
    ] {
        assert!(!validation::value(&program, 0, &value));
        assert!(!validation::value(
            &program,
            1,
            &Value::variant(true, Some(value))
        ));
    }
    let mut budget = Budget::new(Limits::default());
    assert_eq!(
        Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(code)
            .unwrap(),
        Value::variant(false, None)
    );
    assert_eq!(budget.storage_used(), 0);
}

#[test]
fn empty_result_metadata_does_not_grant_slots_or_received_input_contracts() {
    let (code, _) = code_ids();
    let mut plan = literal();
    plan.slots = Vec::new().into();
    plan.points = plan.points.map(|_, _| Point {
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
    let program = Program::new(
        [Type::Uninhabited],
        BTreeMap::from([(code.into(), plan.clone())]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(Limits::default());
    assert_eq!(
        Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(code)
            .unwrap_err()
            .rule,
        Rule::IndexOutOfBounds
    );
    assert_eq!(budget.storage_used(), 0);
    for storage in [Storage::Owned, Storage::Alias, Storage::Auxiliary] {
        let mut invalid = plan.clone();
        invalid.slots = vec![Slot { data: 0, storage }].into();
        assert!(matches!(
            Program::new(
                [Type::Uninhabited],
                BTreeMap::from([(code.into(), invalid)]),
                BTreeMap::new()
            ),
            Err(Rule::InvalidPlan)
        ));
    }
    for operation in [
        nocter_model::ValueInputOperation::Read,
        nocter_model::ValueInputOperation::Edit,
        nocter_model::ValueInputOperation::Take,
    ] {
        let contract = CallContract {
            inputs: vec![InputContract { data: 0, operation }].into(),
            result: 0,
            initializer: false,
            environment_backing: false,
        };
        assert!(matches!(
            Program::linked(
                [Type::Uninhabited],
                BTreeMap::from([(code.into(), contract)]),
                BTreeMap::new(),
                BTreeMap::new()
            ),
            Err(Rule::InvalidPlan)
        ));
    }
}
