//! Bound ABI channels are independent of the remaining body-template `TypeId` cache.
use super::*;
use nocter_declarations::value_access::FunctionWitness;
use nocter_runtime_contract::RuntimeAbiIdentity;

#[test]
fn closed_metadata_cannot_select_between_two_actual_input_contexts() {
    let fixture = ValueSourceFixture::new(
        "func first(): i32 { 41 }\nfunc second(): i32 { 42 }\nfunc apply(action: func(): i32): i32 { read selected = action\nselected() }\nfunc run(): i32 { apply(first) + apply(second) - 41 }",
    );
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let apply = callable(&unit, "apply");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    let frames = graph
        .instances()
        .iter()
        .filter(|(_, frame)| frame.key().code() == ValueCode::Declaration(apply))
        .collect::<Vec<_>>();
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].1.metadata(), frames[1].1.metadata());
    assert!(
        frames[0]
            .1
            .metadata()
            .iter()
            .all(nocter_declarations::value_access::DataInstanceKey::is_context_free)
    );
    let inputs = frames
        .iter()
        .map(|(id, frame)| {
            let local = graph.body(*id).unwrap().typed().body().inputs().explicit()[0].local;
            frame.input(local).unwrap().instance()
        })
        .collect::<Vec<_>>();
    assert_ne!(
        inputs[0], inputs[1],
        "identical closed metadata must not merge actual bindings"
    );
    let storage = storage::prepare(&graph, RuntimeAbiIdentity::Arm64DarwinV1, []).unwrap();
    assert_ne!(
        storage.instance(graph.unit().program(), inputs[0]).unwrap(),
        storage.instance(graph.unit().program(), inputs[1]).unwrap()
    );
}

#[test]
fn value_data_exhaustion_reports_the_original_action_and_allows_a_healthy_retry() {
    let fixture = ValueSourceFixture::new("func run(): i32 { 42 }");
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let mut work =
        nocter_constant_evaluation::state::WorkBudget::new(std::num::NonZeroU64::new(1).unwrap());
    let failure = InstanceProgram::prepare_with_budget(unit.clone(), root, &mut work).unwrap_err();
    let Failure::Value { code, at, failure } = failure else {
        panic!("the value metadata query must report its original action: {failure:?}")
    };
    assert_eq!(code, ValueCode::Declaration(root));
    assert_eq!(
        failure,
        nocter_checking::value_access::InstantiationFailure::WorkLimit
    );
    assert!(
        unit.code_body(code)
            .unwrap()
            .graph()
            .points()
            .get(at.point)
            .unwrap()
            .actions
            .get(at.action)
            .is_some()
    );
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    assert_eq!(graph.instances().len(), 1);
}

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
    assert_eq!(
        unit.instantiate_result_contract(&key, &mut work).unwrap(),
        unit.program().instantiate_closed_data(data).unwrap()
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
fn input_and_return_signatures_ignore_unrelated_closed_storage_metadata() {
    let fixture = ValueSourceFixture::new(
        "func first(): i32 { 41 }\nfunc second(): i32 { 42 }\nfunc consume(take action: func(): i32): func(): i32 { action^ }\nfunc forward(take action: func(): i32): func(): i32 { let saved = consume(action^)\nsaved^ }\nfunc run(): i32 { let action = forward(first)\naction() }",
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
            frame.metadata.insert(
                wrong
                    .instance()
                    .function_representation(graph.unit().program())
                    .unwrap()
                    .unwrap(),
            );
        }
        frame
    });
    let caller = graph.instances().get(id).unwrap();
    let (&call, invocation) = caller.calls().iter().next().unwrap();
    assert_eq!(caller.prepared(call, 0), Some(&actual_input));
    assert_eq!(invocation.argument_payload(0), Some(&actual_input));
    assert_original_values(&graph, id, data, &actual_input);
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

fn assert_original_values(
    graph: &InstanceProgram,
    id: AccessInstanceId,
    data: nocter_model::TypeId,
    expected: &Payload,
) {
    let caller = graph.instances().get(id).unwrap();
    let checked = graph.body(id).unwrap();
    let sources = graph
        .unit()
        .value_sources(caller.key().code())
        .unwrap()
        .flow()
        .unwrap();
    let mut definitions = 0;
    let mut local_definitions = 0;
    for ((at, slot), _) in sources.definitions() {
        if !checked.initialization().reachable(at.point) {
            continue;
        }
        if let nocter_checking::value_access::ValueSourceSlot::Expression(expression) = slot
            && checked
                .typed()
                .body()
                .expressions()
                .get(expression)
                .unwrap()
                .ty
                .access()
                .is_some_and(|access| access.data() == data)
        {
            assert_eq!(caller.definition(at, slot), Some(expected));
            definitions += 1;
        }
        if let nocter_checking::value_access::ValueSourceSlot::Local(local) = slot
            && checked.typed().body().locals().get(local).unwrap().data == data
        {
            assert_eq!(caller.definition(at, slot), Some(expected));
            local_definitions += 1;
        }
    }
    assert!(definitions > 0);
    assert!(local_definitions > 0);
    let mut operands = 0;
    for ((at, expression), _) in sources.operands() {
        if checked.initialization().reachable(at.point)
            && checked
                .typed()
                .body()
                .expressions()
                .get(expression)
                .unwrap()
                .ty
                .access()
                .is_some_and(|access| access.data() == data)
        {
            assert_eq!(caller.operand(at, expression), Some(expected));
            operands += 1;
        }
    }
    assert!(operands > 0);
}
