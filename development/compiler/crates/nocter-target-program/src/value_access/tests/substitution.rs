use super::*;
use nocter_checking::value_access::CallKind;
use nocter_declarations::value_access::FunctionWitness;

#[test]
fn executable_selection_rejects_an_unbound_or_weaker_view_even_when_its_underlying_code_can_read() {
    let fixture = ValueSourceFixture::new(
        "func identity(value: i32): i32 { value }\nfunc apply(action: func(i32): i32): i32 { action(42) }\nfunc run(): i32 { apply(identity) }",
    );
    let unit = accepted(&fixture);
    let apply = callable(&unit, "apply");
    let identity = callable(&unit, "identity");
    let parameter = unit.program().codes().get(apply).unwrap().parameters()[0].into();
    let body = unit.body(apply).unwrap().typed().body();
    let (target, kind, arguments) = body
        .expressions()
        .iter()
        .find_map(|(_, expression)| match &expression.kind {
            ExpressionKind::Call {
                target,
                kind,
                arguments,
            } => Some((*target, *kind, arguments.as_ref())),
            _ => None,
        })
        .unwrap();
    assert!(
        unit.program()
            .bind_code(ValueCode::Declaration(apply), [])
            .is_err()
    );
    assert!(
        unit.program()
            .bind_code(
                ValueCode::Declaration(apply),
                [(
                    parameter,
                    FunctionWitness {
                        origin: ValueFunctionOrigin::Declaration(identity),
                        invocation: ValueInvocation::Edit,
                    }
                )]
            )
            .is_err()
    );
    let caller = unit
        .program()
        .bind_code(
            ValueCode::Declaration(apply),
            [(parameter, identity.into())],
        )
        .unwrap();
    let selected = projection::callee(unit.program(), &caller, target, kind, arguments).unwrap();
    assert_eq!(selected.code(), ValueCode::Declaration(identity));
    assert!(selected.witnesses().is_empty());
    assert_eq!(
        projection::witness(
            unit.program(),
            &caller,
            FunctionWitness {
                origin: ValueFunctionOrigin::Parameter(parameter),
                invocation: ValueInvocation::Edit
            }
        )
        .unwrap()
        .invocation(),
        ValueInvocation::Edit
    );
}

#[test]
fn explicit_edit_inputs_cannot_reclassify_a_direct_call_as_an_editable_environment() {
    let fixture = ValueSourceFixture::new(
        "func update(edit value: i32): void { value += 1 }\nfunc run(): i32 { var value = 41\nupdate(value)\nvalue }",
    );
    let unit = accepted(&fixture);
    let run = callable(&unit, "run");
    let (kind, arguments) = unit
        .body(run)
        .unwrap()
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(_, expression)| match &expression.kind {
            ExpressionKind::Call {
                kind, arguments, ..
            } => Some((*kind, arguments.as_ref())),
            _ => None,
        })
        .unwrap();
    assert_eq!(kind, CallKind::Direct);
    assert_eq!(
        arguments[0].operation,
        nocter_model::ValueInputOperation::Edit
    );
    assert_eq!(kind.invocation(arguments), Some(ValueInvocation::Read));
    assert_eq!(CallKind::Value.invocation(&[]), None);
    assert_eq!(
        CallKind::Direct.invocation(&[]),
        Some(ValueInvocation::Read)
    );
}
