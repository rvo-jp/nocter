//! Actual payload roots are independent of a frame's default type map and executable reachability.
use super::*;
use nocter_declarations::value_access::FunctionWitness;
use nocter_model::ValueFunctionOrigin;
use nocter_runtime_contract::{RuntimeAbiIdentity, RuntimeType};

#[test]
fn an_additional_actual_callable_root_materializes_its_environment_without_creating_a_code_instance()
 {
    let fixture = ValueSourceFixture::new(
        "func first(): i32 { 42 }\nfunc forward(take action: func(): i32): func(): i32 { action^ }\nfunc dormant(take input: error): void { let action = (input^;): i32 { 42 } }\nfunc run(): i32 { let action = forward(first)\naction() }",
    );
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    let program = graph.unit().program();
    let forward = callable(graph.unit(), "forward");
    let parameter = program.codes().get(forward).unwrap().parameters()[0];
    let schema = program
        .function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Parameter(parameter.into()),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    let dormant = callable(graph.unit(), "dormant");
    let closure = graph
        .unit()
        .code_body(dormant.into())
        .unwrap()
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(_, expression)| match expression.kind {
            ExpressionKind::Closure { closure, .. } => Some(closure),
            _ => None,
        })
        .unwrap();
    let actual = program
        .concrete_witness(FunctionWitness {
            origin: ValueFunctionOrigin::Closure(closure),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    let root = program.instantiate_function(schema, &actual).unwrap();
    let abi = RuntimeAbiIdentity::Arm64DarwinV1;
    let baseline = storage::prepare(&graph, abi, []).unwrap();
    assert!(baseline.instance(program, &root).is_err());
    let representation = root.function_representation(program).unwrap().unwrap();
    assert_ne!(root.data(), representation.data());
    let prepared = storage::prepare(&graph, abi, [root.clone(), representation.clone()]).unwrap();
    let id = prepared.instance(program, &root).unwrap();
    assert_eq!(prepared.instance(program, &representation).unwrap(), id);
    let Some(RuntimeType::StaticCallable {
        environment: Some(environment),
    }) = prepared.environment().types().get(id)
    else {
        panic!("actual captured representation")
    };
    assert!(prepared.environment().types().get(*environment).is_some());
    assert!(
        graph
            .instances()
            .iter()
            .all(
                |(_, instance)| instance.key().code() != ValueCode::Closure(closure)
                    && instance.key().code() != ValueCode::Declaration(dormant)
            )
    );
    assert!(baseline.instance(program, &root).is_err());
}
