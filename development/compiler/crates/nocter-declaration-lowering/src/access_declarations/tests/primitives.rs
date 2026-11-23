//! Primitive authority is selected by the host; authored names and matching layouts cannot grant it.
use super::*;
use nocter_compile_input::{ModuleIdentity, PrimitiveRoleLocator, ToolchainInput};
use nocter_model::PackageIdentity;
use nocter_runtime_contract::PrimitiveRole;

fn profile(name: &str) -> (ToolchainInput, ModuleIdentity) {
    let package = PackageIdentity::new("test:standard");
    let module = ModuleIdentity::new(package.clone(), ["error"]);
    let toolchain = ToolchainInput::new(
        package.clone(),
        ModuleIdentity::new(package, ["prelude"]),
        Vec::new(),
        Vec::new(),
    )
    .with_primitive_roles(vec![
        PrimitiveRoleLocator::new(PrimitiveRole::NewError, module.clone(), name),
        PrimitiveRoleLocator::new(PrimitiveRole::ErrorContext, module.clone(), "context_error"),
    ]);
    (toolchain, module)
}
#[test]
fn selected_headers_have_no_fabricated_body_and_bind_only_after_exact_contract_validation() {
    let (toolchain, module) = profile("create_failure");
    let authority = PrimitiveAuthority::for_module(&toolchain, &module).unwrap();
    let text = "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\nfunc helper(): i32 { 1 }";
    let (sources, tree) = input(text, ParseGoal::V2CoreUnit);
    let mut preparation =
        Preparation::prepare_with_primitive_authority(bound(&sources, &tree), &authority).unwrap();
    let before = preparation.current();
    let id = before
        .program()
        .resolve_code(before.program().symbols().get("create_failure").unwrap())
        .unwrap();
    assert_eq!(before.primitives().role(id), None);
    assert_eq!(
        preparation.advance(id).unwrap(),
        nocter_declarations::value_access::HeaderStep::Ready
    );
    assert_eq!(before.primitives().role(id), None);
    let context = preparation.current();
    assert_eq!(context.primitives().role(id), Some(PrimitiveRole::NewError));
    assert_eq!(authority.module(), &module);
    assert_eq!(context.code_source(id).unwrap().body(), None);
    assert_eq!(
        context.program().codes().get(id).unwrap().kind(),
        nocter_declarations::value_access::CodeKind::Primitive
    );
    assert!(context.body_source(id).is_err());
    let query = preparation.finish().unwrap();
    let changed = format!("// moved\n{}", text.replace("{ 1 }", "{ 42 }"));
    let (sources, tree) = input(&changed, ParseGoal::V2CoreUnit);
    let context = query.bind(bound(&sources, &tree)).unwrap();
    assert_eq!(context.primitives().role(id), Some(PrimitiveRole::NewError));
    assert_eq!(context.code_source(id).unwrap().body(), None);
    let changed = changed.replace("code: str", "take code: str");
    let (sources, tree) = input(&changed, ParseGoal::V2CoreUnit);
    assert!(matches!(
        query.bind(bound(&sources, &tree)),
        Err(Failure::ChangedHeader)
    ));
}
#[test]
fn source_names_wrong_modules_and_mismatching_data_or_operations_cannot_acquire_roles() {
    let (toolchain, module) = profile("new_error");
    let text = "primitive func new_error(code: str, message: str): error";
    let (sources, tree) = input(text, ParseGoal::V2CoreUnit);
    assert!(
        matches!(Query::prepare(bound(&sources, &tree)), Err(Failure::Primitive(diagnostic)) if diagnostic.rule() == PrimitiveRule::Unauthorized)
    );
    for module in [
        ModuleIdentity::new(module.package().clone(), ["other"]),
        ModuleIdentity::new(PackageIdentity::new("test:user"), ["error"]),
    ] {
        let authority = PrimitiveAuthority::for_module(&toolchain, &module).unwrap();
        assert!(
            matches!(Query::prepare_with_primitive_authority(bound(&sources, &tree), &authority), Err(Failure::Primitive(diagnostic)) if diagnostic.rule() == PrimitiveRule::Unauthorized)
        );
    }
    let authority = PrimitiveAuthority::for_module(&toolchain, &module).unwrap();
    for (text, rule) in [
        (
            "primitive func new_error(code: str): error",
            PrimitiveRule::InputCount,
        ),
        (
            "primitive func new_error(take code: str, message: str): error",
            PrimitiveRule::Input(0),
        ),
        (
            "primitive func new_error(code: str, message: i32): error",
            PrimitiveRule::Input(1),
        ),
        (
            "primitive func new_error(code: str, message: str): i32",
            PrimitiveRule::Result,
        ),
        (
            "primitive func context_error(value: error, message: str): error",
            PrimitiveRule::Input(0),
        ),
        (
            "func new_error(code: str, message: str): error { loop {} }",
            PrimitiveRule::MissingMarker,
        ),
        ("struct new_error {}", PrimitiveRule::MissingMarker),
        ("const new_error: i32 = 42", PrimitiveRule::MissingMarker),
    ] {
        let (sources, tree) = input(text, ParseGoal::V2CoreUnit);
        let Err(Failure::Primitive(diagnostic)) =
            Query::prepare_with_primitive_authority(bound(&sources, &tree), &authority)
        else {
            panic!("{text}");
        };
        assert_eq!(diagnostic.rule(), rule, "{text}");
        let range = diagnostic.source().primary().span().range();
        assert!(range.start() < range.end());
    }
}

#[test]
fn process_argument_authority_requires_both_exact_optional_layers() {
    let package = PackageIdentity::new("test:standard");
    let module = ModuleIdentity::new(package.clone(), ["process"]);
    let toolchain = ToolchainInput::new(package, module.clone(), Vec::new(), Vec::new())
        .with_primitive_roles(vec![PrimitiveRoleLocator::new(
            PrimitiveRole::ProcessArgument,
            module.clone(),
            "argument",
        )]);
    let authority = PrimitiveAuthority::for_module(&toolchain, &module).unwrap();
    for result in ["str??", "str", "str?", "str?!", "str???", "error??"] {
        let text = format!("primitive func argument(index: usize): {result}");
        let (sources, tree) = input(&text, ParseGoal::V2CoreUnit);
        let admitted = Query::prepare_with_primitive_authority(bound(&sources, &tree), &authority);
        if result == "str??" {
            assert!(admitted.is_ok());
        } else {
            assert!(
                matches!(admitted, Err(Failure::Primitive(diagnostic))
                if diagnostic.rule() == PrimitiveRule::Result),
                "{result}"
            );
        }
    }
}

#[test]
fn primitive_roles_select_the_value_namespace_without_reserving_a_type_spelling() {
    let (toolchain, module) = profile("new_error");
    let authority = PrimitiveAuthority::for_module(&toolchain, &module).unwrap();
    let text =
        "struct new_error { value: i32 }\nprimitive func new_error(code: str, message: str): error";
    let (sources, tree) = input(text, ParseGoal::V2CoreUnit);
    let query =
        Query::prepare_with_primitive_authority(bound(&sources, &tree), &authority).unwrap();
    let context = query.bind(bound(&sources, &tree)).unwrap();
    let symbol = context.program().symbols().get("new_error").unwrap();
    assert!(context.program().resolve_nominal(symbol).is_some());
    let code = context.program().resolve_code(symbol).unwrap();
    assert_eq!(
        context.primitives().role(code),
        Some(PrimitiveRole::NewError)
    );
    assert!(context.code_source(code).unwrap().body().is_none());
}
#[test]
fn conflicting_catalogs_and_unqualified_roles_fail_before_binding() {
    let (toolchain, module) = profile("new_error");
    let duplicate = toolchain.clone().with_primitive_roles(vec![
        PrimitiveRoleLocator::new(PrimitiveRole::NewError, module.clone(), "first"),
        PrimitiveRoleLocator::new(
            PrimitiveRole::NewError,
            ModuleIdentity::new(module.package().clone(), ["other"]),
            "second",
        ),
    ]);
    assert!(matches!(
        PrimitiveAuthority::for_module(&duplicate, &module),
        Err(PrimitiveAuthorityError::RepeatedRole(
            PrimitiveRole::NewError
        ))
    ));
    let duplicate = toolchain.clone().with_primitive_roles(vec![
        PrimitiveRoleLocator::new(PrimitiveRole::NewError, module.clone(), "same"),
        PrimitiveRoleLocator::new(PrimitiveRole::ErrorContext, module.clone(), "same"),
    ]);
    assert!(matches!(
        PrimitiveAuthority::for_module(&duplicate, &module),
        Err(PrimitiveAuthorityError::RepeatedName(_, _))
    ));
    let unsupported = toolchain.with_primitive_roles(vec![PrimitiveRoleLocator::new(
        PrimitiveRole::ErrorCode,
        module.clone(),
        "read_code",
    )]);
    let authority = PrimitiveAuthority::for_module(&unsupported, &module).unwrap();
    let (sources, tree) = input(
        "primitive func read_code(value: error): str",
        ParseGoal::V2CoreUnit,
    );
    assert!(
        matches!(Query::prepare_with_primitive_authority(bound(&sources, &tree), &authority), Err(Failure::Primitive(diagnostic)) if diagnostic.rule() == PrimitiveRule::UnsupportedRole)
    );
}

#[test]
fn a_failed_signature_cannot_become_a_checked_binding_on_retry_or_snapshot_reuse() {
    let (toolchain, module) = profile("new_error");
    let authority = PrimitiveAuthority::for_module(&toolchain, &module).unwrap();
    let (sources, tree) = input(
        "primitive func new_error(take code: str, message: str): error",
        ParseGoal::V2CoreUnit,
    );
    let mut preparation =
        Preparation::prepare_with_primitive_authority(bound(&sources, &tree), &authority).unwrap();
    let before = preparation.current();
    let id = before
        .program()
        .resolve_code(before.program().symbols().get("new_error").unwrap())
        .unwrap();
    for _ in 0..2 {
        assert!(
            matches!(preparation.advance(id), Err(Failure::Primitive(diagnostic)) if diagnostic.rule() == PrimitiveRule::Input(0))
        );
        assert_eq!(preparation.current().primitives().role(id), None);
        assert_eq!(before.primitives().role(id), None);
    }
    assert!(
        matches!(preparation.finish(), Err(Failure::Primitive(diagnostic)) if diagnostic.rule() == PrimitiveRule::Input(0))
    );
}
