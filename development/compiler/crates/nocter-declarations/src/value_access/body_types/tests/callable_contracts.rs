//! Type contracts are distinct identities without runtime inputs, producers or environments.
use super::*;
use nocter_model::{
    ValueCallableContract, ValueContractOwner, ValueFunctionOrigin, ValueInputOperation,
};

fn signature(data: TypeId) -> FunctionConstraint {
    FunctionConstraint {
        invocation: ValueInvocation::Read,
        inputs: Box::new([]),
        result: data,
    }
}
fn witness(program: &Program, data: TypeId) -> FunctionWitness {
    let Some(PayloadSchema::Function(witness)) = program.data_schema(data) else {
        panic!("callable contract")
    };
    witness
}

#[test]
fn nested_contracts_keep_identity_and_signature_through_canonical_body_replay() {
    let (headers, [first, second, _]) = super::application_headers();
    let integer = headers.data_type(BuiltinType::I32).unwrap();
    let mut left = headers.body_types(first).unwrap();
    let inner = left.callable_contract(0, signature(integer)).unwrap();
    let distinct = left.callable_contract(1, signature(integer)).unwrap();
    assert_ne!(inner, distinct);
    let pair = left.tuple(&[inner, distinct]).unwrap();
    let outer_signature = FunctionConstraint {
        invocation: ValueInvocation::Edit,
        inputs: vec![FunctionInput {
            data: pair,
            operation: ValueInputOperation::Take,
        }]
        .into(),
        result: inner,
    };
    let outer = left.callable_contract(2, outer_signature.clone()).unwrap();
    let left = left.finish();
    let mut right = headers.body_types(second).unwrap();
    let other = right.callable_contract(0, signature(integer)).unwrap();
    let right = right.finish();
    assert_eq!(
        inner, other,
        "independent suffixes must exercise a raw ID collision"
    );
    let canonical = headers
        .canonical_body_types(BTreeMap::from([
            (first.into(), &left),
            (second.into(), &right),
        ]))
        .unwrap();
    let inner = canonical.project(first, inner).unwrap();
    let distinct = canonical.project(first, distinct).unwrap();
    let pair = canonical.project(first, pair).unwrap();
    let outer = canonical.project(first, outer).unwrap();
    let other = canonical.project(second, other).unwrap();
    let program = canonical.finish();
    assert_ne!(inner, other);
    assert_ne!(inner, distinct);
    let outer_witness = witness(&program, outer);
    let contract = program.function_contract(outer_witness).unwrap();
    assert!(
        program
            .function_data(FunctionWitness {
                invocation: ValueInvocation::Read,
                ..outer_witness
            })
            .is_none()
    );
    assert!(
        program
            .function_data(FunctionWitness {
                invocation: ValueInvocation::Take,
                ..outer_witness
            })
            .is_some()
    );
    assert_eq!(contract.result, inner);
    assert_eq!(contract.inputs[0].data, pair);
    assert_eq!(contract.inputs[0].operation, ValueInputOperation::Take);
    assert_eq!(contract.invocation, ValueInvocation::Edit);
    assert!(program.concrete_witness(outer_witness).is_err());
    assert_eq!(program.data_copyable(outer), Some(false));
    assert_eq!(
        program.payload_readiness(outer),
        PayloadReadiness::Unsupported
    );
    assert!(program.instantiate_closed_data(outer).is_err());
    assert_eq!(outer_witness.substitute(|_| None).unwrap(), None);
    let resolved = program.resolve_body_types(&BTreeMap::new()).unwrap();
    let outer = resolved.project(outer).unwrap();
    assert_eq!(
        resolved
            .program()
            .function_contract(witness(resolved.program(), outer)),
        Some(contract)
    );
}

#[test]
fn rejected_contract_construction_is_atomic_and_does_not_create_values_or_inputs() {
    let (headers, [first, _, _]) = super::application_headers();
    let integer = headers.data_type(BuiltinType::I32).unwrap();
    let mut body = headers.body_types(first).unwrap();
    let before = body.program().types.store().type_count();
    let invalid = headers.types.store().builtin(BuiltinType::F64);
    assert_eq!(
        body.callable_contract(0, signature(invalid)),
        Err(BodyTypeError::UnsupportedCallableSignature)
    );
    assert_eq!(body.program().types.store().type_count(), before);
    let data = body.callable_contract(0, signature(integer)).unwrap();
    let before = body.program().types.store().type_count();
    assert_eq!(
        body.callable_contract(0, signature(integer)),
        Err(BodyTypeError::InvalidExtension)
    );
    assert_eq!(body.program().types.store().type_count(), before);
    let selected = witness(body.program(), data);
    assert_eq!(
        body.program().function_contract(selected),
        Some(signature(integer))
    );
    let unknown = FunctionWitness {
        origin: ValueFunctionOrigin::Contract(ValueCallableContract {
            owner: ValueContractOwner::Body(first.into()),
            ordinal: 99,
        }),
        invocation: ValueInvocation::Read,
    };
    assert!(body.program().function_data(unknown).is_none());
    assert!(body.program().function_contract(unknown).is_none());
    assert!(body.program().closure_parameters.is_empty());
    assert!(body.program().closures.is_empty());
    assert!(body.program().preserves_headers(&headers));
}

#[test]
fn result_substitution_projects_nested_contract_signatures_without_resolving_the_contract_to_code()
{
    let (headers, [root, _, factory]) = super::application_headers();
    let mut body = headers.body_types(root).unwrap();
    let call = nocter_model::ArenaBuilder::<nocter_model::AccessExpressionId, ()>::new().insert(());
    let pending = body
        .application(nocter_model::ValueCode::Declaration(root), call, factory)
        .unwrap();
    let source = witness(body.program(), pending);
    let outer = body.callable_contract(0, signature(pending)).unwrap();
    let body = body.finish();
    let canonical = headers
        .canonical_body_types(BTreeMap::from([(root.into(), &body)]))
        .unwrap();
    let outer = canonical.project(root, outer).unwrap();
    let canonical = canonical.finish();
    let actual = FunctionWitness::from(root);
    let resolved = canonical
        .resolve_body_types(&BTreeMap::from([(source, actual)]))
        .unwrap();
    let outer = resolved.project(outer).unwrap();
    let program = resolved.program();
    let selected = witness(program, outer);
    assert!(matches!(selected.origin, ValueFunctionOrigin::Contract(_)));
    assert_eq!(
        program.function_contract(selected).unwrap().result,
        program.function_data(actual).unwrap()
    );
    assert!(program.concrete_witness(selected).is_err());
}

#[test]
fn nested_contract_metadata_replays_a_deep_signature_graph_without_value_expansion() {
    let (headers, [root, _, _]) = super::application_headers();
    let mut body = headers.body_types(root).unwrap();
    let mut data = headers.data_type(BuiltinType::I32).unwrap();
    for ordinal in 0..1_000 {
        data = body.callable_contract(ordinal, signature(data)).unwrap();
    }
    let body = body.finish();
    let canonical = headers
        .canonical_body_types(BTreeMap::from([(root.into(), &body)]))
        .unwrap();
    let mut data = canonical.project(root, data).unwrap();
    let program = canonical.finish();
    for ordinal in (0..1_000).rev() {
        let selected = witness(&program, data);
        assert_eq!(
            selected.origin,
            ValueFunctionOrigin::Contract(ValueCallableContract {
                owner: ValueContractOwner::Body(root.into()),
                ordinal
            })
        );
        data = program.function_contract(selected).unwrap().result;
    }
    assert_eq!(data, program.data_type(BuiltinType::I32).unwrap());
}

#[test]
fn value_binding_keeps_code_and_environment_separate_from_the_receiving_contract() {
    let (headers, [root, wrong, _]) = super::application_headers();
    let void = headers.data_type(BuiltinType::Void).unwrap();
    let integer = headers.data_type(BuiltinType::I32).unwrap();
    let mut body = headers.body_types(root).unwrap();
    let contract = body.callable_contract(0, signature(void)).unwrap();
    let code = body.reserve_closure().unwrap();
    let captured = body.closure(code, integer, signature(void)).unwrap();
    let program = body.finish();
    let named = program.function_data(FunctionWitness::from(root)).unwrap();
    let wrong = program.function_data(FunctionWitness::from(wrong)).unwrap();
    assert!(program.function_binding(named, contract));
    assert!(program.function_binding(captured, contract));
    assert!(!program.function_adapter(named, contract));
    assert!(!program.function_binding(named, captured));
    assert!(!program.function_binding(wrong, contract));
    let keys = [named, captured].map(|source| {
        let actual = program.concrete_witness(witness(&program, source)).unwrap();
        let bound = program.instantiate_function(contract, &actual).unwrap();
        assert_eq!(bound.data(), contract);
        let crate::value_access::InstanceSchema::Function(selected) =
            bound.schema(&program).unwrap()
        else {
            panic!("same complete source witness")
        };
        assert_eq!(selected, actual);
        bound
    });
    assert!(!keys[0].has_same_representation(&program, &keys[1]));
    assert!(program.instantiate_closed_data(contract).is_err());
    assert_eq!(program.data_copyable(contract), Some(false));
    let dependencies = program
        .payload_dependencies_all([contract, integer, contract])
        .unwrap();
    assert_eq!(dependencies.contracts().len(), 1);
    assert!(dependencies.parameters().is_empty());
    assert!(dependencies.producers().is_empty());
    assert_eq!(dependencies.readiness(), PayloadReadiness::Unsupported);
}
