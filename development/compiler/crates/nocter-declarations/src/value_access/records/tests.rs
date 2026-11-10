use super::*;
use crate::value_access::PayloadSchema;
use crate::value_access::{ParameterDefinition, Reservations};
use nocter_model::{BuiltinType, CompileTimeGuarantee, ValueInputOperation};

fn field(symbols: &SymbolTable, name: &str, data: DataReference) -> FieldDefinition {
    FieldDefinition {
        name: symbols.get(name).unwrap(),
        data,
    }
}

fn nested(reverse: bool) -> super::super::Program {
    let symbols = SymbolTable::from_spellings([
        "Inner", "Outer", "Ordinary", "Empty", "f", "input", "left", "right", "value",
    ]);
    let mut reservations = Reservations::new(symbols.clone());
    let empty = reservations.reserve_record(symbols.get("Empty").unwrap(), 0);
    let inner = reservations.reserve_record(symbols.get("Inner").unwrap(), 1);
    let ordinary = reservations.reserve_record(symbols.get("Ordinary").unwrap(), 0);
    let outer = reservations.reserve_record(symbols.get("Outer").unwrap(), 2);
    let function = reservations.reserve(symbols.get("f").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    let records = if reverse {
        [outer, ordinary, inner, empty]
    } else {
        [empty, inner, ordinary, outer]
    };
    for id in records {
        let (copy, fields) = if id == inner {
            (
                true,
                vec![field(
                    &symbols,
                    "value",
                    DataReference::Builtin(BuiltinType::I32),
                )],
            )
        } else if id == outer {
            (
                true,
                vec![
                    field(&symbols, "left", DataReference::Record(inner)),
                    field(&symbols, "right", DataReference::Record(inner)),
                ],
            )
        } else {
            (false, vec![])
        };
        builder.define_record(id, copy, fields).unwrap();
    }
    builder
        .define(
            function,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [ParameterDefinition {
                name: symbols.get("input").unwrap(),
                data: BuiltinType::Error.into(),
                operation: ValueInputOperation::Take,
            }],
            BuiltinType::Void,
        )
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn records_share_the_program_authority_and_preserve_canonical_ids() {
    let a = nested(false);
    let b = nested(true);
    assert_eq!(a.records(), b.records());
    assert_eq!(a.fields(), b.fields());
    assert_eq!(a.callables(), b.callables());
    assert_eq!(a.parameters(), b.parameters());
    let outer = a.resolve_record(a.symbols().get("Outer").unwrap()).unwrap();
    let record = a.records().get(outer).unwrap();
    assert_eq!(
        a.data_schema(record.data()),
        Some(PayloadSchema::Aggregate(
            super::super::AggregateSchema::record(record, a.fields())
        ))
    );
    assert_eq!(a.data_kind(record.data()), None); // A record is not flattened to a scalar.
    assert_eq!(a.data_copyable(record.data()), Some(true));
    assert_eq!(record.fields().len(), 2);
    assert_ne!(record.fields()[0], record.fields()[1]);
    for (position, &id) in record.fields().iter().enumerate() {
        let f = *a.fields().get(id).unwrap();
        assert_eq!(f.owner(), outer);
        assert_eq!(f.position(), position);
        assert_eq!(record.resolve_field(f.name()), Some(id));
        assert_eq!(a.data_copyable(f.data()), Some(true));
    }
}

#[test]
fn copy_opt_in_is_not_inferred_from_empty_or_all_copy_fields() {
    let symbols = SymbolTable::from_spellings(["Empty", "CopyEmpty", "Plain", "value"]);
    let mut reservations = Reservations::new(symbols.clone());
    let empty = reservations.reserve_record(symbols.get("Empty").unwrap(), 0);
    let copy_empty = reservations.reserve_record(symbols.get("CopyEmpty").unwrap(), 0);
    let plain = reservations.reserve_record(symbols.get("Plain").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    builder.define_record(empty, false, []).unwrap();
    builder.define_record(copy_empty, true, []).unwrap();
    builder
        .define_record(
            plain,
            false,
            [field(
                &symbols,
                "value",
                DataReference::Builtin(BuiltinType::Bool),
            )],
        )
        .unwrap();
    let program = builder.finish().unwrap();
    for (id, expected) in [(empty, false), (copy_empty, true), (plain, false)] {
        let data = program.records().get(id).unwrap().data();
        assert_eq!(program.data_copyable(data), Some(expected));
    }
    assert_ne!(
        program.records().get(empty).unwrap().data(),
        program.records().get(copy_empty).unwrap().data()
    );
}

#[test]
fn opaque_error_and_noncopy_records_cannot_be_hidden_by_copy_opt_in() {
    let symbols = SymbolTable::from_spellings(["A", "B", "Plain", "first", "second"]);
    let mut reservations = Reservations::new(symbols.clone());
    let a = reservations.reserve_record(symbols.get("A").unwrap(), 2);
    let b = reservations.reserve_record(symbols.get("B").unwrap(), 1);
    let plain = reservations.reserve_record(symbols.get("Plain").unwrap(), 0);
    let mut builder = reservations.finish().unwrap();
    builder
        .define_record(
            a,
            true,
            [
                field(&symbols, "first", DataReference::Record(b)),
                field(&symbols, "second", DataReference::Record(plain)),
            ],
        )
        .unwrap();
    builder
        .define_record(
            b,
            true,
            [field(
                &symbols,
                "first",
                DataReference::Builtin(BuiltinType::Error),
            )],
        )
        .unwrap();
    builder.define_record(plain, false, []).unwrap();
    let mut headers = builder.into_publication().unwrap();
    let before = headers.snapshot();
    let expected = super::super::HeaderFailure::Rejected(vec![DeclarationViolation {
        declaration: DeclarationId::Record(b),
        position: HeaderPosition::Field(0),
        rule: DeclarationRule::NonCopyField,
    }]);
    // The invalid child cannot publish as non-Copy, or make A produce a secondary Copy failure.
    assert_eq!(headers.publish(a), Err(expected));
    let after = headers.snapshot();
    assert!(after.records().is_empty());
    assert!(after.fields().is_empty());
    assert_eq!(
        after.types.store().type_count(),
        before.types.store().type_count()
    );
    assert!(after.preserves_headers(&before));
    // An independent ordinary non-Copy record remains a valid dependency after the failure.
    headers.publish(plain).unwrap();
    assert!(!headers.snapshot().records().get(plain).unwrap().copyable());
    assert!(matches!(
        headers.finish(),
        Err(super::super::HeaderFailure::Rejected(_))
    ));
}

#[test]
fn self_mutual_and_containing_recursive_payloads_cannot_publish() {
    let symbols = SymbolTable::from_spellings(["A", "B", "C", "Self", "value"]);
    let mut reservations = Reservations::new(symbols.clone());
    let a = reservations.reserve_record(symbols.get("A").unwrap(), 1);
    let b = reservations.reserve_record(symbols.get("B").unwrap(), 1);
    let c = reservations.reserve_record(symbols.get("C").unwrap(), 1);
    let recursive = reservations.reserve_record(symbols.get("Self").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    for (id, child) in [(a, b), (b, a), (c, a), (recursive, recursive)] {
        builder
            .define_record(
                id,
                false,
                [field(&symbols, "value", DataReference::Record(child))],
            )
            .unwrap();
    }
    let mut headers = builder.into_publication().unwrap();
    for (request, owner, through) in [
        (a, b, a),
        (b, a, b),
        (c, b, a),
        (recursive, recursive, recursive),
    ] {
        assert_eq!(
            headers.publish(request),
            Err(super::super::HeaderFailure::Rejected(vec![
                DeclarationViolation {
                    declaration: DeclarationId::Record(owner),
                    position: HeaderPosition::Field(0),
                    rule: DeclarationRule::RecursivePayload { through },
                }
            ]))
        );
        assert!(headers.snapshot().records().is_empty());
        assert!(headers.snapshot().fields().is_empty());
    }
    // Final whole-unit closure still rejects unused invalid declarations.
    assert!(headers.finish().is_err());
}

#[test]
fn missing_or_second_definitions_never_replace_a_valid_schema() {
    let symbols = SymbolTable::from_spellings(["A", "B"]);
    let mut reservations = Reservations::new(symbols.clone());
    let a = reservations.reserve_record(symbols.get("A").unwrap(), 0);
    let b = reservations.reserve_record(symbols.get("B").unwrap(), 0);
    let mut builder = reservations.finish().unwrap();
    builder.define_record(a, true, []).unwrap();
    assert_eq!(
        builder.define_record(a, false, []),
        Err(DefinitionError::AlreadyDefined)
    );
    let violations = builder.finish().unwrap_err();
    assert_eq!(
        violations,
        [DeclarationViolation {
            declaration: DeclarationId::Record(b),
            position: HeaderPosition::Name,
            rule: DeclarationRule::MissingDefinition
        }]
    );
}

#[test]
fn unknown_reservations_are_rejected_at_definition_and_field_use() {
    let symbols = SymbolTable::from_spellings(["A", "value"]);
    let mut identities = ArenaBuilder::<NominalTypeId, ()>::new();
    identities.insert(());
    let absent = identities.insert(());
    let mut reservations = Reservations::new(symbols.clone());
    let a = reservations.reserve_record(symbols.get("A").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    assert_eq!(
        builder.define_record(absent, true, []),
        Err(DefinitionError::UnknownId)
    );
    builder
        .define_record(
            a,
            false,
            [field(&symbols, "value", DataReference::Record(absent))],
        )
        .unwrap();
    assert_eq!(
        builder.finish().unwrap_err(),
        [DeclarationViolation {
            declaration: DeclarationId::Record(a),
            position: HeaderPosition::Field(0),
            rule: DeclarationRule::UnknownRecordType { record: absent }
        }]
    );
}

#[test]
fn duplicate_record_names_and_callable_collisions_fail_before_any_definition() {
    let symbols = SymbolTable::from_spellings(["A", "f"]);
    let mut reservations = Reservations::new(symbols.clone());
    let a = reservations.reserve_record(symbols.get("A").unwrap(), 0);
    let duplicate = reservations.reserve_record(symbols.get("A").unwrap(), 0);
    let collision = reservations.reserve_record(symbols.get("f").unwrap(), 0);
    let repeated_collision = reservations.reserve_record(symbols.get("f").unwrap(), 0);
    let f = reservations.reserve(symbols.get("f").unwrap(), 0);
    assert_eq!(
        reservations.finish().unwrap_err(),
        [
            DeclarationViolation {
                declaration: DeclarationId::Record(duplicate),
                position: HeaderPosition::Name,
                rule: DeclarationRule::DuplicateRecord { first: a }
            },
            DeclarationViolation {
                declaration: DeclarationId::Record(collision),
                position: HeaderPosition::Name,
                rule: DeclarationRule::RecordShadowsCallable { declaration: f }
            },
            DeclarationViolation {
                declaration: DeclarationId::Record(repeated_collision),
                position: HeaderPosition::Name,
                rule: DeclarationRule::DuplicateRecord { first: collision }
            },
        ]
    );
}

#[test]
fn duplicate_field_names_fail_after_valid_reservations() {
    let symbols = SymbolTable::from_spellings(["A", "value"]);
    let mut reservations = Reservations::new(symbols.clone());
    let a = reservations.reserve_record(symbols.get("A").unwrap(), 2);
    let mut builder = reservations.finish().unwrap();
    let value = field(&symbols, "value", DataReference::Builtin(BuiltinType::I32));
    builder
        .define_record(a, false, [value.clone(), value])
        .unwrap();
    assert_eq!(
        builder.finish().unwrap_err(),
        [DeclarationViolation {
            declaration: DeclarationId::Record(a),
            position: HeaderPosition::Field(1),
            rule: DeclarationRule::DuplicateField { first: 0 }
        }]
    );
}

#[test]
fn reserved_and_unknown_record_names_fail_without_requesting_a_schema() {
    let symbols = SymbolTable::from_spellings(["A", "i32", "value"]);
    let foreign_symbols = SymbolTable::from_spellings(["a", "b", "c", "d"]);
    let unknown = foreign_symbols.get("d").unwrap();
    let mut reservations = Reservations::new(symbols.clone());
    let builtin = reservations.reserve_record(symbols.get("i32").unwrap(), 0);
    let absent = reservations.reserve_record(unknown, 0);
    assert_eq!(
        reservations.finish().unwrap_err(),
        [
            DeclarationViolation {
                declaration: DeclarationId::Record(builtin),
                position: HeaderPosition::Name,
                rule: DeclarationRule::ReservedBuiltinName,
            },
            DeclarationViolation {
                declaration: DeclarationId::Record(absent),
                position: HeaderPosition::Name,
                rule: DeclarationRule::UnknownSymbol,
            },
        ]
    );
}

#[test]
fn unsupported_field_data_and_unknown_field_names_cannot_enter_frozen_schemas() {
    let symbols = SymbolTable::from_spellings(["A", "i32", "value"]);
    let foreign_symbols = SymbolTable::from_spellings(["a", "b", "c", "d"]);
    let unknown = foreign_symbols.get("d").unwrap();
    let mut reservations = Reservations::new(symbols.clone());
    let a = reservations.reserve_record(symbols.get("A").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    builder
        .define_record(
            a,
            true,
            [FieldDefinition {
                name: unknown,
                data: DataReference::Builtin(BuiltinType::F64),
            }],
        )
        .unwrap();
    assert_eq!(
        builder.finish().unwrap_err(),
        [
            DeclarationViolation {
                declaration: DeclarationId::Record(a),
                position: HeaderPosition::Field(0),
                rule: DeclarationRule::UnknownSymbol,
            },
            DeclarationViolation {
                declaration: DeclarationId::Record(a),
                position: HeaderPosition::Field(0),
                rule: DeclarationRule::UnsupportedDataType {
                    data: BuiltinType::F64
                },
            },
        ]
    );
}

#[test]
fn field_names_are_local_and_type_names_do_not_change_field_identity() {
    let symbols = SymbolTable::from_spellings(["A", "B"]);
    let mut reservations = Reservations::new(symbols.clone());
    let a = reservations.reserve_record(symbols.get("A").unwrap(), 1);
    let b = reservations.reserve_record(symbols.get("B").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    let fields = [field(
        &symbols,
        "A",
        DataReference::Builtin(BuiltinType::I32),
    )];
    builder.define_record(a, true, fields.clone()).unwrap();
    builder.define_record(b, true, fields).unwrap();
    let program = builder.finish().unwrap();
    let a = program.records().get(a).unwrap();
    let b = program.records().get(b).unwrap();
    assert_ne!(a.data(), b.data());
    assert_ne!(a.fields()[0], b.fields()[0]);
    assert_eq!(
        program.fields().get(a.fields()[0]).unwrap().data(),
        program.fields().get(b.fields()[0]).unwrap().data()
    );
}

#[test]
fn deep_shared_schemas_do_not_recurse_or_expand_payload_trees() {
    const COUNT: usize = 4096;
    let names: Vec<_> = (0..COUNT).map(|i| format!("R{i:04}")).collect();
    let symbols =
        SymbolTable::from_spellings(names.iter().map(String::as_str).chain(["left", "right"]));
    let mut reservations = Reservations::new(symbols.clone());
    let ids: Vec<_> = names
        .iter()
        .map(|name| {
            reservations.reserve_record(
                symbols.get(name).unwrap(),
                if name == &names[0] { 0 } else { 2 },
            )
        })
        .collect();
    let mut builder = reservations.finish().unwrap();
    builder.define_record(ids[0], true, []).unwrap();
    for pair in ids.windows(2) {
        builder
            .define_record(
                pair[1],
                true,
                [
                    field(&symbols, "left", DataReference::Record(pair[0])),
                    field(&symbols, "right", DataReference::Record(pair[0])),
                ],
            )
            .unwrap();
    }
    let mut headers = builder.into_publication().unwrap();
    let early = headers.snapshot();
    assert!(early.records().is_empty());
    assert!(early.fields().is_empty());
    assert!(!early.headers_complete());
    headers.publish(*ids.last().unwrap()).unwrap();
    let program = headers.snapshot();
    assert!(program.headers_complete());
    assert!(program.preserves_headers(&early));
    assert_eq!(program.records().len(), COUNT);
    assert_eq!(program.fields().len(), 2 * (COUNT - 1));
    for &id in &ids {
        assert_eq!(
            program.data_copyable(program.records().get(id).unwrap().data()),
            Some(true)
        );
    }
}

#[test]
fn ordinary_records_preserve_move_only_error_fields_without_flattening() {
    let symbols = SymbolTable::from_spellings(["Failure", "Wrapper", "error", "value"]);
    let mut reservations = Reservations::new(symbols.clone());
    let failure = reservations.reserve_record(symbols.get("Failure").unwrap(), 1);
    let wrapper = reservations.reserve_record(symbols.get("Wrapper").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    builder
        .define_record(
            failure,
            false,
            [field(
                &symbols,
                "error",
                DataReference::Builtin(BuiltinType::Error),
            )],
        )
        .unwrap();
    builder
        .define_record(
            wrapper,
            false,
            [field(&symbols, "value", DataReference::Record(failure))],
        )
        .unwrap();
    let program = builder.finish().unwrap();
    let wrapper = program.records().get(wrapper).unwrap();
    let failure_data = program.fields().get(wrapper.fields()[0]).unwrap().data();
    let failure = program.records().get(failure).unwrap();
    assert_eq!(failure_data, failure.data());
    assert_eq!(program.data_copyable(wrapper.data()), Some(false));
    let error_data = program.fields().get(failure.fields()[0]).unwrap().data();
    assert_eq!(
        program.data_schema(error_data),
        Some(PayloadSchema::Builtin(BuiltinType::Error))
    );
    assert_eq!(program.data_copyable(error_data), Some(false));
}

#[test]
fn callable_parameters_cannot_shadow_authored_record_names() {
    let symbols = SymbolTable::from_spellings(["A", "f"]);
    let mut reservations = Reservations::new(symbols.clone());
    let a = reservations.reserve_record(symbols.get("A").unwrap(), 0);
    let f = reservations.reserve(symbols.get("f").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    builder.define_record(a, false, []).unwrap();
    builder
        .define(
            f,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [ParameterDefinition {
                name: symbols.get("A").unwrap(),
                data: BuiltinType::I32.into(),
                operation: ValueInputOperation::Read,
            }],
            BuiltinType::Void,
        )
        .unwrap();
    assert_eq!(
        builder.finish().unwrap_err(),
        [DeclarationViolation {
            declaration: DeclarationId::Callable(f),
            position: HeaderPosition::Parameter(0),
            rule: DeclarationRule::ParameterShadowsRecord { declaration: a },
        }]
    );
}

#[test]
fn rejected_copy_fields_publish_neither_tuple_types_nor_partial_schemas() {
    use crate::value_access::{HeaderFailure, TupleData};
    let symbols = SymbolTable::from_spellings(["Plain", "Invalid", "first", "second"]);
    let mut reservations = Reservations::new(symbols.clone());
    let plain = reservations.reserve_record(symbols.get("Plain").unwrap(), 0);
    let invalid = reservations.reserve_record(symbols.get("Invalid").unwrap(), 2);
    let mut builder = reservations.finish().unwrap();
    builder.define_record(plain, false, []).unwrap();
    builder
        .define_record(
            invalid,
            true,
            [
                field(
                    &symbols,
                    "first",
                    DataReference::Tuple(TupleData::new(
                        DataReference::Record(plain),
                        BuiltinType::Bool.into(),
                        [],
                    )),
                ),
                field(&symbols, "second", BuiltinType::Error.into()),
            ],
        )
        .unwrap();
    let mut headers = builder.into_publication().unwrap();
    headers.publish(plain).unwrap();
    let before = headers.snapshot();
    let failure = HeaderFailure::Rejected(
        (0..2)
            .map(|position| DeclarationViolation {
                declaration: DeclarationId::Record(invalid),
                position: HeaderPosition::Field(position),
                rule: DeclarationRule::NonCopyField,
            })
            .collect(),
    );
    assert_eq!(headers.advance(invalid), Err(failure));
    assert!(matches!(
        headers.publish(invalid),
        Err(HeaderFailure::Rejected(_))
    ));
    let after = headers.snapshot();
    assert_eq!(after.records().len(), 1);
    assert!(after.fields().is_empty());
    assert!(!after.header_ready(invalid));
    assert_eq!(after.type_named("Invalid"), None);
    assert_eq!(
        after.types.store().type_count(),
        before.types.store().type_count()
    );
    assert!(after.preserves_headers(&before));
}
