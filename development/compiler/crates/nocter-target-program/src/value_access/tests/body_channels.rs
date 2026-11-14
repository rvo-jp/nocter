//! Bound ABI channels are independent of the remaining body-template `TypeId` cache.
use super::*;
use nocter_declarations::value_access::FunctionWitness;
use nocter_runtime_contract::RuntimeAbiIdentity;

#[test]
fn an_independent_return_contract_does_not_publish_a_value_from_a_diverging_body() {
    let fixture = ValueSourceFixture::new("func run(): i32 { loop {} }");
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let key = unit.program().bind_code(root.into(), []).unwrap();
    let data = unit.code_body(root.into()).unwrap().typed().result_data();
    let mut work =
        nocter_constant_evaluation::state::WorkBudget::new(std::num::NonZeroU64::new(100).unwrap());
    assert_eq!(
        unit.instantiate_result_data(&key, data, &mut work),
        Err(nocter_checking::value_access::InstantiationFailure::Unresolved)
    );
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    let abi = RuntimeAbiIdentity::Arm64DarwinV1;
    let storage = storage::prepare(&graph, abi, []).unwrap();
    let signature = storage::signature(&graph, &storage, graph.root()).unwrap();
    let result = graph
        .instances()
        .get(graph.root())
        .unwrap()
        .result()
        .payload()
        .unwrap();
    assert_eq!(
        signature.result(),
        storage
            .instance(graph.unit().program(), result.instance())
            .unwrap()
    );
}

#[test]
fn input_and_return_signatures_ignore_an_unrelated_payload_in_the_body_type_cache() {
    let fixture = ValueSourceFixture::new(
        "func first(): i32 { 41 }\nfunc second(): i32 { 42 }\nfunc consume(take action: func(): i32): func(): i32 { action^ }\nfunc forward(take action: func(): i32): func(): i32 { consume(action^) }\nfunc run(): i32 { let action = forward(first)\naction() }",
    );
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let mut graph = InstanceProgram::prepare(unit, root).unwrap();
    let code = callable(graph.unit(), "forward");
    let (id, frame) = instance(&graph, "forward");
    let body = graph.body(id).unwrap().typed().body();
    let input = body.inputs().explicit()[0];
    let data = body.locals().get(input.local).unwrap().data;
    let actual_input = frame.input(input.local).unwrap().clone();
    let actual_result = frame.result().payload().unwrap();
    let second = callable(graph.unit(), "second");
    let actual = graph
        .unit()
        .program()
        .concrete_witness(FunctionWitness {
            origin: ValueFunctionOrigin::Declaration(second),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    let substituted = graph
        .unit()
        .program()
        .instantiate_function(data, &actual)
        .unwrap();
    let wrong = graph
        .unit()
        .data_evidence(data)
        .unwrap()
        .payload()
        .unwrap()
        .instantiate(graph.unit().program(), &substituted)
        .unwrap();
    assert_ne!(wrong, actual_input);
    graph.instances = graph.instances.map(|_, frame| {
        let mut frame = frame.clone();
        if frame.key().code() == ValueCode::Declaration(code) {
            frame
                .data
                .insert(data, DataEvidence::Payload(wrong.clone()));
        }
        frame
    });
    let caller = graph.instances().get(id).unwrap();
    let (&call, invocation) = caller.calls().iter().next().unwrap();
    assert_eq!(caller.prepared(call, 0), Some(&actual_input));
    assert_eq!(invocation.argument_payload(0), Some(&actual_input));
    let abi = RuntimeAbiIdentity::Arm64DarwinV1;
    let prepared = storage::prepare(&graph, abi, []).unwrap();
    let signature = storage::signature(&graph, &prepared, id).unwrap();
    assert_eq!(signature.explicit()[0].local, input.local);
    assert_eq!(signature.explicit()[0].operation, input.operation);
    assert_eq!(
        signature.explicit()[0].data,
        prepared
            .instance(graph.unit().program(), actual_input.instance())
            .unwrap()
    );
    assert_eq!(
        signature.result(),
        prepared
            .instance(graph.unit().program(), actual_result.instance())
            .unwrap()
    );
    assert_ne!(
        signature.result(),
        prepared
            .instance(graph.unit().program(), wrong.instance())
            .unwrap()
    );
}
