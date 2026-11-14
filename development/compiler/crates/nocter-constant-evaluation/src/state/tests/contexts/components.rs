//! A typed component receipt preserves sibling data and the single container owner.
use super::*;

fn field(slot: usize, index: usize) -> Place {
    Place {
        slot,
        fields: vec![index].into(),
    }
}

fn fixture(retire: bool, stale: bool) -> (AccessCodeId, Vec<Type>, Plan) {
    let (root, other) = code_ids();
    let types = vec![
        Type::Integer(nocter_model::BuiltinType::I32),
        Type::NamedFunction(root),
        Type::NamedFunction(other),
        Type::Record(Box::new([1, 0])),
        Type::Record(Box::new([2, 0])),
    ];
    let mut receipt = action(
        Operation::None,
        &[0, 1],
        vec![
            Effect::MoveRoot {
                source: Place::root(1),
                destination: field(0, 0),
                data: 4,
            },
            Effect::Context {
                place: field(0, 0),
                data: 2,
            },
        ],
    );
    if retire {
        receipt.alternatives[0].before = Box::new([Effect::Dispose(field(0, 0))]);
    }
    let actions = vec![
        action(
            Operation::Literal {
                destination: 0,
                value: Value::aggregate(vec![Value::unit(), Value::integer(42)]),
            },
            &[],
            vec![Effect::Construct(0)],
        ),
        action(
            Operation::Alias {
                destination: 2,
                source: if stale { Place::root(0) } else { field(0, 1) },
            },
            &[0],
            vec![],
        ),
        action(
            Operation::Literal {
                destination: 1,
                value: Value::unit(),
            },
            &[0],
            vec![Effect::Construct(1)],
        ),
        receipt,
        action(
            Operation::Copy {
                source: Place::root(2),
                destination: Place::root(3),
            },
            &[0],
            vec![Effect::Construct(3), Effect::Dispose(Place::root(0))],
        ),
    ];
    let mut points = ArenaBuilder::new();
    let entry = points.insert(Point {
        actions: actions.into(),
        terminator: Terminator::Return(Place::root(3)),
    });
    let output = if stale { 3 } else { 0 };
    let plan = Plan {
        slots: [
            (3, Storage::Owned),
            (2, Storage::Owned),
            (output, Storage::Alias),
            (output, Storage::Owned),
        ]
        .map(|(data, storage)| Slot { data, storage })
        .into(),
        inputs: Box::new([]),
        statics: Box::new([]),
        environment_backing: None,
        result: output,
        initializer: true,
        entry,
        points: points.finish(),
    };
    (root, types, plan)
}

#[test]
fn selected_receipts_preserve_siblings_and_reject_stale_parent_views() {
    for stale in [false, true] {
        let (root, types, plan) = fixture(true, stale);
        let program = Program::new(
            types,
            BTreeMap::from([(root.into(), plan)]),
            BTreeMap::new(),
        )
        .unwrap();
        let mut budget = Budget::new(Limits::default());
        let result = Executor::new(&program, &mut budget, 0).evaluate_initializer(root);
        if stale {
            assert!(matches!(
                result,
                Err(Error {
                    rule: Rule::InvalidPlan,
                    ..
                })
            ));
        } else {
            assert_eq!(result.unwrap(), Value::integer(42));
        }
        assert_eq!(budget.storage_used(), 0);
    }
}

#[test]
fn a_component_receipt_cannot_reinterpret_its_sibling_or_change_the_supplied_value_type() {
    for wrong_sibling in [false, true] {
        let (root, mut types, mut plan) = fixture(true, false);
        if wrong_sibling {
            types.push(Type::Bool);
            types.push(Type::Record(Box::new([2, 5])));
            plan.points = plan.points.map(|_, point| {
                let mut point = point.clone();
                let Effect::MoveRoot { data, .. } = &mut point.actions[3].alternatives[0].after[0]
                else {
                    unreachable!()
                };
                *data = 6;
                point
            });
        } else {
            std::sync::Arc::make_mut(&mut plan.slots)[1].data = 1;
        }
        assert!(matches!(
            Program::new(
                types,
                BTreeMap::from([(root.into(), plan)]),
                BTreeMap::new()
            ),
            Err(Rule::InvalidPlan)
        ));
    }
}

#[test]
fn a_partial_receipt_requires_a_retired_subplace_and_failure_leaves_no_live_storage() {
    for retire in [false, true] {
        let (root, types, plan) = fixture(retire, false);
        let program = Program::new(
            types,
            BTreeMap::from([(root.into(), plan)]),
            BTreeMap::new(),
        )
        .unwrap();
        let mut budget = Budget::new(Limits::default());
        let result = Executor::new(&program, &mut budget, 0).evaluate_initializer(root);
        if retire {
            assert_eq!(result.unwrap(), Value::integer(42));
        } else {
            assert!(matches!(
                result,
                Err(Error {
                    rule: Rule::InvalidPlan,
                    ..
                })
            ));
        }
        assert_eq!(budget.storage_used(), 0);
    }
}

#[test]
fn deeply_nested_receipt_compatibility_uses_no_recursive_host_stack() {
    let (root, other) = code_ids();
    let mut types = vec![
        Type::Integer(nocter_model::BuiltinType::I32),
        Type::NamedFunction(root),
        Type::NamedFunction(other),
    ];
    let (mut before, mut after) = (1, 2);
    let mut path = Vec::new();
    for _ in 0..10_000 {
        types.push(Type::Record(Box::new([before])));
        before = types.len() - 1;
        types.push(Type::Record(Box::new([after])));
        after = types.len() - 1;
        path.push(Projection::Field(0));
    }
    let program = Program::new(
        types,
        BTreeMap::from([(root.into(), literal())]),
        BTreeMap::new(),
    )
    .unwrap();
    super::super::super::validation::compatible_receipt(&program, before, after, &path).unwrap();
}
