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

#[test]
fn scalar_enums_share_nominal_reservation_and_publish_variants_in_order() {
    let symbols = SymbolTable::from_spellings(["Container", "Status", "busy", "idle", "state"]);
    let name = |spelling| symbols.get(spelling).unwrap();
    let mut reservations = Reservations::new(symbols.clone());
    let container = reservations.reserve_record(name("Container"), 1);
    let status = reservations
        .reserve_enum_in(ScopeId::ROOT, name("Status"), [0, 0])
        .unwrap();
    assert_ne!(container, status);
    let mut headers = reservations.finish().unwrap().into_publication();
    headers
        .define_record(
            container,
            true,
            [field(&symbols, "state", DataReference::Nominal(status))],
        )
        .unwrap();
    headers
        .define_enum(
            status,
            true,
            [name("idle"), name("busy")].map(VariantDefinition::scalar),
        )
        .unwrap();
    let waiting = headers.snapshot();
    assert!(waiting.enumeration(status).is_none() && waiting.variants().is_empty());
    let status_data = waiting.nominal_type_identity(status).unwrap();

    headers.publish(container).unwrap();
    let ready = headers.snapshot();
    assert!(ready.preserves_headers(&waiting));
    let status_schema = ready.enumeration(status).unwrap();
    assert_eq!(status_schema.data(), status_data);
    assert_eq!(ready.nominal_identity(status_data), Some(status));
    assert!(ready.record(container).unwrap().copyable());
    assert!(ready.record(status).is_none() && ready.enumeration(container).is_none());
    for (position, (variant, spelling)) in status_schema
        .variants()
        .iter()
        .zip(["idle", "busy"])
        .enumerate()
    {
        assert_eq!(
            status_schema.resolve_variant(name(spelling)),
            Some(*variant)
        );
        let entry = ready.variants().get(*variant).unwrap();
        assert_eq!(
            (entry.owner(), entry.name(), entry.position()),
            (status, name(spelling), position)
        );
    }
}

#[test]
fn invalid_enum_definitions_publish_no_nominal_or_variant_schema() {
    use crate::value_access::HeaderFailure;
    let symbols = SymbolTable::from_spellings(["Status", "idle"]);
    let idle = symbols.get("idle").unwrap();
    for (names, expected) in [
        (vec![], DeclarationRule::EmptyEnumeration),
        (
            vec![idle, idle],
            DeclarationRule::DuplicateVariant { first: 0 },
        ),
    ] {
        let mut reservations = Reservations::new(symbols.clone());
        let status = reservations
            .reserve_enum_in(
                ScopeId::ROOT,
                symbols.get("Status").unwrap(),
                std::iter::repeat_n(0, names.len()),
            )
            .unwrap();
        let mut headers = reservations.finish().unwrap().into_publication();
        let before = headers.snapshot();
        let variants: Vec<_> = names.into_iter().map(VariantDefinition::scalar).collect();
        headers.define_enum(status, true, variants).unwrap();
        let HeaderFailure::Rejected(violations) = headers.publish(status).unwrap_err() else {
            panic!("invalid enum schema");
        };
        assert_eq!(violations[0].rule, expected);
        let after = headers.snapshot();
        assert!(after.preserves_headers(&before));
        assert!(after.nominals().is_empty() && after.variants().is_empty());
    }
}

#[test]
fn opaque_scalar_enum_does_not_give_copy_to_a_containing_record() {
    use super::super::HeaderFailure;
    let symbols = SymbolTable::from_spellings(["Container", "Status", "idle", "state"]);
    let name = |spelling| symbols.get(spelling).unwrap();
    let mut reservations = Reservations::new(symbols.clone());
    let status = reservations
        .reserve_enum_in(ScopeId::ROOT, name("Status"), [0])
        .unwrap();
    let container = reservations.reserve_record(name("Container"), 1);
    let mut headers = reservations.finish().unwrap().into_publication();
    headers
        .define_enum(status, false, [VariantDefinition::scalar(name("idle"))])
        .unwrap();
    headers
        .define_record(
            container,
            true,
            [field(&symbols, "state", DataReference::Nominal(status))],
        )
        .unwrap();
    let HeaderFailure::Rejected(violations) = headers.publish(container).unwrap_err() else {
        panic!("enum without Copy cannot populate a Copy record");
    };
    assert_eq!(violations[0].rule, DeclarationRule::NonCopyField);
    let program = headers.snapshot();
    assert!(!program.enumeration(status).unwrap().copyable());
    assert!(program.record(container).is_none());
}

#[test]
fn enum_payload_fields_and_records_share_nominal_publication() {
    let symbols = SymbolTable::from_spellings([
        "Envelope", "Event", "Point", "none", "number", "point", "value", "x",
    ]);
    let name = |spelling| symbols.get(spelling).unwrap();
    let mut reservations = Reservations::new(symbols.clone());
    let envelope = reservations.reserve_record(name("Envelope"), 1);
    let event = reservations
        .reserve_enum_in(ScopeId::ROOT, name("Event"), [0, 1, 1])
        .unwrap();
    let point = reservations.reserve_record(name("Point"), 1);
    let mut headers = reservations.finish().unwrap().into_publication();
    headers
        .define_record(
            envelope,
            true,
            [field(&symbols, "value", DataReference::Nominal(event))],
        )
        .unwrap();
    headers
        .define_record(point, true, [field(&symbols, "x", BuiltinType::I32.into())])
        .unwrap();
    headers
        .define_enum(
            event,
            true,
            [
                VariantDefinition::scalar(name("none")),
                VariantDefinition {
                    name: name("number"),
                    fields: Box::new([field(&symbols, "value", BuiltinType::I32.into())]),
                },
                VariantDefinition {
                    name: name("point"),
                    fields: Box::new([field(&symbols, "value", DataReference::Nominal(point))]),
                },
            ],
        )
        .unwrap();
    headers.publish(envelope).unwrap();
    let program = headers.snapshot();
    let schema = program.enumeration(event).unwrap();
    assert!(schema.has_payload() && schema.copyable());
    let envelope = program.record(envelope).unwrap();
    assert!(envelope.copyable());
    assert!(program.owned_payload_supported(schema.data()));
    assert!(program.owned_payload_supported(envelope.data()));
    let &[empty, number, compound] = schema.variants() else {
        panic!("declaration-order variants");
    };
    assert!(program.variants().get(empty).unwrap().fields().is_empty());
    let get_field = |id| {
        let [field] = program.variants().get(id).unwrap().fields() else {
            panic!("one payload field");
        };
        program.fields().get(*field).unwrap()
    };
    let number_field = get_field(number);
    let compound_field = get_field(compound);
    assert_eq!(number_field.name(), compound_field.name());
    assert_eq!(number_field.variant(), Some(number));
    assert_eq!(compound_field.variant(), Some(compound));
    assert_eq!(compound_field.position(), 0);
    assert_eq!(compound_field.data(), program.record(point).unwrap().data());
}

#[test]
fn invalid_payload_arity_and_copy_leave_the_enum_unpublished() {
    use crate::value_access::HeaderFailure;
    let symbols = SymbolTable::from_spellings(["Event", "empty", "report", "reason"]);
    let name = |spelling| symbols.get(spelling).unwrap();
    let mut reservations = Reservations::new(symbols.clone());
    let event = reservations
        .reserve_enum_in(ScopeId::ROOT, name("Event"), [0, 1])
        .unwrap();
    let mut headers = reservations.finish().unwrap().into_publication();
    let before = headers.snapshot();
    assert!(matches!(
        headers.define_enum(
            event,
            true,
            [name("empty"), name("report")].map(VariantDefinition::scalar)
        ),
        Err(DefinitionError::WrongArity {
            expected: 1,
            actual: 0
        })
    ));
    headers
        .define_enum(
            event,
            true,
            [
                VariantDefinition::scalar(name("empty")),
                VariantDefinition {
                    name: name("report"),
                    fields: Box::new([field(&symbols, "reason", BuiltinType::Error.into())]),
                },
            ],
        )
        .unwrap();
    let HeaderFailure::Rejected(violations) = headers.publish(event).unwrap_err() else {
        panic!("non-Copy payload invalidates the requested contract");
    };
    assert_eq!(
        violations,
        [DeclarationViolation {
            declaration: event.into(),
            position: HeaderPosition::VariantField {
                variant: 1,
                field: 0
            },
            rule: DeclarationRule::NonCopyField,
        }]
    );
    let after = headers.snapshot();
    assert!(after.preserves_headers(&before));
    assert!(
        after.nominals().is_empty() && after.fields().is_empty() && after.variants().is_empty()
    );
}

#[test]
fn mixed_nominal_cycle_is_reported_at_the_enum_payload_that_closes_it() {
    use crate::value_access::HeaderFailure;
    let symbols = SymbolTable::from_spellings(["Entry", "Link", "end", "more", "next"]);
    let name = |spelling| symbols.get(spelling).unwrap();
    let mut reservations = Reservations::new(symbols.clone());
    let entry = reservations.reserve_record(name("Entry"), 1);
    let link = reservations
        .reserve_enum_in(ScopeId::ROOT, name("Link"), [0, 1])
        .unwrap();
    let mut headers = reservations.finish().unwrap().into_publication();
    headers
        .define_record(
            entry,
            false,
            [field(&symbols, "next", DataReference::Nominal(link))],
        )
        .unwrap();
    headers
        .define_enum(
            link,
            false,
            [
                VariantDefinition::scalar(name("end")),
                VariantDefinition {
                    name: name("more"),
                    fields: Box::new([field(&symbols, "next", DataReference::Nominal(entry))]),
                },
            ],
        )
        .unwrap();
    let HeaderFailure::Rejected(violations) = headers.publish(entry).unwrap_err() else {
        panic!("mixed direct containment must be finite");
    };
    assert_eq!(
        violations,
        [DeclarationViolation {
            declaration: link.into(),
            position: HeaderPosition::VariantField {
                variant: 1,
                field: 0
            },
            rule: DeclarationRule::RecursivePayload { through: entry },
        }]
    );
    let program = headers.snapshot();
    assert!(program.nominals().is_empty() && program.variants().is_empty());
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
                    field(&symbols, "left", DataReference::Nominal(inner)),
                    field(&symbols, "right", DataReference::Nominal(inner)),
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
            crate::value_access::CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [ParameterDefinition {
                binding: crate::value_access::ParameterBinding::Name(symbols.get("input").unwrap()),
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
    assert_eq!(a.nominals(), b.nominals());
    assert_eq!(a.fields(), b.fields());
    assert_eq!(a.codes(), b.codes());
    assert_eq!(a.parameters(), b.parameters());
    let outer = a
        .resolve_nominal(a.symbols().get("Outer").unwrap())
        .unwrap();
    let record = a.record(outer).unwrap();
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
        let f = a.fields().get(id).unwrap();
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
        let data = program.record(id).unwrap().data();
        assert_eq!(program.data_copyable(data), Some(expected));
    }
    assert_ne!(
        program.record(empty).unwrap().data(),
        program.record(copy_empty).unwrap().data()
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
                field(&symbols, "first", DataReference::Nominal(b)),
                field(&symbols, "second", DataReference::Nominal(plain)),
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
    let mut headers = builder.into_publication();
    let before = headers.snapshot();
    let expected = super::super::HeaderFailure::Rejected(vec![DeclarationViolation {
        declaration: DeclarationId::Nominal(b),
        position: HeaderPosition::Field(0),
        rule: DeclarationRule::NonCopyField,
    }]);
    // The invalid child cannot publish as non-Copy, or make A produce a secondary Copy failure.
    assert_eq!(headers.publish(a), Err(expected));
    let after = headers.snapshot();
    assert!(after.nominals().is_empty());
    assert!(after.fields().is_empty());
    assert_eq!(
        after.types.store().type_count(),
        before.types.store().type_count()
    );
    assert!(after.preserves_headers(&before));
    // An independent ordinary non-Copy record remains a valid dependency after the failure.
    headers.publish(plain).unwrap();
    assert!(!headers.snapshot().record(plain).unwrap().copyable());
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
                [field(&symbols, "value", DataReference::Nominal(child))],
            )
            .unwrap();
    }
    let mut headers = builder.into_publication();
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
                    declaration: DeclarationId::Nominal(owner),
                    position: HeaderPosition::Field(0),
                    rule: DeclarationRule::RecursivePayload { through },
                }
            ]))
        );
        assert!(headers.snapshot().nominals().is_empty());
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
            declaration: DeclarationId::Nominal(b),
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
            [field(&symbols, "value", DataReference::Nominal(absent))],
        )
        .unwrap();
    assert_eq!(
        builder.finish().unwrap_err(),
        [DeclarationViolation {
            declaration: DeclarationId::Nominal(a),
            position: HeaderPosition::Field(0),
            rule: DeclarationRule::UnknownRecordType { record: absent }
        }]
    );
}

#[test]
fn duplicate_records_fail_independently_of_the_value_namespace() {
    let symbols = SymbolTable::from_spellings(["A", "f"]);
    let mut reservations = Reservations::new(symbols.clone());
    let a = reservations.reserve_record(symbols.get("A").unwrap(), 0);
    let duplicate = reservations.reserve_record(symbols.get("A").unwrap(), 0);
    let collision = reservations.reserve_record(symbols.get("f").unwrap(), 0);
    let repeated_collision = reservations.reserve_record(symbols.get("f").unwrap(), 0);
    let _function = reservations.reserve(symbols.get("f").unwrap(), 0);
    assert_eq!(
        reservations.finish().unwrap_err(),
        [
            DeclarationViolation {
                declaration: DeclarationId::Nominal(duplicate),
                position: HeaderPosition::Name,
                rule: DeclarationRule::DuplicateRecord { first: a }
            },
            DeclarationViolation {
                declaration: DeclarationId::Nominal(repeated_collision),
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
            declaration: DeclarationId::Nominal(a),
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
                declaration: DeclarationId::Nominal(builtin),
                position: HeaderPosition::Name,
                rule: DeclarationRule::ReservedBuiltinName,
            },
            DeclarationViolation {
                declaration: DeclarationId::Nominal(absent),
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
                declaration: DeclarationId::Nominal(a),
                position: HeaderPosition::Field(0),
                rule: DeclarationRule::UnknownSymbol,
            },
            DeclarationViolation {
                declaration: DeclarationId::Nominal(a),
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
    let a = program.record(a).unwrap();
    let b = program.record(b).unwrap();
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
                    field(&symbols, "left", DataReference::Nominal(pair[0])),
                    field(&symbols, "right", DataReference::Nominal(pair[0])),
                ],
            )
            .unwrap();
    }
    let mut headers = builder.into_publication();
    let early = headers.snapshot();
    assert!(early.nominals().is_empty());
    assert!(early.fields().is_empty());
    assert!(!early.headers_complete());
    headers.publish(*ids.last().unwrap()).unwrap();
    let program = headers.snapshot();
    assert!(program.headers_complete());
    assert!(program.preserves_headers(&early));
    assert_eq!(program.nominals().len(), COUNT);
    assert_eq!(program.fields().len(), 2 * (COUNT - 1));
    for &id in &ids {
        assert_eq!(
            program.data_copyable(program.record(id).unwrap().data()),
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
            [field(&symbols, "value", DataReference::Nominal(failure))],
        )
        .unwrap();
    let program = builder.finish().unwrap();
    let wrapper = program.record(wrapper).unwrap();
    let failure_data = program.fields().get(wrapper.fields()[0]).unwrap().data();
    let failure = program.record(failure).unwrap();
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
fn callable_parameters_do_not_collide_with_the_type_namespace() {
    let symbols = SymbolTable::from_spellings(["A", "f"]);
    let mut reservations = Reservations::new(symbols.clone());
    let a = reservations.reserve_record(symbols.get("A").unwrap(), 0);
    let f = reservations.reserve(symbols.get("f").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    builder.define_record(a, false, []).unwrap();
    builder
        .define(
            f,
            crate::value_access::CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [ParameterDefinition {
                binding: crate::value_access::ParameterBinding::Name(symbols.get("A").unwrap()),
                data: BuiltinType::I32.into(),
                operation: ValueInputOperation::Read,
            }],
            BuiltinType::Void,
        )
        .unwrap();
    let program = builder.finish().unwrap();
    assert_eq!(program.resolve_nominal(symbols.get("A").unwrap()), Some(a));
    assert_eq!(program.resolve(symbols.get("f").unwrap()), Some(f));
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
                        DataReference::Nominal(plain),
                        BuiltinType::Bool.into(),
                        [],
                    )),
                ),
                field(&symbols, "second", BuiltinType::Error.into()),
            ],
        )
        .unwrap();
    let mut headers = builder.into_publication();
    headers.publish(plain).unwrap();
    let before = headers.snapshot();
    let failure = HeaderFailure::Rejected(
        (0..2)
            .map(|position| DeclarationViolation {
                declaration: DeclarationId::Nominal(invalid),
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
    assert_eq!(after.nominals().len(), 1);
    assert!(after.fields().is_empty());
    assert!(!after.header_ready(invalid));
    assert_eq!(after.type_named("Invalid"), None);
    assert_eq!(
        after.types.store().type_count(),
        before.types.store().type_count()
    );
    assert!(after.preserves_headers(&before));
}
