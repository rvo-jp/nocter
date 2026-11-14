use super::*;
use nocter_checking::value_access::{accepted::SourceUnit, flow::resources::payload};
use nocter_model::BuiltinType;
use nocter_test_support::ValueSourceFixture;

#[test]
fn frozen_body_edge_checks_the_environment_independently_of_explicit_inputs_and_result() {
    let source = ValueSourceFixture::new(
        "func run(take input: error): i32 { let action = (input^;): i32 { 42 }\naction() }",
    );
    let unit = Arc::new(SourceUnit::prepare(source.syntax()).unwrap().into_unit());
    let run = unit
        .program()
        .resolve(unit.program().symbols().get("run").unwrap())
        .unwrap();
    let graph = InstanceProgram::prepare(unit.clone(), run).unwrap();
    let call = graph
        .instances()
        .get(graph.root())
        .unwrap()
        .calls()
        .values()
        .next()
        .unwrap();
    assert!(call.inputs().is_empty());
    let mut callee = graph
        .instances()
        .get(call.target().instance().unwrap())
        .unwrap()
        .clone();
    validate_body_edge(&unit, &callee, call).unwrap();
    let body = unit.code_body(callee.key().code()).unwrap().typed().body();
    let input = body.inputs().environment().unwrap();
    let integer = unit.program().data_type(BuiltinType::I32).unwrap();
    let key = unit
        .program()
        .instantiate_data(integer, callee.key())
        .unwrap();
    let wrong = payload(unit.program(), integer)
        .unwrap()
        .instantiate(unit.program(), &key)
        .unwrap();
    callee.metadata.insert(key);
    // Unrelated independently closed storage metadata does not select a bound channel.
    validate_body_edge(&unit, &callee, call).unwrap();
    callee.inputs.insert(input.local, wrong);
    assert_eq!(
        validate_body_edge(&unit, &callee, call),
        Err(Failure::InvalidAcceptedUnit)
    );
}
