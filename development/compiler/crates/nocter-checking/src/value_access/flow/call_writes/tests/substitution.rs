use super::*;
use nocter_declarations::value_access::FunctionWitness;
use nocter_model::{ValueFunctionOrigin, ValueInvocation};

#[test]
fn partial_effect_selection_keeps_unbound_targets_unknown_and_rejects_a_weaker_bound_view() {
    let controls = controls(
        "func identity(value: i32): i32 { value }\nfunc apply(action: func(i32): i32): i32 { action(42) }\nfunc run(): i32 { apply(identity) }",
    );
    let apply = code(&controls, "apply");
    let identity = code(&controls, "identity");
    let typed = &controls
        .iter()
        .find(|control| control.typed.code() == ValueCode::Declaration(apply))
        .unwrap()
        .typed;
    let parameter = typed.program().codes().get(apply).unwrap().parameters()[0].into();
    let call = typed
        .body()
        .expressions()
        .iter()
        .find_map(|(id, expression)| {
            matches!(expression.kind, ExpressionKind::Call { .. }).then_some(id)
        })
        .unwrap();
    let mut request = Request::unbound(ValueCode::Declaration(apply));
    assert_eq!(request.call(typed, call).unwrap(), None);
    request.bindings.insert(
        parameter,
        FunctionWitness {
            origin: ValueFunctionOrigin::Declaration(identity),
            invocation: ValueInvocation::Edit,
        },
    );
    assert!(matches!(
        request.call(typed, call),
        Err(IntegrityError::InconsistentTypedBody)
    ));
    request.bindings.get_mut(&parameter).unwrap().invocation = ValueInvocation::Read;
    let target = request.call(typed, call).unwrap().unwrap();
    assert_eq!(target.code, ValueCode::Declaration(identity));
    assert!(target.bindings.is_empty());
}
