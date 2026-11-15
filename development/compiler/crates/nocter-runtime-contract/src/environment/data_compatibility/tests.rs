//! Physical transport metadata remains distinct from exact representation and value availability.
use crate::{
    RuntimeAbiIdentity, RuntimeDataId as Id, RuntimeEnvironment, RuntimePrimitive,
    RuntimeType as Ty, RuntimeTypeRepresentation as Representation, RuntimeTypeRepresentationTable,
    RuntimeTypeTableBuilder, RuntimeValueFieldRepresentation,
};
use nocter_model::ValueField;
use std::collections::BTreeMap;

#[test]
fn guarded_transport_widens_only_excluded_alternatives_and_preserves_positive_runtime_identity() {
    let (runtime, left, right) = fixture();
    for (source, target) in [(3, 4), (5, 6)] {
        assert!(
            runtime.permits_guarded_value_transport(Id::Instance(source), Id::Instance(target))
        );
        assert!(
            !runtime.permits_guarded_value_transport(Id::Instance(target), Id::Instance(source))
        );
        assert!(!runtime.has_same_value_representation(Id::Instance(source), Id::Instance(target)));
    }
    assert!(runtime.permits_guarded_value_transport(left, right));
    assert!(!runtime.permits_guarded_value_transport(right, left));
    assert!(!runtime.has_same_value_representation(left, right));
    for (source, target) in [(0, 1), (0, 0), (1, 2), (4, 8), (999, 999)] {
        assert!(
            !runtime.permits_guarded_value_transport(Id::Instance(source), Id::Instance(target)),
            "{source}->{target}"
        );
    }
    assert!(runtime.has_same_value_representation(Id::Instance(1), Id::Instance(1)));
}

fn fixture() -> (RuntimeEnvironment, Id, Id) {
    let mut types = RuntimeTypeTableBuilder::new();
    for (index, ty) in [
        (0, Ty::Uninhabited),
        (1, Ty::StaticCallable { environment: None }),
        (2, Ty::StaticCallable { environment: None }),
        (3, Ty::Optional(Id::Instance(0))),
        (4, Ty::Optional(Id::Instance(1))),
        (5, Ty::Fallible(Id::Instance(0))),
        (6, Ty::Fallible(Id::Instance(1))),
        (7, Ty::Primitive(RuntimePrimitive::Error)),
        (8, Ty::Optional(Id::Instance(2))),
    ] {
        types.insert(Id::Instance(index), ty).unwrap();
    }
    let mut representations = BTreeMap::new();
    let (mut source, mut target) = (Id::Instance(3), Id::Instance(4));
    for level in 0..48 {
        let before = Id::Instance(10 + level * 2);
        let after = Id::Instance(11 + level * 2);
        for (id, child) in [(before, source), (after, target)] {
            types.insert(id, Ty::Aggregate).unwrap();
            representations.insert(
                id,
                Representation::ValueAggregate {
                    fields: [0, 1]
                        .into_iter()
                        .map(|index| {
                            RuntimeValueFieldRepresentation::new(ValueField::Tuple(index), child)
                        })
                        .collect(),
                },
            );
        }
        (source, target) = (before, after);
    }
    let (left, right) = (Id::Instance(110), Id::Instance(111));
    for (id, element) in [(left, source), (right, target)] {
        types
            .insert(
                id,
                Ty::FixedArray {
                    element,
                    length: u64::MAX,
                },
            )
            .unwrap();
    }
    let runtime = RuntimeEnvironment::new(
        types.finish().unwrap(),
        RuntimeTypeRepresentationTable::new(representations),
        RuntimeAbiIdentity::Arm64DarwinV1,
    )
    .unwrap();
    (runtime, left, right)
}
