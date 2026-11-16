//! Field contract publication grants type identity, never a function value or Copy proof.
use super::*;
use crate::value_access::{FieldDefinition, FunctionDefinition, FunctionWitness, PayloadSchema};
use nocter_model::{
    ValueCallableContract, ValueContractOwner, ValueFunctionOrigin, ValueInvocation,
};
fn callback(result: DataReference) -> DataReference {
    DataReference::Function(Box::new(FunctionDefinition {
        invocation: ValueInvocation::Read,
        inputs: Box::new([]),
        result,
    }))
}
fn witness(program: &Program, data: nocter_model::TypeId) -> FunctionWitness {
    let Some(PayloadSchema::Function(selected)) = program.data_schema(data) else {
        panic!("field contract")
    };
    selected
}
#[test]
fn field_contracts_keep_equal_signatures_distinct_and_nested_contracts_canonical() {
    let symbols = SymbolTable::from_spellings(["Handler", "first", "second", "nested"]);
    let mut reservations = Reservations::new(symbols.clone());
    let record = reservations.reserve_record(symbols.get("Handler").unwrap(), 3);
    let mut builder = reservations.finish().unwrap();
    builder
        .define_record(
            record,
            false,
            [
                FieldDefinition {
                    name: symbols.get("first").unwrap(),
                    data: callback(BuiltinType::I32.into()),
                },
                FieldDefinition {
                    name: symbols.get("second").unwrap(),
                    data: callback(BuiltinType::I32.into()),
                },
                FieldDefinition {
                    name: symbols.get("nested").unwrap(),
                    data: callback(callback(BuiltinType::I32.into())),
                },
            ],
        )
        .unwrap();
    let program = builder.finish().unwrap();
    let fields = program.records().get(record).unwrap().fields();
    let keys = fields
        .iter()
        .map(|&id| witness(&program, program.fields().get(id).unwrap().data()))
        .collect::<Vec<_>>();
    assert_ne!(keys[0], keys[1]);
    assert_eq!(
        program.function_contract(keys[0]),
        program.function_contract(keys[1])
    );
    for index in [0, 1] {
        assert_eq!(
            keys[index].origin,
            ValueFunctionOrigin::Contract(ValueCallableContract {
                owner: ValueContractOwner::Field(fields[index]),
                ordinal: 0
            })
        );
        assert!(program.concrete_witness(keys[index]).is_err());
    }
    let inner = program.function_contract(keys[2]).unwrap().result;
    assert_eq!(
        witness(&program, inner).origin,
        ValueFunctionOrigin::Contract(ValueCallableContract {
            owner: ValueContractOwner::Field(fields[2]),
            ordinal: 0
        })
    );
    assert_eq!(
        keys[2].origin,
        ValueFunctionOrigin::Contract(ValueCallableContract {
            owner: ValueContractOwner::Field(fields[2]),
            ordinal: 1
        })
    );
    let data = program.records().get(record).unwrap().data();
    assert_eq!(program.data_copyable(data), Some(false));
    assert!(program.instantiate_closed_data(data).is_err());
    assert!(program.parameters().is_empty());
}
#[test]
fn callable_signatures_do_not_create_record_payload_cycles() {
    let symbols = SymbolTable::from_spellings(["Handler", "next"]);
    let mut reservations = Reservations::new(symbols.clone());
    let record = reservations.reserve_record(symbols.get("Handler").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    builder
        .define_record(
            record,
            false,
            [FieldDefinition {
                name: symbols.get("next").unwrap(),
                data: callback(DataReference::Record(record)),
            }],
        )
        .unwrap();
    let mut publication = builder.into_publication();
    assert_eq!(publication.advance(record).unwrap(), HeaderStep::Ready);
    let program = publication.snapshot();
    let field = program.records().get(record).unwrap().fields()[0];
    let selected = witness(&program, program.fields().get(field).unwrap().data());
    assert_eq!(
        program.function_contract(selected).unwrap().result,
        program.records().get(record).unwrap().data()
    );
    assert!(program.concrete_witness(selected).is_err());
}
#[test]
fn function_fields_reject_copy_before_publishing_any_field_type_or_contract() {
    for zero in [false, true] {
        let symbols = SymbolTable::from_spellings(["Handler", "action"]);
        let mut reservations = Reservations::new(symbols.clone());
        let record = reservations.reserve_record(symbols.get("Handler").unwrap(), 1);
        let mut builder = reservations.finish().unwrap();
        let data = DataReference::Array {
            element: Box::new(callback(BuiltinType::I32.into())),
            length: u64::from(!zero),
        };
        builder
            .define_record(
                record,
                true,
                [FieldDefinition {
                    name: symbols.get("action").unwrap(),
                    data,
                }],
            )
            .unwrap();
        let mut publication = builder.into_publication();
        let before = publication.snapshot();
        if zero {
            publication.publish(record).unwrap();
            assert!(
                publication
                    .snapshot()
                    .records()
                    .get(record)
                    .unwrap()
                    .copyable()
            );
        } else {
            assert!(
                matches!(publication.publish(record), Err(HeaderFailure::Rejected(rules)) if rules.iter().any(|rule| rule.rule == DeclarationRule::NonCopyField))
            );
            let after = publication.snapshot();
            assert!(after.records().get(record).is_none());
            assert!(after.fields().is_empty());
            assert!(after.callable_contracts.is_empty());
            assert_eq!(
                after.types.store().type_count(),
                before.types.store().type_count()
            );
            assert!(after.preserves_headers(&before));
        }
    }
}

#[test]
fn field_contracts_survive_body_replay_without_overriding_the_field_owner() {
    let symbols = SymbolTable::from_spellings(["Handler", "callback", "run"]);
    let mut reservations = Reservations::new(symbols.clone());
    let record = reservations.reserve_record(symbols.get("Handler").unwrap(), 1);
    let run = reservations.reserve(symbols.get("run").unwrap(), 0);
    let mut builder = reservations.finish().unwrap();
    builder
        .define_record(
            record,
            false,
            [FieldDefinition {
                name: symbols.get("callback").unwrap(),
                data: callback(BuiltinType::I32.into()),
            }],
        )
        .unwrap();
    builder
        .define(
            run,
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    let headers = builder.finish().unwrap();
    let mut branch = headers.body_types(run).unwrap();
    let integer = headers.data_type(BuiltinType::I32).unwrap();
    branch
        .callable_contract(
            0,
            crate::value_access::FunctionConstraint {
                invocation: ValueInvocation::Read,
                inputs: Box::new([]),
                result: integer,
            },
        )
        .unwrap();
    let branch = branch.finish();
    let canonical = headers
        .canonical_body_types(std::collections::BTreeMap::from([(run.into(), &branch)]))
        .unwrap()
        .finish();
    assert!(canonical.preserves_headers(&headers));
    let field = headers.records().get(record).unwrap().fields()[0];
    let selected = witness(&headers, headers.fields().get(field).unwrap().data());
    assert_eq!(
        canonical.function_contract(selected),
        headers.function_contract(selected)
    );
    let resolved = canonical
        .resolve_body_types(&std::collections::BTreeMap::new())
        .unwrap();
    assert!(resolved.program().preserves_headers(&headers));
    assert_eq!(
        resolved.program().function_contract(selected),
        headers.function_contract(selected)
    );
    let mut forged = branch.clone();
    let ValueFunctionOrigin::Contract(id) = selected.origin else {
        panic!("field contract")
    };
    forged.callable_contracts.insert(
        id,
        crate::value_access::FunctionConstraint {
            invocation: ValueInvocation::Read,
            inputs: Box::new([]),
            result: headers.data_type(BuiltinType::Bool).unwrap(),
        },
    );
    assert_eq!(
        forged.function_contract(selected),
        headers.function_contract(selected)
    );
    assert!(
        headers
            .canonical_body_types(std::collections::BTreeMap::from([(run.into(), &forged)]))
            .is_err()
    );
}

#[test]
fn invalid_field_contract_signatures_publish_no_field_or_type() {
    let symbols = SymbolTable::from_spellings(["Handler", "callback"]);
    let mut reservations = Reservations::new(symbols.clone());
    let record = reservations.reserve_record(symbols.get("Handler").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    builder
        .define_record(
            record,
            false,
            [FieldDefinition {
                name: symbols.get("callback").unwrap(),
                data: callback(BuiltinType::F64.into()),
            }],
        )
        .unwrap();
    let mut publication = builder.into_publication();
    let before = publication.snapshot();
    assert!(
        matches!(publication.publish(record), Err(HeaderFailure::Rejected(rules)) if rules.iter().any(|rule| matches!(rule.rule, DeclarationRule::UnsupportedDataType { .. })))
    );
    let after = publication.snapshot();
    assert!(after.fields().is_empty());
    assert!(after.callable_contracts.is_empty());
    assert_eq!(
        after.types.store().type_count(),
        before.types.store().type_count()
    );
}
