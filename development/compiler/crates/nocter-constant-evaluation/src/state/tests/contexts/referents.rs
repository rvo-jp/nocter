//! Alias receipts preserve one actual owner and reject unrelated or retired backing.
use super::*;

fn fixture(split: bool) -> (AccessCodeId, Vec<Type>, Plan) {
    let (root, types, mut plan) = super::fixture(Selection::Preserved, split);
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        for action in &mut point.actions {
            for effect in &mut action.alternatives[0].after {
                if let Effect::ReplaceRoot {
                    source,
                    destination,
                    data,
                } = effect
                {
                    *effect = Effect::ReplaceReferent {
                        source: source.clone(),
                        destination: Place::root(2),
                        owner: Place::root(*destination),
                        owner_data: *data,
                        view_data: *data,
                    };
                }
            }
        }
        point
    });
    (root, types, plan)
}

#[test]
fn referent_receipts_update_the_matched_view_and_owner_across_edges() {
    for split in [false, true] {
        let (root, types, plan) = fixture(split);
        let program = Program::new(
            types,
            BTreeMap::from([(root.into(), plan)]),
            BTreeMap::new(),
        )
        .unwrap();
        let mut budget = Budget::new(Limits::default());
        let value = Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(root)
            .unwrap();
        assert_eq!(value, Value::integer(42));
        drop(value);
        assert_eq!(budget.storage_used(), 0);
    }
}

#[test]
fn referent_receipt_contexts_release_on_storage_failure_and_retry() {
    let (root, types, plan) = fixture(true);
    let program = Program::new(
        types,
        BTreeMap::from([(root.into(), plan)]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut succeeded = false;
    for units in 1..256 {
        let mut budget = Budget::new(Limits::new(
            CompileTimeEvaluationLimits::default(),
            NonZeroU64::new(units).unwrap(),
        ));
        for _ in 0..2 {
            match Executor::new(&program, &mut budget, 0).evaluate_initializer(root) {
                Ok(value) => {
                    assert_eq!(value, Value::integer(42));
                    drop(value);
                    succeeded = true;
                }
                Err(error) => assert_eq!(error.rule, Rule::StorageLimit),
            }
            assert_eq!(budget.storage_used(), 0, "storage {units}");
        }
        if succeeded {
            break;
        }
    }
    assert!(succeeded);
}

#[test]
fn referent_receipts_require_actual_suppliers_and_compatible_views() {
    for case in 0..4 {
        let (root, types, mut plan) = fixture(false);
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            for action in &mut point.actions {
                for effect in &mut action.alternatives[0].after {
                    if let Effect::ReplaceReferent {
                        owner_data,
                        view_data,
                        owner,
                        destination,
                        ..
                    } = effect
                    {
                        match case {
                            0 => *owner_data = 3,
                            1 => *view_data = 3,
                            2 => owner.slot = 2,
                            3 => destination.slot = 1,
                            _ => unreachable!(),
                        }
                    }
                }
            }
            point
        });
        assert!(
            matches!(
                Program::new(
                    types,
                    BTreeMap::from([(root.into(), plan)]),
                    BTreeMap::new()
                ),
                Err(Rule::InvalidPlan)
            ),
            "case {case}"
        );
    }
}

#[test]
fn referent_receipts_reject_unrelated_addresses_and_retired_backing_without_leaks() {
    for unrelated in [false, true] {
        let (root, types, mut plan) = fixture(false);
        if unrelated {
            let mut slots = plan.slots.to_vec();
            slots.push(Slot {
                data: 3,
                storage: Storage::Owned,
            });
            plan.slots = slots.into();
        }
        plan.points = plan.points.map(|_, point| {
            let mut point = point.clone();
            let mut actions = point.actions.to_vec();
            if unrelated {
                for action in &mut actions[1..] {
                    action.alternatives[0].owned.insert(4, Box::new([]));
                    if let Operation::Alias { source, .. } = &mut action.operation {
                        *source = Place::root(4);
                    }
                }
                actions.insert(
                    1,
                    action(
                        Operation::Literal {
                            destination: 4,
                            value: Value::aggregate(vec![Value::unit(), Value::integer(40)]),
                        },
                        &[0],
                        vec![Effect::Construct(4)],
                    ),
                );
            } else {
                actions[3].alternatives[0].before = Box::new([Effect::Dispose(Place {
                    slot: 0,
                    fields: vec![0].into(),
                })]);
            }
            point.actions = actions.into();
            point
        });
        let program = Program::new(
            types,
            BTreeMap::from([(root.into(), plan)]),
            BTreeMap::new(),
        )
        .unwrap();
        let mut budget = Budget::new(Limits::default());
        assert_eq!(
            Executor::new(&program, &mut budget, 0)
                .evaluate_initializer(root)
                .unwrap_err()
                .rule,
            Rule::InvalidPlan
        );
        assert_eq!(budget.storage_used(), 0);
        let (root, types, plan) = fixture(false);
        let healthy = Program::new(
            types,
            BTreeMap::from([(root.into(), plan)]),
            BTreeMap::new(),
        )
        .unwrap();
        let value = Executor::new(&healthy, &mut budget, 0)
            .evaluate_initializer(root)
            .unwrap();
        assert_eq!(value, Value::integer(42));
        drop(value);
        assert_eq!(budget.storage_used(), 0);
    }
}
