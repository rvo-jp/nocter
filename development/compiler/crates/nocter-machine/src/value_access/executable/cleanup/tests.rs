//! Stored-schema cleanup requirements, not source execution qualification.
use super::*;
use nocter_runtime_contract::{
    RuntimeAbiIdentity, RuntimeTypeRepresentationTable, RuntimeTypeTableBuilder,
};

fn runtime(entries: impl IntoIterator<Item = (RuntimeDataId, RuntimeType)>) -> RuntimeEnvironment {
    let mut types = RuntimeTypeTableBuilder::new();
    for (id, shape) in entries {
        types.insert(id, shape).unwrap();
    }
    RuntimeEnvironment::new(
        types.finish().unwrap(),
        RuntimeTypeRepresentationTable::new(BTreeMap::new()),
        RuntimeAbiIdentity::Arm64DarwinV1,
    )
    .unwrap()
}

#[test]
fn choice_dependencies_retain_all_possible_helpers_without_repeating_shared_environments() {
    let [error, empty, owned, other, choice, array]: [RuntimeDataId; 6] =
        std::array::from_fn(|i| RuntimeDataId::Instance(u32::try_from(i).unwrap()));
    let runtime = runtime([
        (error, RuntimeType::Primitive(RuntimePrimitive::Error)),
        (empty, RuntimeType::StaticCallable { environment: None }),
        (
            owned,
            RuntimeType::StaticCallable {
                environment: Some(error),
            },
        ),
        (
            other,
            RuntimeType::StaticCallable {
                environment: Some(error),
            },
        ),
        (
            choice,
            RuntimeType::CallableAlternatives(vec![empty, owned, other].into()),
        ),
        (
            array,
            RuntimeType::FixedArray {
                element: choice,
                length: u64::MAX,
            },
        ),
    ]);
    let mut cache = BTreeMap::new();
    assert!(required(&runtime, array, &mut cache).unwrap());
    assert_eq!(
        cache,
        BTreeMap::from([
            (error, true),
            (empty, false),
            (owned, true),
            (other, true),
            (choice, true),
            (array, true)
        ])
    );
    assert!(required(&runtime, choice, &mut cache).unwrap());
    assert_eq!(cache.len(), 6);
}

#[test]
fn empty_arrays_cut_recursive_choice_metadata_but_positive_cycles_reject() {
    let [choice, first, second, payload]: [RuntimeDataId; 4] =
        std::array::from_fn(|i| RuntimeDataId::Instance(u32::try_from(i).unwrap()));
    for empty in [true, false] {
        let runtime = runtime([
            (
                choice,
                RuntimeType::CallableAlternatives(vec![first, second].into()),
            ),
            (
                first,
                RuntimeType::StaticCallable {
                    environment: Some(payload),
                },
            ),
            (second, RuntimeType::StaticCallable { environment: None }),
            (
                payload,
                if empty {
                    RuntimeType::EmptyArray { element: choice }
                } else {
                    RuntimeType::StaticCallable {
                        environment: Some(choice),
                    }
                },
            ),
        ]);
        let mut cache = BTreeMap::new();
        if empty {
            assert!(!required(&runtime, choice, &mut cache).unwrap());
            assert_eq!(cache.len(), 4);
        } else {
            assert!(matches!(
                required(&runtime, choice, &mut cache),
                Err(Failure::InvalidRoot)
            ));
            assert!(!cache.contains_key(&choice));
        }
    }
}

#[test]
fn fallible_cleanup_retains_success_payload_and_error_alternative_dependencies() {
    let [error, owned, outcome]: [RuntimeDataId; 3] =
        std::array::from_fn(|i| RuntimeDataId::Instance(u32::try_from(i).unwrap()));
    let runtime = runtime([
        (error, RuntimeType::Primitive(RuntimePrimitive::Error)),
        (
            owned,
            RuntimeType::StaticCallable {
                environment: Some(error),
            },
        ),
        (outcome, RuntimeType::Fallible(owned)),
    ]);
    let mut cache = BTreeMap::new();
    assert!(required(&runtime, outcome, &mut cache).unwrap());
    assert_eq!(
        cache,
        BTreeMap::from([(error, true), (owned, true), (outcome, true)])
    );
}

#[test]
fn excluded_fallible_primary_retains_only_its_actual_error_helper() {
    let [empty, error, outcome]: [RuntimeDataId; 3] =
        std::array::from_fn(|i| RuntimeDataId::Instance(u32::try_from(i).unwrap()));
    let runtime = runtime([
        (empty, RuntimeType::Uninhabited),
        (error, RuntimeType::Primitive(RuntimePrimitive::Error)),
        (outcome, RuntimeType::Fallible(empty)),
    ]);
    let mut cache = BTreeMap::new();
    assert!(required(&runtime, outcome, &mut cache).unwrap());
    assert_eq!(cache, BTreeMap::from([(error, true), (outcome, true)]));
}
