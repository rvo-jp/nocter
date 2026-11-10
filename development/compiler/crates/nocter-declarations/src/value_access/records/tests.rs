use super::*;
use crate::value_access::PayloadSchema;
use crate::value_access::{Builder as ProgramBuilder, ParameterDefinition};
use nocter_model::{CompileTimeGuarantee, ValueInputOperation};

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
    let mut builder = ProgramBuilder::new(symbols.clone());
    let empty = builder.reserve_record(symbols.get("Empty").unwrap());
    let inner = builder.reserve_record(symbols.get("Inner").unwrap());
    let ordinary = builder.reserve_record(symbols.get("Ordinary").unwrap());
    let outer = builder.reserve_record(symbols.get("Outer").unwrap());
    let function = builder.reserve(symbols.get("f").unwrap());
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
    let mut builder = ProgramBuilder::new(symbols.clone());
    let empty = builder.reserve_record(symbols.get("Empty").unwrap());
    let copy_empty = builder.reserve_record(symbols.get("CopyEmpty").unwrap());
    let plain = builder.reserve_record(symbols.get("Plain").unwrap());
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
    let mut builder = ProgramBuilder::new(symbols.clone());
    let a = builder.reserve_record(symbols.get("A").unwrap());
    let b = builder.reserve_record(symbols.get("B").unwrap());
    let plain = builder.reserve_record(symbols.get("Plain").unwrap());
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
    let violations = builder.finish().unwrap_err();
    assert_eq!(
        violations,
        vec![
            DeclarationViolation {
                declaration: DeclarationId::Record(a),
                position: HeaderPosition::Field(0),
                rule: DeclarationRule::NonCopyField
            },
            DeclarationViolation {
                declaration: DeclarationId::Record(a),
                position: HeaderPosition::Field(1),
                rule: DeclarationRule::NonCopyField
            },
            DeclarationViolation {
                declaration: DeclarationId::Record(b),
                position: HeaderPosition::Field(0),
                rule: DeclarationRule::NonCopyField
            },
        ]
    );
}

#[test]
fn self_mutual_and_containing_recursive_payloads_cannot_publish() {
    let symbols = SymbolTable::from_spellings(["A", "B", "C", "Self", "value"]);
    let mut builder = ProgramBuilder::new(symbols.clone());
    let a = builder.reserve_record(symbols.get("A").unwrap());
    let b = builder.reserve_record(symbols.get("B").unwrap());
    let c = builder.reserve_record(symbols.get("C").unwrap());
    let recursive = builder.reserve_record(symbols.get("Self").unwrap());
    for (id, child) in [(a, b), (b, a), (c, a), (recursive, recursive)] {
        builder
            .define_record(
                id,
                false,
                [field(&symbols, "value", DataReference::Record(child))],
            )
            .unwrap();
    }
    let violations = builder.finish().unwrap_err();
    assert_eq!(violations.len(), 4);
    for (violation, (id, through)) in
        violations
            .iter()
            .zip([(a, b), (b, a), (c, a), (recursive, recursive)])
    {
        assert_eq!(violation.declaration, DeclarationId::Record(id));
        assert_eq!(violation.position, HeaderPosition::Field(0));
        assert_eq!(
            violation.rule,
            DeclarationRule::RecursivePayload { through }
        );
    }
}

#[test]
fn missing_or_second_definitions_never_replace_a_valid_schema() {
    let symbols = SymbolTable::from_spellings(["A", "B"]);
    let mut builder = ProgramBuilder::new(symbols.clone());
    let a = builder.reserve_record(symbols.get("A").unwrap());
    let b = builder.reserve_record(symbols.get("B").unwrap());
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
    let mut builder = ProgramBuilder::new(symbols.clone());
    let a = builder.reserve_record(symbols.get("A").unwrap());
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
fn duplicate_record_and_field_names_and_callable_collisions_are_rejected() {
    let symbols = SymbolTable::from_spellings(["A", "f", "value"]);
    let mut builder = ProgramBuilder::new(symbols.clone());
    let a = builder.reserve_record(symbols.get("A").unwrap());
    let duplicate = builder.reserve_record(symbols.get("A").unwrap());
    let collision = builder.reserve_record(symbols.get("f").unwrap());
    let f = builder.reserve(symbols.get("f").unwrap());
    builder
        .define(
            f,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    let value = field(&symbols, "value", DataReference::Builtin(BuiltinType::I32));
    builder
        .define_record(a, false, [value.clone(), value])
        .unwrap();
    builder.define_record(duplicate, false, []).unwrap();
    builder.define_record(collision, false, []).unwrap();
    assert_eq!(
        builder.finish().unwrap_err(),
        [
            DeclarationViolation {
                declaration: DeclarationId::Record(a),
                position: HeaderPosition::Field(1),
                rule: DeclarationRule::DuplicateField { first: 0 }
            },
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
        ]
    );
}

#[test]
fn unsupported_and_unknown_symbols_cannot_enter_frozen_schemas() {
    let symbols = SymbolTable::from_spellings(["A", "i32", "value"]);
    let foreign_symbols = SymbolTable::from_spellings(["a", "b", "c", "d"]);
    let unknown = foreign_symbols.get("d").unwrap();
    let mut builder = ProgramBuilder::new(symbols.clone());
    let a = builder.reserve_record(symbols.get("A").unwrap());
    let builtin = builder.reserve_record(symbols.get("i32").unwrap());
    let absent = builder.reserve_record(unknown);
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
    builder.define_record(builtin, false, []).unwrap();
    builder.define_record(absent, false, []).unwrap();
    let violations = builder.finish().unwrap_err();
    assert_eq!(violations.len(), 4);
    assert!(violations.iter().any(|v| v.rule
        == DeclarationRule::UnsupportedDataType {
            data: BuiltinType::F64
        }));
    assert!(
        violations
            .iter()
            .any(|v| v.rule == DeclarationRule::ReservedBuiltinName)
    );
    assert_eq!(
        violations
            .iter()
            .filter(|v| v.rule == DeclarationRule::UnknownSymbol)
            .count(),
        2
    );
}

#[test]
fn field_names_are_local_and_type_names_do_not_change_field_identity() {
    let symbols = SymbolTable::from_spellings(["A", "B"]);
    let mut builder = ProgramBuilder::new(symbols.clone());
    let a = builder.reserve_record(symbols.get("A").unwrap());
    let b = builder.reserve_record(symbols.get("B").unwrap());
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
    let mut builder = ProgramBuilder::new(symbols.clone());
    let ids: Vec<_> = names
        .iter()
        .map(|name| builder.reserve_record(symbols.get(name).unwrap()))
        .collect();
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
    let program = builder.finish().unwrap();
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
    let mut builder = ProgramBuilder::new(symbols.clone());
    let failure = builder.reserve_record(symbols.get("Failure").unwrap());
    let wrapper = builder.reserve_record(symbols.get("Wrapper").unwrap());
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
    let mut builder = ProgramBuilder::new(symbols.clone());
    let a = builder.reserve_record(symbols.get("A").unwrap());
    let f = builder.reserve(symbols.get("f").unwrap());
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
