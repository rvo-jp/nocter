use super::*;
use crate::value_access::{
    CodeKind, DeclarationRule, FieldDefinition, HeaderStep, Reservations, TupleData,
};
use nocter_model::{CompileTimeGuarantee, UsizeTerm};

fn array(element: DataReference, length: u64) -> DataReference {
    DataReference::Array {
        element: Box::new(element),
        length,
    }
}
#[test]
fn zero_array_breaks_self_containment_without_granting_copy_to_its_element() {
    for copy in [false, true] {
        let symbols = SymbolTable::from_spellings(["Node", "next"]);
        let mut reservations = Reservations::new(symbols.clone());
        let node = reservations.reserve_record(symbols.get("Node").unwrap(), 1);
        let mut builder = reservations.finish().unwrap();
        builder
            .define_record(
                node,
                copy,
                [FieldDefinition {
                    name: symbols.get("next").unwrap(),
                    data: array(DataReference::Record(node), 0),
                }],
            )
            .unwrap();
        let program = builder.finish().unwrap();
        let record = program.records().get(node).unwrap();
        let data = program.fields().get(record.fields()[0]).unwrap().data();
        let Some(PayloadSchema::Array(schema)) = program.data_schema(data) else {
            panic!("array schema");
        };
        assert!(schema.is_empty());
        assert_eq!(schema.element(), record.data());
        assert_eq!(program.data_copyable(data), Some(true));
        assert_eq!(program.data_copyable(record.data()), Some(copy));
        assert!(
            program
                .component(data, nocter_model::ValueField::Tuple(0))
                .is_none()
        );
    }
}
#[test]
fn positive_counts_retain_containment_edges_and_failed_cycles_publish_no_fields() {
    for length in [0, 1, u64::MAX] {
        let symbols = SymbolTable::from_spellings(["A", "B", "value"]);
        let mut reservations = Reservations::new(symbols.clone());
        let a = reservations.reserve_record(symbols.get("A").unwrap(), 1);
        let b = reservations.reserve_record(symbols.get("B").unwrap(), 1);
        let mut builder = reservations.finish().unwrap();
        builder
            .define_record(
                a,
                false,
                [FieldDefinition {
                    name: symbols.get("value").unwrap(),
                    data: DataReference::Tuple(TupleData::new(
                        array(DataReference::Record(b), length),
                        BuiltinType::Void.into(),
                        [],
                    )),
                }],
            )
            .unwrap();
        builder
            .define_record(
                b,
                false,
                [FieldDefinition {
                    name: symbols.get("value").unwrap(),
                    data: array(DataReference::Record(a), 1),
                }],
            )
            .unwrap();
        let mut headers = builder.into_publication();
        let before = headers.snapshot();
        if length == 0 {
            headers.publish(a).unwrap();
            assert!(headers.snapshot().header_ready(a));
            assert!(!headers.snapshot().header_ready(b));
            assert!(headers.finish().unwrap().headers_complete());
        } else {
            assert_eq!(
                headers.advance(a).unwrap(),
                HeaderStep::Require(Box::new([b.into()]))
            );
            let error = headers.publish(a).unwrap_err();
            let crate::value_access::HeaderFailure::Rejected(violations) = error else {
                panic!("containment failure");
            };
            assert_eq!(violations[0].declaration, b.into());
            assert!(
                matches!(violations[0].rule, DeclarationRule::RecursivePayload { through } if through == a)
            );
            let after = headers.snapshot();
            assert!(after.fields().is_empty());
            assert!(after.records().is_empty());
            assert_eq!(
                after.types.store().type_count(),
                before.types.store().type_count()
            );
        }
    }
}
#[test]
fn empty_count_does_not_validate_a_bad_element_or_an_invalid_independent_declaration() {
    for builtin in [BuiltinType::Never, BuiltinType::U8] {
        let symbols = SymbolTable::from_spellings(["Bad", "value"]);
        let mut reservations = Reservations::new(symbols.clone());
        let bad = reservations.reserve_record(symbols.get("Bad").unwrap(), 1);
        let mut builder = reservations.finish().unwrap();
        builder
            .define_record(
                bad,
                false,
                [FieldDefinition {
                    name: symbols.get("value").unwrap(),
                    data: array(builtin.into(), 0),
                }],
            )
            .unwrap();
        assert!(
            builder
                .finish()
                .unwrap_err()
                .iter()
                .any(|v| v.rule == DeclarationRule::UnsupportedDataType { data: builtin })
        );
    }
    let symbols = SymbolTable::from_spellings(["Parent", "Bad", "value"]);
    let mut reservations = Reservations::new(symbols.clone());
    let parent = reservations.reserve_record(symbols.get("Parent").unwrap(), 1);
    let bad = reservations.reserve_record(symbols.get("Bad").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    builder
        .define_record(
            parent,
            true,
            [FieldDefinition {
                name: symbols.get("value").unwrap(),
                data: array(DataReference::Record(bad), 0),
            }],
        )
        .unwrap();
    builder
        .define_record(
            bad,
            false,
            [FieldDefinition {
                name: symbols.get("value").unwrap(),
                data: BuiltinType::Never.into(),
            }],
        )
        .unwrap();
    let mut headers = builder.into_publication();
    headers.publish(parent).unwrap();
    assert!(!headers.snapshot().header_ready(bad));
    assert!(headers.finish().is_err());
}
#[test]
fn array_identity_and_copy_use_one_element_descriptor_even_at_maximum_cardinality() {
    let symbols = SymbolTable::from_spellings([
        "Item", "empty", "same", "one", "huge", "nested", "pair", "value",
    ]);
    let mut reservations = Reservations::new(symbols.clone());
    let item = reservations.reserve_record(symbols.get("Item").unwrap(), 0);
    let ids = ["empty", "same", "one", "huge", "nested", "pair"]
        .map(|name| reservations.reserve(symbols.get(name).unwrap(), 0));
    let mut builder = reservations.finish().unwrap();
    builder.define_record(item, false, []).unwrap();
    for (id, data) in ids.into_iter().zip([
        array(DataReference::Record(item), 0),
        array(DataReference::Record(item), 0),
        array(DataReference::Record(item), 1),
        array(BuiltinType::I32.into(), u64::MAX),
        array(array(BuiltinType::Error.into(), 0), u64::MAX),
        DataReference::Tuple(TupleData::new(
            BuiltinType::I32.into(),
            BuiltinType::I32.into(),
            [],
        )),
    ]) {
        builder
            .define(
                id,
                CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
                [],
                data,
            )
            .unwrap();
    }
    let mut headers = builder.into_publication();
    headers.publish(ids[0]).unwrap();
    let early = headers.snapshot();
    assert!(!early.header_ready(item));
    assert_eq!(
        early.data_copyable(early.codes().get(ids[0]).unwrap().result()),
        Some(true)
    );
    assert_eq!(
        headers.advance(ids[2]).unwrap(),
        HeaderStep::Require(Box::new([item.into()]))
    );
    let program = headers.finish().unwrap();
    let data = ids.map(|id| program.codes().get(id).unwrap().result());
    assert_eq!(data[0], data[1]);
    assert_ne!(data[0], data[2]);
    assert_ne!(data[3], data[5]);
    for (index, copy) in [(0, true), (2, false), (3, true), (4, true)] {
        assert_eq!(program.data_copyable(data[index]), Some(copy));
    }
    let Some(PayloadSchema::Array(huge)) = program.data_schema(data[3]) else {
        panic!("array");
    };
    assert_eq!(huge.length(), u64::MAX);
    assert_eq!(
        program.types.store().get(data[3]),
        Some(&TypeKind::FixedArray {
            element: huge.element(),
            length: UsizeTerm::Value(u64::MAX)
        })
    );
    assert_eq!(
        program
            .types
            .store()
            .iter()
            .filter(|(_, ty)| matches!(ty, TypeKind::FixedArray { .. }))
            .count(),
        5
    );
}

#[test]
fn copy_records_consume_the_array_payload_policy_before_publishing_any_parent_field() {
    for length in [0, 1, u64::MAX] {
        let symbols = SymbolTable::from_spellings(["Holder", "Item", "value"]);
        let mut reservations = Reservations::new(symbols.clone());
        let holder = reservations.reserve_record(symbols.get("Holder").unwrap(), 1);
        let item = reservations.reserve_record(symbols.get("Item").unwrap(), 0);
        let mut builder = reservations.finish().unwrap();
        builder.define_record(item, false, []).unwrap();
        builder
            .define_record(
                holder,
                true,
                [FieldDefinition {
                    name: symbols.get("value").unwrap(),
                    data: array(DataReference::Record(item), length),
                }],
            )
            .unwrap();
        let mut headers = builder.into_publication();
        if length == 0 {
            headers.publish(holder).unwrap();
            assert!(!headers.snapshot().header_ready(item));
            let program = headers.finish().unwrap();
            assert!(!program.records().get(item).unwrap().copyable());
            assert!(program.records().get(holder).unwrap().copyable());
        } else {
            let crate::value_access::HeaderFailure::Rejected(failures) =
                headers.publish(holder).unwrap_err()
            else {
                panic!("Copy failure");
            };
            assert_eq!(failures[0].rule, DeclarationRule::NonCopyField);
            let program = headers.snapshot();
            assert!(program.header_ready(item));
            assert!(!program.header_ready(holder));
            assert!(program.fields().is_empty());
            assert!(
                !program
                    .types
                    .store()
                    .iter()
                    .any(|(_, ty)| matches!(ty, TypeKind::FixedArray { .. }))
            );
        }
    }
}

#[test]
fn zero_count_does_not_hide_an_unknown_nominal_identity() {
    let symbols = SymbolTable::from_spellings(["Holder", "value"]);
    let mut reservations = Reservations::new(symbols.clone());
    let holder = reservations.reserve_record(symbols.get("Holder").unwrap(), 1);
    let mut outside = nocter_model::ArenaBuilder::<NominalTypeId, ()>::new();
    outside.insert(());
    let missing = outside.insert(());
    let mut builder = reservations.finish().unwrap();
    builder
        .define_record(
            holder,
            false,
            [FieldDefinition {
                name: symbols.get("value").unwrap(),
                data: array(DataReference::Record(missing), 0),
            }],
        )
        .unwrap();
    assert!(
        builder
            .finish()
            .unwrap_err()
            .iter()
            .any(|v| v.rule == DeclarationRule::UnknownRecordType { record: missing })
    );
}

#[test]
fn cycle_diagnostics_choose_the_positive_field_after_an_empty_array_of_the_same_record() {
    let symbols = SymbolTable::from_spellings(["A", "B", "empty", "payload"]);
    let mut reservations = Reservations::new(symbols.clone());
    let a = reservations.reserve_record(symbols.get("A").unwrap(), 2);
    let b = reservations.reserve_record(symbols.get("B").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    builder
        .define_record(
            a,
            false,
            [
                FieldDefinition {
                    name: symbols.get("empty").unwrap(),
                    data: array(DataReference::Record(b), 0),
                },
                FieldDefinition {
                    name: symbols.get("payload").unwrap(),
                    data: array(DataReference::Record(b), 1),
                },
            ],
        )
        .unwrap();
    builder
        .define_record(
            b,
            false,
            [FieldDefinition {
                name: symbols.get("payload").unwrap(),
                data: DataReference::Record(a),
            }],
        )
        .unwrap();
    let crate::value_access::HeaderFailure::Rejected(failures) =
        builder.into_publication().publish(b).unwrap_err()
    else {
        panic!("payload cycle");
    };
    assert_eq!(failures[0].declaration, a.into());
    assert_eq!(
        failures[0].position,
        crate::value_access::HeaderPosition::Field(1)
    );
    assert_eq!(
        failures[0].rule,
        DeclarationRule::RecursivePayload { through: b }
    );
}
