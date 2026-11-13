use super::*;
use nocter_model::{
    AccessExpressionId, ArenaBuilder, ValueApplication, ValueCode, ValueFunctionOrigin,
};

#[test]
fn application_templates_are_immutable_shape_facts_without_payload_or_copy_evidence() {
    let (header, [root, ..]) = application_headers();
    let mut body = header.body_types(root).unwrap();
    let code = body.reserve_closure().unwrap();
    let named = FunctionWitness::from(root);
    let result = header.function_data(named).unwrap();
    let void = header.data_type(BuiltinType::Void).unwrap();
    body.closure(
        code,
        void,
        FunctionConstraint {
            invocation: ValueInvocation::Read,
            inputs: Box::new([]),
            result,
        },
    )
    .unwrap();
    let old = body.program().clone();
    let call = ArenaBuilder::<AccessExpressionId, ()>::new().insert(());
    let key = ValueApplication {
        owner: ValueCode::Declaration(root),
        call,
    };
    let data = body
        .application(
            key.owner,
            call,
            FunctionWitness {
                origin: ValueFunctionOrigin::Closure(code),
                invocation: ValueInvocation::Read,
            },
        )
        .unwrap();
    assert!(old.application_template(key).is_none());
    assert_eq!(body.program().application_template(key), Some(named));
    assert_eq!(
        body.program().payload_readiness(data),
        PayloadReadiness::Pending
    );
    assert_eq!(body.program().data_copyable(data), Some(false));
    assert_eq!(
        body.program().function_contract(FunctionWitness {
            origin: ValueFunctionOrigin::Application(key),
            invocation: ValueInvocation::Read
        }),
        header.function_contract(named)
    );
}

#[test]
fn deep_application_template_queries_are_iterative_and_reject_a_cycle() {
    let (header, [root, ..]) = application_headers();
    let mut body = header.body_types(root).unwrap();
    let mut calls = ArenaBuilder::<AccessExpressionId, ()>::new();
    let mut selected = FunctionWitness::from(root);
    for _ in 0..10_000 {
        let key = ValueApplication {
            owner: ValueCode::Declaration(root),
            call: calls.insert(()),
        };
        body.program.applications.insert(key, selected);
        selected = FunctionWitness {
            origin: ValueFunctionOrigin::Application(key),
            invocation: ValueInvocation::Read,
        };
    }
    assert_eq!(
        body.program().function_contract(selected),
        header.function_contract(root.into())
    );
    let ValueFunctionOrigin::Application(key) = selected.origin else {
        unreachable!()
    };
    body.program.applications.insert(key, selected);
    assert!(body.program().function_contract(selected).is_none());
}
