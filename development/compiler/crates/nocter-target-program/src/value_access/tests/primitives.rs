//! Checked runtime operations remain distinct from source instances and their local slots.
use super::*;
use nocter_compile_input::{ModuleIdentity, PrimitiveRoleLocator, ToolchainInput};
use nocter_model::{PackageIdentity, ValueInputOperation};
use nocter_runtime_contract::PrimitiveRole;

fn accepted_primitives(source: &str) -> Arc<Unit> {
    let package = PackageIdentity::new("test:standard");
    let module = ModuleIdentity::new(package.clone(), ["error"]);
    let toolchain = ToolchainInput::new(
        package.clone(),
        ModuleIdentity::new(package, ["prelude"]),
        Vec::new(),
        Vec::new(),
    )
    .with_primitive_roles(vec![
        PrimitiveRoleLocator::new(PrimitiveRole::NewError, module.clone(), "create_failure"),
        PrimitiveRoleLocator::new(PrimitiveRole::ErrorContext, module.clone(), "context_error"),
    ]);
    let authority =
        access_declarations::PrimitiveAuthority::for_module(&toolchain, &module).unwrap();
    let fixture = ValueSourceFixture::new(source);
    let declarations =
        access_declarations::Query::prepare_with_primitive_authority(fixture.syntax(), &authority)
            .unwrap();
    Arc::new(Unit::prepare(&declarations.bind(fixture.syntax()).unwrap()).unwrap())
}
const HEADERS: &str = "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\n";

#[test]
fn trusted_calls_have_positional_inputs_without_primitive_instances_or_body_locals() {
    let source = format!(
        "{HEADERS}func run(): error {{ let original = create_failure(\"test\", \"message\")\ncontext_error(original^, \"context\") }}"
    );
    let unit = accepted_primitives(&source);
    let graph = Arc::new(InstanceProgram::prepare(unit.clone(), callable(&unit, "run")).unwrap());
    assert_eq!(graph.instances().len(), 1);
    let calls: Vec<_> = graph
        .instances()
        .get(graph.root())
        .unwrap()
        .calls()
        .values()
        .collect();
    assert_eq!(calls.len(), 2);
    for call in calls {
        let CallTarget::Primitive(role) = call.target() else {
            panic!("runtime operation");
        };
        let signature = role.value_signature().unwrap();
        assert_eq!(
            unit.program().data_kind(call.result().data()),
            Some(signature.result())
        );
        assert_eq!(call.environment(), Environment::None);
        for (index, (input, expected)) in call.inputs().iter().zip(signature.inputs()).enumerate() {
            assert_eq!(input.argument, index);
            assert_eq!(input.operation, expected.operation());
            assert_eq!(
                unit.program().data_kind(input.payload.data()),
                Some(expected.data())
            );
        }
    }
    storage::prepare(
        &graph,
        nocter_runtime_contract::RuntimeAbiIdentity::Arm64DarwinV1,
    )
    .unwrap();
    let lifecycle = crate::value_access::lifecycle::Program::prepare(graph).unwrap();
    assert_eq!(lifecycle.bodies().len(), 1);
    assert_eq!(
        InstanceProgram::prepare(unit.clone(), callable(&unit, "create_failure")).unwrap_err(),
        Failure::BodylessRoot(callable(&unit, "create_failure"))
    );
}

#[test]
fn primitive_witnesses_pass_through_ordinary_callback_instances() {
    let source = format!(
        "{HEADERS}func apply(make: func(str, str): error): error {{ make(\"test\", \"message\") }}\nfunc run(): error {{ apply(create_failure) }}"
    );
    let unit = accepted_primitives(&source);
    let graph = InstanceProgram::prepare(unit.clone(), callable(&unit, "run")).unwrap();
    assert_eq!(graph.instances().len(), 2);
    let primitive = graph
        .instances()
        .iter()
        .flat_map(|(_, instance)| instance.calls().values())
        .find(|call| matches!(call.target(), CallTarget::Primitive(_)))
        .unwrap();
    assert_eq!(
        primitive.target(),
        CallTarget::Primitive(PrimitiveRole::NewError)
    );
    assert!(matches!(
        primitive.environment(),
        Environment::CodeToken { argument: 0, .. }
    ));
    assert!(
        primitive
            .inputs()
            .iter()
            .all(|input| input.operation == ValueInputOperation::Read)
    );
    assert_eq!(primitive.inputs()[0].argument, 1);
    assert_eq!(primitive.inputs()[1].argument, 2);
}
