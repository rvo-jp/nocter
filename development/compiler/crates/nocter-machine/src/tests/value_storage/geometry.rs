//! Physical correspondence, independent of source admission and initialized values.
use super::*;
use nocter_model::{ValuePath, ValueProjection};
use nocter_runtime_contract::{
    RuntimeDataId as Data, RuntimePrimitive, RuntimeTypeRepresentation,
    RuntimeTypeRepresentationTable, RuntimeTypeTableBuilder, RuntimeValueFieldRepresentation,
};

fn plan(entries: impl IntoIterator<Item = (Data, RuntimeType)>) -> MachineLayoutPlan {
    represented_plan(entries, std::collections::BTreeMap::new())
}
fn represented_plan(
    entries: impl IntoIterator<Item = (Data, RuntimeType)>,
    representations: std::collections::BTreeMap<
        Data,
        nocter_runtime_contract::RuntimeTypeRepresentation,
    >,
) -> MachineLayoutPlan {
    let mut types = RuntimeTypeTableBuilder::new();
    let mut roots = Vec::new();
    for (id, shape) in entries {
        types.insert(id, shape).unwrap();
        roots.push(id);
    }
    let runtime = RuntimeEnvironment::new(
        types.finish().unwrap(),
        RuntimeTypeRepresentationTable::new(representations),
        RuntimeAbiIdentity::Arm64DarwinV1,
    )
    .unwrap();
    MachineLayoutPlan::from_runtime(&runtime, roots).unwrap()
}

fn shifted_sibling_types() -> ([Data; 8], [(Data, RuntimeType); 8]) {
    let ids: [Data; 8] = std::array::from_fn(|i| Data::Instance(u32::try_from(i).unwrap()));
    let [byte, word, wide, small, large, same, before, after] = ids;
    let entries = [
        (byte, RuntimeType::Primitive(RuntimePrimitive::Unsigned(8))),
        (word, RuntimeType::Primitive(RuntimePrimitive::Unsigned(32))),
        (wide, RuntimeType::Primitive(RuntimePrimitive::Unsigned(64))),
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
        (
            same,
            RuntimeType::StaticCallable {
                environment: Some(byte),
            },
        ),
        (before, RuntimeType::Tuple(vec![small, byte, wide].into())),
        (after, RuntimeType::Tuple(vec![large, byte, wide].into())),
    ];
    (ids, entries)
}

#[test]
fn equal_root_size_and_alignment_do_not_hide_shifted_sibling_fields() {
    let ([_, _, _, small, _, same, before, after], entries) = shifted_sibling_types();
    let layouts = plan(entries.clone());
    let old = layouts.get(before).unwrap();
    let new = layouts.get(after).unwrap();
    assert_eq!((old.size(), old.alignment()), (new.size(), new.alignment()));
    assert_eq!(old.size(), 16);
    let (MachineLayoutKind::Tuple { elements: a }, MachineLayoutKind::Tuple { elements: b }) =
        (old.kind(), new.kind())
    else {
        panic!("tuple geometry")
    };
    assert_eq!(a[1].offset(), 1);
    assert_eq!(b[1].offset(), 4);
    assert!(!layouts.has_same_geometry(before, after));
    assert!(!layouts.has_same_geometry(after, before));
    assert!(
        layouts.has_same_geometry(small, same),
        "different context IDs may have identical geometry"
    );
    assert!(layouts.has_same_geometry(before, before));
    assert!(!layouts.has_same_geometry(Data::Instance(999), Data::Instance(999)));
}

#[test]
fn logical_projection_offsets_require_correspondence_and_distinguish_moved_siblings() {
    let ([byte, _, wide, small, large, _, before, after], entries) = shifted_sibling_types();
    let layouts = plan(entries.clone());
    let fields = |values: &[Data]| RuntimeTypeRepresentation::ValueAggregate {
        fields: values
            .iter()
            .enumerate()
            .map(|(index, &data)| {
                RuntimeValueFieldRepresentation::new(nocter_model::ValueField::Tuple(index), data)
            })
            .collect(),
    };
    let value_entries = entries.into_iter().map(|(id, shape)| {
        (
            id,
            if id == before || id == after {
                RuntimeType::Aggregate
            } else {
                shape
            },
        )
    });
    let offsets = represented_plan(
        value_entries,
        std::collections::BTreeMap::from([
            (before, fields(&[small, byte, wide])),
            (after, fields(&[large, byte, wide])),
        ]),
    );
    assert!(offsets.has_same_projection_offset(
        before,
        after,
        &ValuePath::new([ValueProjection::Tuple(0)])
    ));
    assert!(!offsets.has_same_projection_offset(
        before,
        after,
        &ValuePath::new([ValueProjection::Tuple(1)])
    ));
    assert!(offsets.has_same_projection_offset(
        before,
        after,
        &ValuePath::new([ValueProjection::Tuple(2)])
    ));
    assert!(!offsets.has_same_projection_offset(
        before,
        after,
        &ValuePath::new([ValueProjection::Tuple(3)])
    ));
    assert!(!offsets.has_same_projection_offset(
        Data::Instance(999),
        Data::Instance(999),
        &ValuePath::default()
    ));

    assert!(
        !layouts.has_same_projection_offset(
            before,
            after,
            &ValuePath::new([ValueProjection::Tuple(0)])
        ),
        "physical tuple order is not logical source correspondence"
    );
}

#[test]
fn empty_and_max_length_zero_size_arrays_need_no_element_expansion() {
    let [a, b, left, right, empty_a, empty_b, absent_a, absent_b]: [Data; 8] =
        std::array::from_fn(|i| Data::Instance(u32::try_from(i).unwrap()));
    let layouts = plan([
        (a, RuntimeType::StaticCallable { environment: None }),
        (b, RuntimeType::StaticCallable { environment: None }),
        (
            left,
            RuntimeType::FixedArray {
                element: a,
                length: u64::MAX,
            },
        ),
        (
            right,
            RuntimeType::FixedArray {
                element: b,
                length: u64::MAX,
            },
        ),
        (empty_a, RuntimeType::EmptyArray { element: absent_a }),
        (empty_b, RuntimeType::EmptyArray { element: absent_b }),
    ]);
    assert_eq!(layouts.get(left).unwrap().size(), 0);
    assert!(layouts.has_same_geometry(left, right));
    assert!(layouts.has_same_geometry(empty_a, empty_b));
    assert!(
        !layouts.has_same_geometry(left, empty_a),
        "cardinality remains part of geometry"
    );
}

#[test]
fn deeply_shared_geometry_walks_each_pair_without_expanding_its_logical_tree() {
    let mut entries = vec![
        (Data::Instance(0), RuntimeType::Unit),
        (Data::Instance(1), RuntimeType::Unit),
    ];
    let mut pair = [Data::Instance(0), Data::Instance(1)];
    for level in 1..=4096 {
        let next = [Data::Instance(level * 2), Data::Instance(level * 2 + 1)];
        for (id, child) in next.into_iter().zip(pair) {
            entries.push((id, RuntimeType::Tuple(vec![child, child].into())));
        }
        pair = next;
    }
    let layouts = plan(entries);
    assert!(layouts.has_same_geometry(pair[0], pair[1]));
}

#[test]
fn projection_offsets_distinguish_known_indices_from_all_existing_coordinates() {
    let ids: [Data; 10] = std::array::from_fn(|i| Data::Instance(u32::try_from(i).unwrap()));
    let [
        byte,
        word,
        one_old,
        one_new,
        many_old,
        many_new,
        empty_old,
        empty_new,
        zst,
        huge,
    ] = ids;
    let layouts = plan([
        (byte, RuntimeType::Primitive(RuntimePrimitive::Unsigned(8))),
        (word, RuntimeType::Primitive(RuntimePrimitive::Unsigned(32))),
        (
            one_old,
            RuntimeType::FixedArray {
                element: byte,
                length: 1,
            },
        ),
        (
            one_new,
            RuntimeType::FixedArray {
                element: word,
                length: 1,
            },
        ),
        (
            many_old,
            RuntimeType::FixedArray {
                element: byte,
                length: 4,
            },
        ),
        (
            many_new,
            RuntimeType::FixedArray {
                element: word,
                length: 4,
            },
        ),
        (empty_old, RuntimeType::EmptyArray { element: byte }),
        (empty_new, RuntimeType::EmptyArray { element: word }),
        (zst, RuntimeType::Unit),
        (
            huge,
            RuntimeType::FixedArray {
                element: zst,
                length: u64::MAX,
            },
        ),
    ]);
    let index = |index| ValuePath::new([ValueProjection::Array(index)]);
    assert!(layouts.has_same_projection_offset(one_old, one_new, &index(None)));
    assert!(layouts.has_same_projection_offset(many_old, many_new, &index(Some(0))));
    assert!(!layouts.has_same_projection_offset(many_old, many_new, &index(Some(1))));
    assert!(!layouts.has_same_projection_offset(many_old, many_new, &index(None)));
    assert!(!layouts.has_same_projection_offset(many_old, many_new, &index(Some(4))));
    assert!(!layouts.has_same_projection_offset(empty_old, empty_new, &index(None)));
    assert!(!layouts.has_same_projection_offset(empty_old, empty_new, &index(Some(0))));
    assert!(!layouts.has_same_projection_offset(one_old, many_old, &index(None)));
    assert!(layouts.has_same_projection_offset(huge, huge, &index(None)));
    assert!(layouts.has_same_projection_offset(huge, huge, &index(Some(u64::MAX - 1))));
    assert!(!layouts.has_same_projection_offset(huge, huge, &index(Some(u64::MAX))));
}
