use super::*;
use crate::{
    RuntimeAbiIdentity, RuntimePrimitive, RuntimeTypeRepresentationTable, RuntimeTypeTableBuilder,
};

fn runtime() -> RuntimeEnvironment {
    let mut types = RuntimeTypeTableBuilder::new();
    for (id, shape) in [
        (0, RuntimeType::StaticCallable { environment: None }),
        (1, RuntimeType::StaticCallable { environment: None }),
        (2, RuntimeType::StaticCallable { environment: None }),
        (
            3,
            RuntimeType::CallableAlternatives(
                vec![RuntimeDataId::Instance(1), RuntimeDataId::Instance(0)].into(),
            ),
        ),
        (
            4,
            RuntimeType::CallableAlternatives(
                vec![
                    RuntimeDataId::Instance(0),
                    RuntimeDataId::Instance(2),
                    RuntimeDataId::Instance(1),
                ]
                .into(),
            ),
        ),
        (5, RuntimeType::Unit),
        (6, RuntimeType::Uninhabited),
        (7, RuntimeType::Primitive(RuntimePrimitive::Unsigned(8))),
    ] {
        types.insert(RuntimeDataId::Instance(id), shape).unwrap();
    }
    RuntimeEnvironment::new(
        types.finish().unwrap(),
        RuntimeTypeRepresentationTable::new(BTreeMap::new()),
        RuntimeAbiIdentity::Arm64DarwinV1,
    )
    .unwrap()
}

#[test]
fn maps_complete_static_identity_and_remaps_original_choice_indices() {
    let runtime = runtime();
    for (source, target, expected) in [
        (0, 3, vec![(0, 1)]),
        (1, 3, vec![(1, 0)]),
        (3, 4, vec![(1, 2), (0, 0)]),
        (3, 3, vec![(1, 0), (0, 1)]),
    ] {
        let actual = runtime
            .callable_case_targets(
                RuntimeDataId::Instance(source),
                RuntimeDataId::Instance(target),
            )
            .unwrap();
        assert_eq!(
            actual
                .iter()
                .map(|case| (case.data(), case.target()))
                .collect::<Vec<_>>(),
            expected
                .into_iter()
                .map(|(data, tag)| (RuntimeDataId::Instance(data), tag))
                .collect::<Vec<_>>()
        );
        assert!(runtime.permits_value_transport(
            RuntimeDataId::Instance(source),
            RuntimeDataId::Instance(target)
        ));
        if source != target {
            assert!(!runtime.permits_guarded_value_transport(
                RuntimeDataId::Instance(source),
                RuntimeDataId::Instance(target)
            ));
            assert!(!runtime.has_same_value_representation(
                RuntimeDataId::Instance(source),
                RuntimeDataId::Instance(target)
            ));
        }
    }
}

#[test]
fn refuses_missing_positive_cases_and_never_selects_from_geometry_or_exclusion() {
    let runtime = runtime();
    for (source, target) in [
        (2, 3),
        (4, 3),
        (3, 0),
        (0, 1),
        (0, 0),
        (5, 3),
        (6, 3),
        (7, 3),
        (999, 3),
        (0, 999),
    ] {
        if source != target {
            assert!(!runtime.permits_value_transport(
                RuntimeDataId::Instance(source),
                RuntimeDataId::Instance(target)
            ));
        }
        assert!(
            runtime
                .callable_case_targets(
                    RuntimeDataId::Instance(source),
                    RuntimeDataId::Instance(target)
                )
                .is_none(),
            "{source}->{target}"
        );
    }
}
