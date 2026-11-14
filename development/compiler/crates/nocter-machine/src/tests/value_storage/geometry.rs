//! Physical correspondence, independent of source admission and initialized values.
use super::*;
use nocter_runtime_contract::{
    RuntimeDataId as Data, RuntimePrimitive, RuntimeTypeRepresentationTable,
    RuntimeTypeTableBuilder,
};

fn plan(entries: impl IntoIterator<Item = (Data, RuntimeType)>) -> MachineLayoutPlan {
    let mut types = RuntimeTypeTableBuilder::new();
    let mut roots = Vec::new();
    for (id, shape) in entries {
        types.insert(id, shape).unwrap();
        roots.push(id);
    }
    let runtime = RuntimeEnvironment::new(
        types.finish().unwrap(),
        RuntimeTypeRepresentationTable::new(std::collections::BTreeMap::new()),
        RuntimeAbiIdentity::Arm64DarwinV1,
    )
    .unwrap();
    MachineLayoutPlan::from_runtime(&runtime, roots).unwrap()
}

#[test]
fn equal_root_size_and_alignment_do_not_hide_shifted_sibling_fields() {
    let ids: [Data; 8] = std::array::from_fn(|i| Data::Instance(u32::try_from(i).unwrap()));
    let [byte, word, wide, small, large, same, before, after] = ids;
    let layouts = plan([
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
    ]);
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
