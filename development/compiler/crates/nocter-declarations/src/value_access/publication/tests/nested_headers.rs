//! Header signature children are immutable type contracts, never additional values or inputs.
use super::*;
use crate::value_access::{FunctionDefinition, FunctionWitness, PayloadReadiness, PayloadSchema};
use nocter_model::{ValueContractOwner, ValueFunctionOrigin, ValueInputOperation};

fn signature(result: DataReference) -> FunctionDefinition {
    FunctionDefinition {
        invocation: ValueInvocation::Read,
        inputs: Box::new([(ValueInputOperation::Read, BuiltinType::I32.into())]),
        result,
    }
}
fn callable(result: DataReference) -> DataReference {
    DataReference::Function(Box::new(signature(result)))
}
fn witness(program: &Program, data: nocter_model::TypeId) -> FunctionWitness {
    let Some(PayloadSchema::Function(witness)) = program.data_schema(data) else {
        panic!("callable signature")
    };
    witness
}

fn contract_child(
    program: &Program,
    owner: ValueContractOwner,
    expected: &FunctionConstraint,
) -> nocter_model::TypeId {
    let child = program
        .function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Contract(nocter_model::ValueCallableContract {
                owner,
                ordinal: 0,
            }),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    assert_eq!(
        program.function_contract(witness(program, child)),
        Some(expected.clone())
    );
    assert!(program.instantiate_closed_data(child).is_err());
    assert!(program.concrete_witness(witness(program, child)).is_err());
    assert_eq!(
        program.payload_readiness(child),
        PayloadReadiness::Unsupported
    );
    child
}

#[test]
fn nested_header_contracts_preserve_owners_reconciliation_and_body_replay() {
    for compound in [false, true] {
        let symbols = SymbolTable::from_spellings(["first", "second", "action"]);
        let mut reservations = Reservations::new(symbols.clone());
        let ids =
            ["first", "second"].map(|name| reservations.reserve(symbols.get(name).unwrap(), 1));
        let mut builder = reservations.finish().unwrap();
        let nested = signature(callable(BuiltinType::I32.into()));
        let recipe = if compound {
            DataReference::Array {
                element: Box::new(DataReference::Function(Box::new(nested))),
                length: 0,
            }
            .into()
        } else {
            crate::value_access::ValueDefinition::Function(nested)
        };
        for id in ids {
            builder
                .define(
                    id,
                    CodeKind::Function(CompileTimeGuarantee::Evaluatable),
                    [ParameterDefinition {
                        name: symbols.get("action").unwrap(),
                        operation: ValueInputOperation::Take,
                        data: recipe.clone(),
                    }],
                    recipe.clone(),
                )
                .unwrap();
        }
        let mut publication = builder.into_publication();
        publication.publish(ids[1]).unwrap();
        let early = publication.snapshot();
        let input = early.codes().get(ids[1]).unwrap().parameters()[0];
        let before = early.types.store().type_count();
        assert_eq!(
            early.existing_input_value_reference(input, &recipe),
            Some(early.parameters().get(input).unwrap().data())
        );
        publication.publish(ids[0]).unwrap();
        let headers = publication.snapshot();
        assert!(headers.preserves_headers(&early));
        assert_eq!(early.types.store().type_count(), before);
        assert_eq!(headers.parameters().len(), 2);
        let mut children = Vec::new();
        for id in ids {
            let code = headers.codes().get(id).unwrap();
            let parameter = headers.parameters().get(code.parameters()[0]).unwrap();
            assert_eq!(
                headers.existing_input_value_reference(code.parameters()[0], &recipe),
                Some(parameter.data())
            );
            assert_eq!(
                headers.existing_result_value_reference(id, &recipe),
                Some(code.result())
            );
            for (contracts, owner) in [
                (
                    &parameter.type_contracts,
                    ValueContractOwner::Parameter(code.parameters()[0]),
                ),
                (&code.type_contracts, ValueContractOwner::Result(id)),
            ] {
                assert_eq!(contracts.len(), 1);
                let child = contract_child(&headers, owner, &contracts[0]);
                children.push(child);
            }
            let changed = crate::value_access::ValueDefinition::Function(signature(callable(
                BuiltinType::I64.into(),
            )));
            assert_eq!(
                headers.existing_input_value_reference(code.parameters()[0], &changed),
                None
            );
            assert_eq!(headers.existing_result_value_reference(id, &changed), None);
            assert_eq!(parameter.components.len(), usize::from(compound));
            assert_eq!(code.result_contracts.len(), 1);
        }
        for (index, child) in children.iter().enumerate() {
            assert!(!children[..index].contains(child));
        }
        let body = headers.body_types(ids[0]).unwrap().finish();
        let canonical = headers
            .canonical_body_types(BTreeMap::from([(ids[0].into(), &body)]))
            .unwrap();
        let merged = canonical.finish();
        for child in children {
            assert_eq!(
                merged.function_contract(witness(&merged, child)),
                headers.function_contract(witness(&headers, child))
            );
        }
    }
}

#[test]
fn invalid_nested_signature_publishes_no_header_contract_or_type() {
    let symbols = SymbolTable::from_spellings(["run", "action"]);
    let mut reservations = Reservations::new(symbols.clone());
    let code = reservations.reserve(symbols.get("run").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    builder
        .define(
            code,
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [ParameterDefinition {
                name: symbols.get("action").unwrap(),
                operation: ValueInputOperation::Read,
                data: crate::value_access::ValueDefinition::Function(signature(callable(
                    BuiltinType::F64.into(),
                ))),
            }],
            BuiltinType::Void,
        )
        .unwrap();
    let mut publication = builder.into_publication();
    let before = publication.snapshot();
    assert!(matches!(
        publication.publish(code),
        Err(HeaderFailure::Rejected(_))
    ));
    let after = publication.snapshot();
    assert!(after.preserves_headers(&before));
    assert!(after.parameters().is_empty());
    assert_eq!(
        after.types.store().type_count(),
        before.types.store().type_count()
    );
}

#[test]
fn signature_children_do_not_advance_sibling_payload_ordinals() {
    let symbols = SymbolTable::from_spellings(["receive", "actions"]);
    let mut reservations = Reservations::new(symbols.clone());
    let code = reservations.reserve(symbols.get("receive").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    let outer = DataReference::Function(Box::new(FunctionDefinition {
        invocation: ValueInvocation::Edit,
        inputs: Box::new([(ValueInputOperation::Take, callable(BuiltinType::I32.into()))]),
        result: callable(BuiltinType::I32.into()),
    }));
    let recipe = DataReference::Tuple(TupleData::new(outer.clone(), outer, []));
    builder
        .define(
            code,
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [ParameterDefinition {
                name: symbols.get("actions").unwrap(),
                operation: ValueInputOperation::Take,
                data: recipe.clone().into(),
            }],
            recipe.clone(),
        )
        .unwrap();
    let headers = builder.finish().unwrap();
    let declaration = headers.codes().get(code).unwrap();
    let input = declaration.parameters()[0];
    let parameter = headers.parameters().get(input).unwrap();
    assert_eq!(headers.parameters().len(), 1);
    assert_eq!(parameter.components.len(), 2);
    assert_eq!(declaration.result_contracts.len(), 2);
    assert_eq!(parameter.type_contracts.len(), 4);
    assert_eq!(declaration.type_contracts.len(), 4);
    assert_eq!(
        headers.existing_input_value_reference(input, &recipe.clone().into()),
        Some(parameter.data())
    );
    assert_eq!(
        headers.existing_result_value_reference(code, &recipe.into()),
        Some(declaration.result())
    );
    for contracts in [&parameter.type_contracts, &declaration.type_contracts] {
        assert!(contracts.windows(2).all(|pair| pair[0] == pair[1]));
    }
    for contracts in [&parameter.components, &declaration.result_contracts] {
        for (ordinal, contract) in contracts.iter().enumerate() {
            for (position, data) in [contract.inputs[0].data, contract.result]
                .into_iter()
                .enumerate()
            {
                let ValueFunctionOrigin::Contract(child) = witness(&headers, data).origin else {
                    panic!("signature child");
                };
                assert_eq!(child.ordinal, ordinal * 2 + position);
            }
        }
    }
}
