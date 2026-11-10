//! Live working storage: sharing, bounded peaks, failed cleanup and copy-on-write.
use super::{budget::Storage as MeteredStorage, *};
use std::num::NonZeroU64;

fn budget(units: u64) -> Budget {
    Budget::new(Limits::new(
        crate::CompileTimeEvaluationLimits::default(),
        NonZeroU64::new(units).unwrap(),
    ))
}
fn integer(storage: &MeteredStorage, value: i32) -> Value {
    storage.primitive(Data::I32(value)).unwrap()
}
#[test]
fn returned_values_retain_one_charge_without_charging_handle_clones() {
    let mut budget = budget(16);
    let (code, _) = super::tests::code_ids();
    let program = Program::new(
        [Type::I32],
        BTreeMap::from([(code, super::tests::literal())]),
    )
    .unwrap();
    let result = Executor::new(&program, &mut budget, 0)
        .evaluate_initializer(code)
        .unwrap();
    let used = budget.storage_used();
    assert!(used > 0);
    let clones = vec![result.clone(); 100];
    assert_eq!(budget.storage_used(), used);
    budget.restart_work();
    assert_eq!(budget.storage_used(), used);
    drop(result);
    assert_eq!(budget.storage_used(), used);
    drop(clones);
    assert_eq!(budget.storage_used(), 0);
}
#[test]
fn retained_outputs_eventually_exhaust_storage_and_failed_frames_are_reusable() {
    let mut budget = budget(16);
    let (code, _) = super::tests::code_ids();
    let program = Program::new(
        [Type::I32],
        BTreeMap::from([(code, super::tests::literal())]),
    )
    .unwrap();
    let mut outputs = Vec::new();
    let mut executor = Executor::new(&program, &mut budget, 0);
    loop {
        match executor.evaluate_initializer(code) {
            Ok(value) => outputs.push(value),
            Err(error) => {
                assert_eq!(error.rule, Rule::StorageLimit);
                assert!(error.site.is_some());
                break;
            }
        }
    }
    assert!(!outputs.is_empty());
    assert_eq!(
        executor.evaluate_initializer(code).unwrap_err().rule,
        Rule::StorageLimit
    );
    drop(outputs);
    for _ in 0..100 {
        assert_eq!(
            executor.evaluate_initializer(code).unwrap(),
            Value::integer(42)
        );
    }
    drop(executor);
    assert_eq!(budget.storage_used(), 0);
}
#[test]
fn importing_foreign_data_preserves_sharing_and_cannot_bypass_the_new_budget() {
    let first = budget(100);
    let leaf = integer(&first.storage, 42);
    let shared = first
        .storage
        .record([leaf.clone(), leaf].into_iter())
        .unwrap();
    let foreign = first
        .storage
        .record([shared.clone(), shared].into_iter())
        .unwrap();
    assert_eq!(first.storage_used(), 7);
    let second = budget(100);
    let imported = second.storage.import(&foreign).unwrap();
    assert_eq!(imported, foreign);
    assert_eq!(second.storage_used(), 7);
    let insufficient = budget(3);
    assert_eq!(
        insufficient.storage.import(&foreign).unwrap_err(),
        Rule::StorageLimit
    );
    assert_eq!(insufficient.storage_used(), 0);
    drop(imported);
    assert_eq!(second.storage_used(), 0);
    assert_eq!(first.storage_used(), 7);
}
#[test]
fn updating_shared_data_charges_the_copied_path_and_preserves_the_snapshot() {
    use super::memory::{Cell, Frame, Memory};
    let budget = budget(100);
    let leaf = integer(&budget.storage, 1);
    let record = budget.storage.record([leaf].into_iter()).unwrap();
    let snapshot = record.clone();
    let mut memory = Memory::new(budget.storage.clone());
    let mut frame = Frame::new(
        &budget.storage,
        vec![Slot {
            data: 0,
            storage: Storage::Owned,
        }]
        .into(),
    )
    .unwrap();
    frame.set(&budget.storage, 0, Cell::Data(record)).unwrap();
    memory.frames.push(frame);
    let address = memory
        .address(
            0,
            &Place {
                slot: 0,
                fields: vec![0].into(),
            },
        )
        .unwrap();
    let replacement = integer(&budget.storage, 2);
    let before = budget.storage_used();
    memory.store(&address, replacement).unwrap();
    assert_eq!(budget.storage_used(), before + 2);
    assert_eq!(memory.read(&address).unwrap(), Value::integer(2));
    assert_eq!(snapshot, Value::record(vec![Value::integer(1)]));
    drop(snapshot);
    let after_snapshot = budget.storage_used();
    let replacement = integer(&budget.storage, 3);
    memory.store(&address, replacement).unwrap();
    assert_eq!(budget.storage_used(), after_snapshot);
    drop(address);
    drop(memory);
    assert_eq!(budget.storage_used(), 0);
}
#[test]
fn insufficient_space_rejects_shared_path_copy_before_mutation() {
    let budget = budget(4);
    let leaf = integer(&budget.storage, 1);
    let record = budget.storage.record([leaf].into_iter()).unwrap();
    let mut edited = record.clone();
    assert_eq!(
        edited.make_mut(&budget.storage).unwrap_err(),
        Rule::StorageLimit
    );
    assert_eq!(edited, record);
    assert_eq!(budget.storage_used(), 3);
    drop(edited);
    drop(record);
    assert_eq!(budget.storage_used(), 0);
}
#[test]
fn deeply_nested_values_import_and_release_without_recursive_host_stack_growth() {
    let mut source = Value::integer(1);
    for _ in 0..10_000 {
        source = Value::record(vec![source]);
    }
    let budget = budget(100_000);
    let imported = budget.storage.import(&source).unwrap();
    assert_eq!(budget.storage_used(), 20_001);
    assert_eq!(imported, source);
    drop(source);
    drop(imported);
    assert_eq!(budget.storage_used(), 0);
}
#[test]
fn wide_import_and_staging_fail_without_leaking_reservations() {
    let source = Value::record(vec![Value::integer(1); 1_000]);
    let budget = budget(10);
    assert_eq!(
        budget.storage.import(&source).unwrap_err(),
        Rule::StorageLimit
    );
    assert_eq!(budget.storage_used(), 0);
    assert!(matches!(
        budget.storage.items(usize::MAX),
        Err(Rule::StorageLimit)
    ));
    assert_eq!(budget.storage_used(), 0);
}

#[test]
fn closed_literal_dags_validate_and_execute_without_expanding_shared_subgraphs() {
    let (code, _) = super::tests::code_ids();
    let mut types = vec![Type::I32];
    let mut source = Value::integer(42);
    for index in 0..120 {
        types.push(Type::Record(vec![index, index].into()));
        source = Value::record(vec![source.clone(), source]);
    }
    let mut plan = super::tests::literal();
    plan.result = 120;
    plan.slots = plan
        .slots
        .iter()
        .map(|slot| Slot { data: 120, ..*slot })
        .collect();
    plan.points = plan.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Literal {
            destination: 0,
            value: source.clone(),
        };
        point
    });
    let program = Program::new(types, BTreeMap::from([(code, plan)])).unwrap();
    let mut budget = budget(1024);
    let value = Executor::new(&program, &mut budget, 0)
        .evaluate_initializer(code)
        .unwrap();
    assert_eq!(value, source);
    assert_eq!(budget.storage_used(), 361);
    drop(value);
    assert_eq!(budget.storage_used(), 0);
}

#[test]
fn excessive_suspended_depth_rejects_without_overflow_or_allocating_a_frame() {
    let (code, _) = super::tests::code_ids();
    let program = Program::new(
        [Type::I32],
        BTreeMap::from([(code, super::tests::literal())]),
    )
    .unwrap();
    let mut budget = budget(16);
    assert_eq!(
        Executor::new(&program, &mut budget, usize::MAX)
            .evaluate_initializer(code)
            .unwrap_err()
            .rule,
        Rule::CallDepthLimit
    );
    assert_eq!(budget.storage_used(), 0);
}

#[test]
fn publication_roundtrip_preserves_deep_shared_data_without_expanding_a_tree() {
    let mut value = Value::integer(42);
    for _ in 0..10_000 {
        value = Value::record(vec![value.clone(), value]);
    }
    let frozen = value.freeze();
    assert_eq!(frozen.nodes().len(), 10_001);
    assert_eq!(Value::from_frozen(&frozen).unwrap(), value);
}
