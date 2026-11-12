//! Source acceptance preserves checked runtime declarations without inventing callable bodies.
use super::*;
use nocter_compile_input::{ModuleIdentity, PrimitiveRoleLocator, ToolchainInput};
use nocter_constant_evaluation::state::Limits;
use nocter_model::PackageIdentity;
use nocter_runtime_contract::PrimitiveRole;

fn authority() -> declarations::PrimitiveAuthority {
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
    declarations::PrimitiveAuthority::for_module(&toolchain, &module).unwrap()
}
const HEADERS: &str = "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\n";

#[test]
fn runtime_primitive_calls_close_with_header_evidence_and_no_source_body() {
    let text = format!(
        "{HEADERS}func run(): error {{ let original = create_failure(\"test\", \"message\")\ncontext_error(original^, \"context\") }}"
    );
    let fixture = nocter_test_support::ValueSourceFixture::new(&text);
    let source = SourceUnit::prepare_with_primitive_authority_and_limits(
        fixture.syntax(),
        &authority(),
        Limits::default(),
    )
    .unwrap();
    let unit = source.unit();
    assert_eq!(unit.bodies().count(), 1);
    for (name, role) in [
        ("create_failure", PrimitiveRole::NewError),
        ("context_error", PrimitiveRole::ErrorContext),
    ] {
        let id = unit
            .program()
            .resolve_code(unit.program().symbols().get(name).unwrap())
            .unwrap();
        assert_eq!(unit.primitives().role(id), Some(role));
        assert!(unit.body(id).is_none());
        assert!(unit.code_body(id.into()).is_none());
    }
    let context = source.declarations().bind(fixture.syntax()).unwrap();
    assert!(unit.bind(&context).is_ok());
    assert!(Unit::prepare(&context).is_ok());
    let fresh =
        declarations::Query::prepare_with_primitive_authority(fixture.syntax(), &authority())
            .unwrap();
    assert!(matches!(
        unit.bind(&fresh.bind(fixture.syntax()).unwrap()),
        Err(BodyFailure::ChangedAuthority)
    ));
}

#[test]
fn runtime_primitive_calls_in_required_closures_report_the_call_domain() {
    for body in [
        "const func run(): error { create_failure(\"test\", \"message\") }",
        "const VALUE: error = create_failure(\"test\", \"message\")",
        "func run(): error { create_failure(\"test\", \"message\") }\nconst VALUE: error = run()",
    ] {
        let text = format!("{HEADERS}{body}");
        let fixture = nocter_test_support::ValueSourceFixture::new(&text);
        let result = SourceUnit::prepare_with_primitive_authority_and_limits(
            fixture.syntax(),
            &authority(),
            Limits::default(),
        );
        assert!(
            matches!(result, Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) if matches!(diagnostic.rule(), crate::value_access::constant::Rule::RuntimeCall(_))),
            "{text}"
        );
    }
}

#[test]
fn primitive_only_units_are_closed_without_hiding_unused_ordinary_failures() {
    let fixture = nocter_test_support::ValueSourceFixture::new(HEADERS);
    let source = SourceUnit::prepare_with_primitive_authority_and_limits(
        fixture.syntax(),
        &authority(),
        Limits::default(),
    )
    .unwrap();
    assert_eq!(source.unit().bodies().count(), 0);
    assert!(
        source
            .unit()
            .bind(&source.declarations().bind(fixture.syntax()).unwrap())
            .is_ok()
    );
    let text = format!("{HEADERS}func unused(value: error): error {{ value }}");
    let fixture = nocter_test_support::ValueSourceFixture::new(&text);
    assert!(matches!(
        SourceUnit::prepare_with_primitive_authority_and_limits(
            fixture.syntax(),
            &authority(),
            Limits::default(),
        ),
        Err(SourceFailure::Checking(Failure::Body {
            failure: BodyFailure::Rejected(_),
            ..
        }))
    ));
}
