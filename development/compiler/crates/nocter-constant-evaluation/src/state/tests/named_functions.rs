//! A zero-environment witness still retains code identity across invocation-view adaptation.
use super::*;

#[test]
fn invocation_views_preserve_code_identity_and_one_owned_payload() {
    let (root, other) = code_ids();
    for (destination, valid) in [
        (Type::NamedFunction(root), true),
        (Type::NamedFunction(other), false),
        (Type::Unit, false),
    ] {
        let mut plan = literal();
        Arc::make_mut(&mut plan.slots)[1].data = 1;
        plan.result = 1;
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            point.actions[0].operation = Operation::Literal {
                destination: 0,
                value: Value::unit(),
            };
            point.actions[0].alternatives[0].after = vec![
                Effect::Construct(0),
                Effect::AdaptFunction {
                    source: Place::root(0),
                    destination: Place::root(1),
                },
            ]
            .into();
            point
        });
        let program = Program::new(
            [Type::NamedFunction(root), destination],
            BTreeMap::from([(root.into(), plan)]),
            BTreeMap::new(),
        );
        if valid {
            let program = program.unwrap();
            let mut budget = Budget::new(Limits::default());
            for _ in 0..3 {
                let value = Executor::new(&program, &mut budget, 0)
                    .evaluate_initializer(root)
                    .unwrap();
                assert_eq!(value.data(), &Data::Unit);
                assert!(budget.storage_used() > 0);
                drop(value);
                assert_eq!(budget.storage_used(), 0);
            }
        } else {
            assert!(matches!(program, Err(Rule::InvalidPlan)));
        }
    }
}
