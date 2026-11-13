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
    let call = ArenaBuilder::<AccessExpressionId, ()>::new().insert(());
    let pending = body
        .application(ValueCode::Declaration(root), call, factory)
        .unwrap();
    let environment = body.tuple(&[captures, pending]).unwrap();
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
    assert_eq!(dependencies.readiness(), PayloadReadiness::Unsupported);
    assert!(!body.program().owned_payload_supported(nested_data));
    assert_eq!(body.program().data_copyable(nested_data), Some(false));
    let empty = body.array(nested_data, 0).unwrap();
    let empty_dependencies = body.program().payload_dependencies(empty).unwrap();
    assert!(empty_dependencies.parameters().is_empty());
    assert!(!empty_dependencies.has_pending_producers());
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
