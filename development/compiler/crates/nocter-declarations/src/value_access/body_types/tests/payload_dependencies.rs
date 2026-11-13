use super::*;
use nocter_model::{
    AccessExpressionId, ArenaBuilder, ValueCode, ValueFunctionOrigin, ValueParameter,
};

#[test]
fn formal_environment_descriptors_preserve_views_and_pending_producers_without_payload_evidence() {
    let (header, [root, factory, _]) = application_headers();
    let mut body = header.body_types(root).unwrap();
    let outer = body.reserve_closure().unwrap();
    let void = header.data_type(BuiltinType::Void).unwrap();
    let contract = FunctionConstraint {
        invocation: ValueInvocation::Read,
        inputs: Box::new([]),
        result: void,
    };
    let formal = body.closure_parameter(outer, 0, contract.clone()).unwrap();
    let parameter = ValueParameter::Closure {
        owner: outer,
        position: 0,
    };
    let read = FunctionWitness {
        origin: ValueFunctionOrigin::Parameter(parameter),
        invocation: ValueInvocation::Read,
    };
    let edit = FunctionWitness {
        invocation: ValueInvocation::Edit,
        ..read
    };
    let edited = body.program().function_data(edit).unwrap();
    let read_id = body.reserve_closure().unwrap();
    let read_capture = body.closure(read_id, formal, contract.clone()).unwrap();
    let edit_id = body.reserve_closure().unwrap();
    let edit_capture = body.closure(edit_id, edited, contract.clone()).unwrap();
    let captures = body
        .tuple(&[read_capture, edit_capture, read_capture])
        .unwrap();
    let (producers, expected_producers) = producer_environment(&mut body, root, factory);
    let environment = body.tuple(&[captures, producers]).unwrap();
    let nested = body.reserve_closure().unwrap();
    let nested_data = body.closure(nested, environment, contract.clone()).unwrap();
    body.closure(
        outer,
        void,
        FunctionConstraint {
            inputs: vec![FunctionInput {
                data: formal,
                operation: nocter_model::ValueInputOperation::Read,
            }]
            .into(),
            ..contract
        },
    )
    .unwrap();
    let dependencies = body.program().payload_dependencies(nested_data).unwrap();
    assert_eq!(
        dependencies.parameters(),
        &std::collections::BTreeSet::from([read, edit])
    );
    assert!(dependencies.has_pending_producers());
    assert_eq!(dependencies.producers(), &expected_producers);
    assert_eq!(dependencies.readiness(), PayloadReadiness::Unsupported);
    assert!(!body.program().owned_payload_supported(nested_data));
    assert_eq!(body.program().data_copyable(nested_data), Some(false));
    let empty = body.array(nested_data, 0).unwrap();
    let empty_dependencies = body.program().payload_dependencies(empty).unwrap();
    assert!(empty_dependencies.parameters().is_empty());
    assert!(!empty_dependencies.has_pending_producers());
    assert!(empty_dependencies.producers().is_empty());
    assert_eq!(empty_dependencies.readiness(), PayloadReadiness::Closed);
    let body = body.finish();
    let canonical = header
        .canonical_body_types(BTreeMap::from([(root.into(), &body)]))
        .unwrap();
    let data = canonical.project(root, nested_data).unwrap();
    let canonical = canonical.finish();
    assert_eq!(canonical.payload_dependencies(data), Some(dependencies));
    assert!(!canonical.owned_payload_supported(data));
    assert!(header.closure(nested).is_none());
}

fn producer_environment(
    body: &mut BodyTypes,
    root: nocter_model::AccessCodeId,
    factory: nocter_model::AccessCodeId,
) -> (TypeId, std::collections::BTreeSet<FunctionWitness>) {
    let mut calls = ArenaBuilder::<AccessExpressionId, ()>::new();
    let call = calls.insert(());
    let second_call = calls.insert(());
    let pending = body
        .application(ValueCode::Declaration(root), call, factory)
        .unwrap();
    let second = body
        .application(ValueCode::Declaration(root), second_call, factory)
        .unwrap();
    let PayloadSchema::Function(first_witness) = body.program().data_schema(pending).unwrap()
    else {
        panic!("first application");
    };
    let PayloadSchema::Function(second_witness) = body.program().data_schema(second).unwrap()
    else {
        panic!("second application");
    };
    let taken = FunctionWitness {
        invocation: ValueInvocation::Take,
        ..first_witness
    };
    let taken_data = body.program().function_data(taken).unwrap();
    let result_witness = FunctionWitness {
        origin: ValueFunctionOrigin::Result(factory),
        invocation: first_witness.invocation,
    };
    let result_data = body.program().function_data(result_witness).unwrap();
    let producers_only = body.tuple(&[pending, second]).unwrap();
    assert_eq!(
        body.program().payload_readiness(producers_only),
        PayloadReadiness::Pending
    );
    let environment = body
        .tuple(&[pending, second, pending, taken_data, result_data])
        .unwrap();
    (
        environment,
        std::collections::BTreeSet::from([first_witness, second_witness, taken, result_witness]),
    )
}
