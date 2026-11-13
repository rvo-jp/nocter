use super::*;
use crate::value_access::{
    ir::ValueType,
    tests::{bound, input},
};
use nocter_declaration_lowering::access_declarations;
use nocter_model::{ValueAccess, ValueAccessMode, ValueInvocation};
use std::sync::Arc;

#[test]
fn application_transport_requires_the_exact_callee_result_and_creation_keeps_the_declared_view() {
    let source = "func left(value: i32): i32 { value }\nfunc right(value: i32): i32 { value + 1 }\nfunc make_left(): func(i32): i32 { left }\nfunc make_right(): func(i32): i32 { right }\nfunc run(): void { let creator = () { make_left() }\nlet other = () { make_right() }\nlet first = creator()\nlet second = other() }";
    let (sources, tree) = input(source);
    let headers = access_declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let run = context
        .program()
        .resolve(context.program().symbols().get("run").unwrap())
        .unwrap();
    let mut query = Query::prepare(&context, run).unwrap();
    let calls: Vec<_> = query
        .body()
        .expressions()
        .iter()
        .filter_map(|(id, expression)| {
            matches!(expression.kind, ExpressionKind::Call { .. }).then_some(id)
        })
        .collect();
    assert_eq!(calls.len(), 2);
    let first = call_result(&query, calls[0]).unwrap().unwrap();
    let second = call_result(&query, calls[1]).unwrap().unwrap();
    assert!(first.created.is_none() && second.created.is_none());
    assert_ne!(first.data, second.data);
    query.body.expressions = query.body.expressions.map(|id, expression| {
        let mut expression = expression.clone();
        if id == calls[0] {
            expression.ty = ValueType::known(ValueAccess::new(second.data, ValueAccessMode::Own));
        }
        expression
    });
    assert!(call_result(&query, calls[0]).is_err());
    let control = query.closures.values_mut().next().unwrap();
    let nested = Arc::make_mut(&mut control.typed);
    let call = nested
        .body()
        .expressions()
        .iter()
        .find_map(|(id, expression)| {
            matches!(expression.kind, ExpressionKind::Call { .. }).then_some(id)
        })
        .unwrap();
    let created = call_result(nested, call).unwrap().unwrap();
    let application = created.created.unwrap();
    let edited = nested
        .program()
        .function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Application(application),
            invocation: ValueInvocation::Edit,
        })
        .unwrap();
    nested.body.expressions = nested.body.expressions.map(|id, expression| {
        let mut expression = expression.clone();
        if id == call {
            expression.ty = ValueType::known(ValueAccess::new(edited, ValueAccessMode::Own));
        }
        expression
    });
    assert!(call_result(nested, call).is_err());
}
