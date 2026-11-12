//! Actual equal, ascending and descending saved numbers share the existing sparse ownership guard.
use super::cleanup::retained_partial_remainder;
use super::*;

fn guarded(mut action: Action, owned: &BTreeMap<usize, Box<[Missing]>>) -> Action {
    action.alternatives[0].owned = owned.clone();
    action
}

fn plan(length: u64, left: u64, right: u64) -> Plan {
    let mut plan = retained_partial_remainder(length, left);
    let mut slots = plan.slots.to_vec();
    slots.extend([
        Slot {
            data: 1,
            storage: Storage::Owned,
        },
        Slot {
            data: 0,
            storage: Storage::Owned,
        },
    ]);
    plan.slots = slots.into();
    let second = Place {
        slot: 0,
        fields: vec![
            Projection::Index {
                slot: 9,
                bound: length,
            },
            Projection::Field(1),
        ]
        .into(),
    };
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        let first = point.actions[5].alternatives[0].owned.clone();
        let mut captured = first.clone();
        captured.insert(9, Box::new([]));
        let mut both = captured.clone();
        both.insert(10, Box::new([]));
        let mut holes = both[&0].to_vec();
        holes.push(second.fields.clone().into());
        both.insert(0, holes.into());
        let mut actions = point.actions[..5].to_vec();
        actions.extend([
            guarded(action(integer(1, right), &[], vec![]), &first),
            guarded(action(index(2), &[], vec![]), &first),
            guarded(action(copy(9, 1), &[], vec![Effect::Construct(9)]), &first),
            guarded(
                action(
                    Operation::None,
                    &[],
                    vec![Effect::Move {
                        source: second.clone(),
                        destination: Place::root(10),
                    }],
                ),
                &captured,
            ),
            guarded(action(integer(1, u64::MAX), &[], vec![]), &both),
            guarded(
                action(
                    integer(4, 42),
                    &[],
                    vec![
                        Effect::DisposeArrayRemainder(Place::root(0)),
                        Effect::Dispose(Place::root(7)),
                        Effect::Dispose(Place::root(9)),
                        Effect::Dispose(Place::root(8)),
                        Effect::Dispose(Place::root(10)),
                        Effect::Construct(4),
                    ],
                ),
                &both,
            ),
        ]);
        point.actions = actions.into();
        point
    });
    plan
}

fn types(length: u64) -> Vec<Type> {
    vec![
        Type::Integer(nocter_model::BuiltinType::I32),
        Type::Integer(nocter_model::BuiltinType::Usize),
        Type::Record(vec![0, 0].into()),
        Type::Array { element: 2, length },
    ]
}

#[test]
fn multiple_holes_keep_bounded_storage_and_recover_after_exhaustion() {
    let (code, _) = code_ids();
    let mut minima = Vec::new();
    for length in [2, u64::MAX] {
        for (left, right) in [
            (0, 0),
            (0, length - 1),
            (length - 1, 0),
            (length - 1, length - 1),
        ] {
            let program = Program::new(
                types(length),
                BTreeMap::from([(code.into(), plan(length, left, right))]),
                BTreeMap::new(),
            )
            .unwrap();
            let mut first = None;
            for limit in 1..=192 {
                let mut budget = Budget::new(Limits::new(
                    CompileTimeEvaluationLimits::default(),
                    NonZeroU64::new(limit).unwrap(),
                ));
                for _ in 0..2 {
                    match Executor::new(&program, &mut budget, 0).evaluate_initializer(code) {
                        Ok(answer) => {
                            assert_eq!(answer, Value::integer(42));
                            first.get_or_insert(limit);
                        }
                        Err(error) => assert_eq!(error.rule, Rule::StorageLimit),
                    }
                    assert_eq!(budget.storage_used(), 0);
                }
            }
            minima.push(first.unwrap());
        }
    }
    assert_eq!(minima[..4], minima[4..]);
}

#[test]
fn remainder_requires_an_owned_array_and_cannot_retire_it_twice() {
    let (code, _) = code_ids();
    let mut wrong = plan(2, 0, 1);
    wrong.points = wrong.points.map(|_, point| {
        let mut point = point.clone();
        point.actions.last_mut().unwrap().alternatives[0].after[0] =
            Effect::DisposeArrayRemainder(Place::root(8));
        point
    });
    assert!(matches!(
        Program::new(
            types(2),
            BTreeMap::from([(code.into(), wrong)]),
            BTreeMap::new()
        ),
        Err(Rule::InvalidPlan)
    ));
    let mut double = plan(2, 0, 1);
    double.points = double.points.map(|_, point| {
        let mut point = point.clone();
        let effects = &mut point.actions.last_mut().unwrap().alternatives[0].after;
        let mut twice = effects.to_vec();
        twice.insert(1, twice[0].clone());
        *effects = twice.into();
        point
    });
    let program = Program::new(
        types(2),
        BTreeMap::from([(code.into(), double)]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(CompileTimeEvaluationLimits::default());
    for _ in 0..2 {
        assert_eq!(
            Executor::new(&program, &mut budget, 0)
                .evaluate_initializer(code)
                .unwrap_err()
                .rule,
            Rule::InvalidPlan
        );
        assert_eq!(budget.storage_used(), 0);
    }
}
