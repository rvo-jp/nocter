//! Readiness grants complete schemas rather than provisional identities or field fragments.
use super::*;
use crate::value_access::{CallableKind, DataReference, FieldDefinition, Reservations, TupleData};
use nocter_model::{
    ArenaBuilder, BuiltinType, CompileTimeGuarantee, GenericApplication, NominalTypeId,
};

#[test]
fn selected_records_publish_transitive_schemas_without_exposing_unrelated_fields() {
    let symbols = SymbolTable::from_spellings(["Child", "Parent", "Unused", "value", "run"]);
    let mut reservations = Reservations::new(symbols.clone());
    let child = reservations.reserve_record(symbols.get("Child").unwrap(), 1);
    let parent = reservations.reserve_record(symbols.get("Parent").unwrap(), 1);
    let unused = reservations.reserve_record(symbols.get("Unused").unwrap(), 0);
    let run = reservations.reserve(symbols.get("run").unwrap(), 0);
    let mut builder = reservations.finish().unwrap();
    let field = |data| FieldDefinition {
        name: symbols.get("value").unwrap(),
        data,
    };
    builder
        .define_record(child, true, [field(BuiltinType::I32.into())])
        .unwrap();
    builder
        .define_record(
            parent,
            true,
            [field(DataReference::Tuple(TupleData::new(
                DataReference::Record(child),
                BuiltinType::Bool.into(),
                [],
            )))],
        )
        .unwrap();
    builder.define_record(unused, false, []).unwrap();
    builder
        .define(
            run,
            CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    let mut headers = builder.into_publication().unwrap();
    headers.publish(run).unwrap();
    let early = headers.snapshot();
    assert!(
        !early.headers_complete(),
        "unreferenced records are part of unit closure"
    );
    assert_eq!(
        early.resolve_record(symbols.get("Parent").unwrap()),
        Some(parent)
    );
    let key = early
        .types
        .store()
        .identity(&TypeKind::Nominal {
            definition: parent,
            arguments: GenericApplication::default(),
        })
        .unwrap();
    assert_eq!(early.data_schema(key), None);
    assert_eq!(early.data_copyable(key), None);
    assert_eq!(early.type_named("Parent"), None);
    assert!(early.fields().is_empty());
    headers.publish(parent).unwrap();
    let later = headers.snapshot();
    assert!(later.preserves_headers(&early));
    assert!(!early.preserves_headers(&later));
    assert_eq!(later.records().len(), 2);
    assert_eq!(later.fields().len(), 2);
    assert!(later.records().get(child).is_some());
    assert!(later.records().get(unused).is_none());
    assert_eq!(later.type_named("Parent"), Some(key));
    assert_eq!(later.data_copyable(key), Some(true));
    assert!(!later.headers_complete());
    headers.publish(parent).unwrap();
    assert_eq!(
        headers.snapshot().types.store().type_count(),
        later.types.store().type_count()
    );
    let mut outside = ArenaBuilder::<NominalTypeId, ()>::new();
    for _ in 0..3 {
        outside.insert(());
    }
    let missing = outside.insert(());
    assert_eq!(
        headers.publish(missing),
        Err(crate::DefinitionError::UnknownId)
    );
    assert_eq!(headers.snapshot().records().len(), 2);
    let complete = headers.finish();
    assert!(complete.headers_complete());
    assert!(complete.preserves_headers(&later));
    assert!(early.records().is_empty(), "an old view stays immutable");
}

#[test]
fn callable_contracts_close_record_data_in_nested_callback_slots() {
    let symbols = SymbolTable::from_spellings(["Data", "run", "callback"]);
    let mut reservations = Reservations::new(symbols.clone());
    let record = reservations.reserve_record(symbols.get("Data").unwrap(), 0);
    let run = reservations.reserve(symbols.get("run").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    builder.define_record(record, false, []).unwrap();
    let contract = super::super::FunctionDefinition {
        invocation: ValueInvocation::Read,
        inputs: Box::new([(
            nocter_model::ValueInputOperation::Read,
            DataReference::Record(record),
        )]),
        result: DataReference::Tuple(TupleData::new(
            DataReference::Record(record),
            BuiltinType::Bool.into(),
            [],
        )),
    };
    builder
        .define(
            run,
            CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [super::super::ParameterDefinition {
                name: symbols.get("callback").unwrap(),
                operation: nocter_model::ValueInputOperation::Read,
                data: ValueDefinition::Function(contract.clone()),
            }],
            ValueDefinition::Function(contract),
        )
        .unwrap();
    let mut headers = builder.into_publication().unwrap();
    headers.publish(run).unwrap();
    let program = headers.snapshot();
    assert!(program.headers_complete());
    let callback = program
        .parameters()
        .iter()
        .next()
        .unwrap()
        .1
        .contract()
        .unwrap();
    assert!(program.data_schema(callback.inputs[0].data).is_some());
    assert!(program.data_schema(callback.result).is_some());
    assert_eq!(
        program.callables().get(run).unwrap().result_contract(),
        Some(callback)
    );
}
