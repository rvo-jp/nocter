use super::*;
use nocter_checking::value_access::{accepted::SourceUnit, flow::resources::payload};
use nocter_model::BuiltinType;
use nocter_test_support::ValueSourceFixture;

#[test]
fn compound_result_edges_reject_swapped_contexts_of_the_same_callable_code() {
    use nocter_declarations::value_access::InstanceSchema;
    use nocter_model::ValueField;
    let source = ValueSourceFixture::new(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-results.nct"
    )));
    let unit = Arc::new(SourceUnit::prepare(source.syntax()).unwrap().into_unit());
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let graph = InstanceProgram::prepare(unit.clone(), code("run")).unwrap();
    let call = graph
        .instances()
        .iter()
        .flat_map(|(_, instance)| instance.calls().values())
        .find(|call| {
            call.target().instance().is_some_and(|id| {
                graph.instances().get(id).unwrap().key().code() == code("pack").into()
            })
        })
        .unwrap();
    let mut callee = graph
        .instances()
        .get(call.target().instance().unwrap())
        .unwrap()
        .clone();
    validate_body_edge(&unit, &callee, call.environment(), call).unwrap();
    let original = callee.result().payload().unwrap();
    let fields = [0, 1].map(|position| {
        let key = original
            .instance()
            .component(program, ValueField::Tuple(position))
            .unwrap();
        let InstanceSchema::Function(witness) = key.schema(program).unwrap() else {
            panic!("callable result leaf");
        };
        witness
    });
    assert_eq!(fields[0].witness(), fields[1].witness());
    assert_ne!(fields[0], fields[1]);
    let swapped = program
        .instantiate_components(
            original.data(),
            [0, 1].map(|position| {
                let field = ValueField::Tuple(position);
                let target = program.component(original.data(), field).unwrap().data;
                (
                    field,
                    program
                        .instantiate_function(target, &fields[1 - position])
                        .unwrap(),
                )
            }),
        )
        .unwrap();
    callee.result = super::super::DataEvidence::Payload(
        payload(program, original.data())
            .unwrap()
            .instantiate(program, &swapped)
            .unwrap(),
    );
    assert_eq!(
        validate_body_edge(&unit, &callee, call.environment(), call),
        Err(Failure::InvalidAcceptedUnit)
    );
}

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
    validate_body_edge(&unit, &callee, call.environment(), call).unwrap();
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
    validate_body_edge(&unit, &callee, call.environment(), call).unwrap();
    callee.inputs.insert(input.local, wrong);
    assert_eq!(
        validate_body_edge(&unit, &callee, call.environment(), call),
        Err(Failure::InvalidAcceptedUnit)
    );
}

#[test]
fn selected_callable_edges_reject_swapped_complete_capture_contexts() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/value-contexts/contextual-joined-sibling-growth.nct"
    ));
    let (definitions, _) = source.split_once("\nconst ANSWER:").unwrap();
    let fixture = ValueSourceFixture::new(&format!(
        "{definitions}\nfunc run(): i32 {{ calculate(true) }}"
    ));
    let unit = Arc::new(SourceUnit::prepare(fixture.syntax()).unwrap().into_unit());
    let root = unit
        .program()
        .resolve(unit.program().symbols().get("run").unwrap())
        .unwrap();
    let graph = InstanceProgram::prepare(unit.clone(), root).unwrap();
    let call = graph
        .instances()
        .iter()
        .flat_map(|(_, instance)| instance.calls().values())
        .find(|call| call.cases().len() == 2)
        .unwrap();
    let [first, second] = call.cases() else {
        panic!("two cases");
    };
    assert_eq!(
        validate_edge(
            &unit,
            graph.instances(),
            first.target,
            first.environment.clone(),
            call
        ),
        Ok(())
    );
    assert_eq!(
        validate_edge(
            &unit,
            graph.instances(),
            second.target,
            second.environment.clone(),
            call
        ),
        Ok(())
    );
    assert_eq!(
        validate_edge(
            &unit,
            graph.instances(),
            first.target,
            second.environment.clone(),
            call
        ),
        Err(Failure::InvalidAcceptedUnit)
    );
}
