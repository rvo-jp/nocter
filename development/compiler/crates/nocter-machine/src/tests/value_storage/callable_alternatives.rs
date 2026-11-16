//! Closed runtime layout fixtures, not source execution certificates.
use super::*;
use nocter_runtime_contract::{
    RuntimeDataId as Data, RuntimePrimitive, RuntimeTypeRepresentationTable,
    RuntimeTypeTableBuilder,
};

fn runtime(entries: impl IntoIterator<Item = (Data, RuntimeType)>) -> RuntimeEnvironment {
    let mut types = RuntimeTypeTableBuilder::new();
    for (id, shape) in entries {
        types.insert(id, shape).unwrap();
    }
    RuntimeEnvironment::new(
        types.finish().unwrap(),
        RuntimeTypeRepresentationTable::new(std::collections::BTreeMap::new()),
        RuntimeAbiIdentity::Arm64DarwinV1,
    )
    .unwrap()
}

#[test]
fn choices_preserve_tag_identity_and_largest_aligned_inline_environment() {
    let [byte, word, small, large, empty, choice, reverse]: [Data; 7] =
        std::array::from_fn(|i| Data::Instance(u32::try_from(i).unwrap()));
    let environment = runtime([
        (byte, RuntimeType::Primitive(RuntimePrimitive::Unsigned(8))),
        (word, RuntimeType::Primitive(RuntimePrimitive::Unsigned(64))),
        (
            small,
            RuntimeType::StaticCallable {
                environment: Some(byte),
            },
        ),
        (
            large,
            RuntimeType::StaticCallable {
                environment: Some(word),
            },
        ),
        (empty, RuntimeType::StaticCallable { environment: None }),
        (
            choice,
            RuntimeType::CallableAlternatives(vec![empty, large, small].into()),
        ),
        (
            reverse,
            RuntimeType::CallableAlternatives(vec![small, large, empty].into()),
        ),
    ]);
    let layouts = MachineLayoutPlan::from_runtime(&environment, [choice, reverse]).unwrap();
    let layout = layouts.get(choice).unwrap();
    assert_eq!((layout.size(), layout.alignment()), (16, 8));
    assert_eq!(
        layout.kind(),
        &MachineLayoutKind::CallableAlternatives {
            tag: crate::MachineScalar::Integer {
                bits: 8,
                signed: false
            },
            payload_offset: 8,
            cases: vec![empty, large, small].into(),
        }
    );
    assert!(!layouts.has_same_geometry(choice, reverse));
    assert!(!environment.has_same_value_representation(choice, reverse));
    assert!(!environment.permits_guarded_value_transport(small, choice));
    assert!(!environment.permits_guarded_value_transport(choice, small));
    assert_eq!(layouts.class(choice), layouts.class(reverse));
}

#[test]
fn empty_environments_still_require_tags_at_width_boundaries() {
    for (count, bytes, bits) in [
        (2, 1, 8),
        (256, 1, 8),
        (257, 2, 16),
        (65_536, 2, 16),
        (65_537, 4, 32),
    ] {
        let cases: Vec<_> = (1..=count).map(Data::Instance).collect();
        let choice = Data::Instance(0);
        let mut entries: Vec<_> = cases
            .iter()
            .map(|case| (*case, RuntimeType::StaticCallable { environment: None }))
            .collect();
        entries.push((
            choice,
            RuntimeType::CallableAlternatives(cases.clone().into()),
        ));
        let environment = runtime(entries);
        let layouts = MachineLayoutPlan::from_runtime(&environment, [choice]).unwrap();
        let layout = layouts.get(choice).unwrap();
        assert_eq!((layout.size(), layout.alignment()), (bytes, bytes));
        assert_eq!(
            layout.kind(),
            &MachineLayoutKind::CallableAlternatives {
                tag: crate::MachineScalar::Integer {
                    bits,
                    signed: false
                },
                payload_offset: bytes,
                cases: cases.into(),
            }
        );
    }
}

#[test]
fn recursive_or_overflowing_positive_environments_do_not_gain_layouts() {
    let [choice, first, second, payload, byte]: [Data; 5] =
        std::array::from_fn(|i| Data::Instance(u32::try_from(i).unwrap()));
    for (shape, overflow) in [
        (
            RuntimeType::StaticCallable {
                environment: Some(choice),
            },
            false,
        ),
        (
            RuntimeType::FixedArray {
                element: byte,
                length: u64::MAX,
            },
            true,
        ),
    ] {
        let environment = runtime([
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
            (payload, shape),
            (byte, RuntimeType::Primitive(RuntimePrimitive::Unsigned(8))),
        ]);
        let error = MachineLayoutPlan::from_runtime(&environment, [choice]).unwrap_err();
        if overflow {
            assert_eq!(error, MachineLayoutError::LayoutOverflow(choice));
        } else {
            assert!(matches!(error, MachineLayoutError::RecursiveValue(_)));
        }
    }
}

#[test]
fn empty_array_environment_cuts_recursive_metadata_without_losing_the_tag() {
    let [choice, first, second, empty]: [Data; 4] =
        std::array::from_fn(|i| Data::Instance(u32::try_from(i).unwrap()));
    let environment = runtime([
        (
            choice,
            RuntimeType::CallableAlternatives(vec![first, second].into()),
        ),
        (
            first,
            RuntimeType::StaticCallable {
                environment: Some(empty),
            },
        ),
        (second, RuntimeType::StaticCallable { environment: None }),
        (empty, RuntimeType::EmptyArray { element: choice }),
    ]);
    let layouts = MachineLayoutPlan::from_runtime(&environment, [choice]).unwrap();
    assert_eq!(
        (
            layouts.get(choice).unwrap().size(),
            layouts.get(choice).unwrap().alignment()
        ),
        (1, 1)
    );
    assert_eq!(layouts.get(first).unwrap().size(), 0);
}
