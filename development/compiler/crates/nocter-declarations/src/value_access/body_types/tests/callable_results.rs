use super::*;
use nocter_model::{AccessExpressionId, ArenaBuilder, ValueCode, ValueFunctionOrigin};
use std::collections::BTreeMap;

#[test]
fn an_anonymous_result_retains_its_existing_application_through_canonical_type_projection() {
    let (header, [root, factory, _]) = application_headers();
    let mut body = header.body_types(root).unwrap();
    let closure = body.reserve_closure().unwrap();
    let call = ArenaBuilder::<AccessExpressionId, ()>::new().insert(());
    let result = body
        .application(ValueCode::Closure(closure), call, factory)
        .unwrap();
    let Some(PayloadSchema::Function(application)) = body.program().data_schema(result) else {
        panic!("application");
    };
    let void = header.data_type(BuiltinType::Void).unwrap();
    let data = body
        .closure(
            closure,
            void,
            FunctionConstraint {
                invocation: ValueInvocation::Read,
                inputs: Box::new([]),
                result,
            },
        )
        .unwrap();
    let body = body.finish();
    assert_eq!(body.closure(closure).unwrap().contract().result, result);
    assert!(header.closure(closure).is_none());
    let canonical = header
        .canonical_body_types(BTreeMap::from([(root.into(), &body)]))
        .unwrap();
    let canonical_data = canonical.project(root, data).unwrap();
    let program = canonical.finish();
    let target = FunctionWitness {
        origin: ValueFunctionOrigin::Declaration(root),
        invocation: ValueInvocation::Read,
    };
    let resolved = program
        .resolve_body_types(&BTreeMap::from([(application, target)]))
        .unwrap();
    let program = resolved.finish();
    assert_eq!(
        program.closure(closure).unwrap().contract().result,
        header.function_data(target).unwrap()
    );
    assert_eq!(
        program.function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Closure(closure),
            invocation: ValueInvocation::Read
        }),
        Some(canonical_data)
    );
    assert!(program.preserves_headers(&header));
}
