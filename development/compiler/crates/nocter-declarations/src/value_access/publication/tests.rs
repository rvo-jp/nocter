//! Readiness grants complete schemas rather than provisional identities or field fragments.
use super::*;
use crate::value_access::{CodeKind, DataReference, FieldDefinition, Reservations, TupleData};
use nocter_model::{
    ArenaBuilder, BuiltinType, CompileTimeGuarantee, GenericApplication, NominalTypeId, SymbolTable,
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
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    let mut headers = builder.into_publication();
    headers.publish(run).unwrap();
    let early = headers.snapshot();
    assert!(early.header_ready(run));
    assert!(!early.header_ready(parent));
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
    assert_eq!(early.record_identity(key), Some(parent));
    assert_eq!(
        early.declaration_label(parent.into()).as_deref(),
        Some("Parent")
    );
    assert_eq!(early.data_copyable(key), None);
    assert_eq!(early.type_named("Parent"), None);
    assert_eq!(early.type_identity_named("Parent"), Some(key));
    assert_eq!(early.type_identity_named("Missing"), None);
    assert_eq!(early.type_identity_named("run"), None);
    assert!(early.fields().is_empty());
    let type_count = early.types.store().type_count();
    assert_eq!(
        headers.advance(parent).unwrap(),
        HeaderStep::Require(Box::new([child.into()]))
    );
    let waiting = headers.snapshot();
    assert!(waiting.records().is_empty());
    assert!(waiting.fields().is_empty());
    assert_eq!(waiting.types.store().type_count(), type_count);
    assert_eq!(headers.advance(child).unwrap(), HeaderStep::Ready);
    assert_eq!(headers.advance(parent).unwrap(), HeaderStep::Ready);
    let later = headers.snapshot();
    assert!(later.header_ready(parent));
    assert!(later.header_ready(child));
    assert!(!later.header_ready(unused));
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
    let complete = headers.finish().unwrap();
    assert!(complete.headers_complete());
    assert!(complete.preserves_headers(&later));
    assert!(early.records().is_empty(), "an old view stays immutable");
}

#[test]
fn unknown_header_steps_preserve_every_ready_contract() {
    let symbols = SymbolTable::from_spellings(["Empty"]);
    let mut reservations = Reservations::new(symbols.clone());
    let record = reservations.reserve_record(symbols.get("Empty").unwrap(), 0);
    let mut builder = reservations.finish().unwrap();
    builder.define_record(record, false, []).unwrap();
    let mut headers = builder.into_publication();
    assert_eq!(headers.advance(record).unwrap(), HeaderStep::Ready);
    let before = headers.snapshot();
    let mut outside = ArenaBuilder::<NominalTypeId, ()>::new();
    outside.insert(());
    let missing = outside.insert(());
    assert_eq!(
        headers.advance(missing),
        Err(super::HeaderFailure::UnknownId)
    );
    assert_eq!(
        headers.publish(missing),
        Err(super::HeaderFailure::UnknownId)
    );
    let after = headers.snapshot();
    assert!(after.headers_complete());
    assert!(after.preserves_headers(&before));
    assert_eq!(after.records(), before.records());
    assert_eq!(
        after.types.store().type_count(),
        before.types.store().type_count()
    );
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
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [super::super::ParameterDefinition {
                name: symbols.get("callback").unwrap(),
                operation: nocter_model::ValueInputOperation::Read,
                data: ValueDefinition::Function(contract.clone()),
            }],
            ValueDefinition::Function(contract),
        )
        .unwrap();
    let mut headers = builder.into_publication();
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
        program.codes().get(run).unwrap().result_contract(),
        Some(callback)
    );
}

#[test]
fn missing_definitions_can_be_loaded_without_changing_slots_or_established_contracts() {
    use crate::value_access::{DeclarationRule, DeclarationViolation, HeaderPosition};
    let symbols = SymbolTable::from_spellings(["Leaf", "Parent", "run", "value"]);
    let mut reservations = Reservations::new(symbols.clone());
    let leaf = reservations.reserve_record(symbols.get("Leaf").unwrap(), 1);
    let parent = reservations.reserve_record(symbols.get("Parent").unwrap(), 1);
    let run = reservations.reserve(symbols.get("run").unwrap(), 0);
    let builder = reservations.finish().unwrap();
    let leaf_fields = builder.field_ids(leaf).unwrap().to_vec();
    let parent_fields = builder.field_ids(parent).unwrap().to_vec();
    let mut headers = builder.into_publication();
    headers
        .define(
            run,
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::I32,
        )
        .unwrap();
    headers.publish(run).unwrap();
    let early = headers.snapshot();
    assert_eq!(headers.is_defined(leaf), Some(false));
    assert_eq!(early.type_named("Leaf"), None);
    let child_data = headers.resolve_data(symbols.get("Leaf").unwrap()).unwrap();
    headers
        .define_record(
            parent,
            true,
            [FieldDefinition {
                name: symbols.get("value").unwrap(),
                data: child_data,
            }],
        )
        .unwrap();
    assert_eq!(
        headers.publish(parent),
        Err(HeaderFailure::Rejected(vec![DeclarationViolation {
            declaration: leaf.into(),
            position: HeaderPosition::Name,
            rule: DeclarationRule::MissingDefinition,
        }]))
    );
    assert!(headers.snapshot().preserves_headers(&early));
    assert!(headers.snapshot().records().is_empty());
    assert_eq!(
        headers.define_record(leaf, true, []),
        Err(crate::DefinitionError::WrongArity {
            expected: 1,
            actual: 0
        })
    );
    assert_eq!(headers.is_defined(leaf), Some(false));
    headers
        .define_record(
            leaf,
            true,
            [FieldDefinition {
                name: symbols.get("value").unwrap(),
                data: BuiltinType::Bool.into(),
            }],
        )
        .unwrap();
    headers.publish(parent).unwrap();
    assert_eq!(
        headers.define_record(leaf, false, []),
        Err(crate::DefinitionError::AlreadyDefined)
    );
    let complete = headers.finish().unwrap();
    assert!(complete.preserves_headers(&early));
    assert!(complete.headers_complete());
    assert_eq!(complete.records().get(leaf).unwrap().fields(), leaf_fields);
    assert_eq!(
        complete.records().get(parent).unwrap().fields(),
        parent_fields
    );
    assert!(complete.records().get(parent).unwrap().copyable());
}
