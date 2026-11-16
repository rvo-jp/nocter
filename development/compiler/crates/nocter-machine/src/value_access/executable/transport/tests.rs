//! Closed stored-domain plans, independently of executable source admission.
use super::*;
use nocter_runtime_contract::{
    RuntimeAbiIdentity, RuntimePrimitive, RuntimeTypeRepresentationTable, RuntimeTypeTableBuilder,
};
fn fixture() -> (RuntimeEnvironment, MachineLayoutPlan) {
    let mut types = RuntimeTypeTableBuilder::new();
    for (index, kind) in [
        (0, RuntimeType::StaticCallable { environment: None }),
        (
            1,
            RuntimeType::StaticCallable {
                environment: Some(RuntimeDataId::Instance(5)),
            },
        ),
        (
            2,
            RuntimeType::StaticCallable {
                environment: Some(RuntimeDataId::Instance(6)),
            },
        ),
        (
            3,
            RuntimeType::CallableAlternatives(
                vec![RuntimeDataId::Instance(0), RuntimeDataId::Instance(1)].into(),
            ),
        ),
        (
            4,
            RuntimeType::CallableAlternatives(
                vec![
                    RuntimeDataId::Instance(1),
                    RuntimeDataId::Instance(2),
                    RuntimeDataId::Instance(0),
                ]
                .into(),
            ),
        ),
        (5, RuntimeType::Primitive(RuntimePrimitive::Unsigned(8))),
        (6, RuntimeType::Primitive(RuntimePrimitive::Unsigned(64))),
    ] {
        types.insert(RuntimeDataId::Instance(index), kind).unwrap();
    }
    let runtime = RuntimeEnvironment::new(
        types.finish().unwrap(),
        RuntimeTypeRepresentationTable::new(BTreeMap::new()),
        RuntimeAbiIdentity::Arm64DarwinV1,
    )
    .unwrap();
    let layouts = MachineLayoutPlan::from_runtime(
        &runtime,
        [RuntimeDataId::Instance(3), RuntimeDataId::Instance(4)],
    )
    .unwrap();
    (runtime, layouts)
}
#[test]
fn callable_transport_retains_exact_case_bytes_and_independent_containing_offsets() {
    let (runtime, layouts) = fixture();
    for (source, target, choice, expected) in [
        (0, 4, false, vec![(0, 2, 0, 8)]),
        (3, 4, true, vec![(0, 2, 1, 8), (1, 0, 1, 8)]),
    ] {
        let pair = (
            RuntimeDataId::Instance(source),
            RuntimeDataId::Instance(target),
        );
        let plan = select(&runtime, &layouts, pair).unwrap();
        let Plan::Callable {
            source_choice,
            cases,
        } = &plan
        else {
            panic!("callable plan")
        };
        assert_eq!(*source_choice, choice);
        assert_eq!(
            cases
                .iter()
                .map(|case| {
                    assert_eq!(case.payload.pair.0, case.payload.pair.1);
                    (
                        case.payload.pair.0,
                        case.target_tag,
                        case.payload.source,
                        case.payload.target,
                    )
                })
                .collect::<Vec<_>>(),
            expected
                .into_iter()
                .map(|(data, tag, source, target)| (
                    RuntimeDataId::Instance(data),
                    tag,
                    source,
                    target
                ))
                .collect::<Vec<_>>()
        );
        assert_eq!(plan.children().len(), cases.len());
    }
    assert!(
        select(
            &runtime,
            &layouts,
            (RuntimeDataId::Instance(4), RuntimeDataId::Instance(3))
        )
        .is_err()
    );
    assert!(
        select(
            &runtime,
            &layouts,
            (RuntimeDataId::Instance(3), RuntimeDataId::Instance(0))
        )
        .is_err()
    );
}
