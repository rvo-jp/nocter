//! Exact toolchain-selected modules share role selection and checked signatures with single sources.
use super::*;
use nocter_compile_input::{ModuleIdentity, PrimitiveRoleLocator, ToolchainInput};
use nocter_model::PackageIdentity;
use nocter_runtime_contract::PrimitiveRole;

#[test]
fn module_primitives_keep_original_roles_and_no_fabricated_bodies_across_header_rebinding() {
    let (fixture, profile) = nocter_test_support::trusted_error_module_fixture();
    let source =
        ModuleSourceUnit::prepare(&fixture.input().with_toolchain(profile.clone())).unwrap();
    let context = source.declarations().current().unwrap();
    let unit = source.unit();
    let mut roles = Vec::new();
    for (id, code) in unit.program().codes().iter() {
        if code.kind() == nocter_declarations::value_access::CodeKind::Primitive {
            let role = unit.primitives().role(id).unwrap();
            roles.push(role);
            let current = context.for_code(id).unwrap();
            assert!(current.code_source(id).unwrap().body().is_none());
            assert!(current.body_source(id).is_err());
            assert!(unit.body(id).is_none());
        }
    }
    roles.sort_unstable();
    let mut expected = vec![PrimitiveRole::NewError, PrimitiveRole::ErrorContext];
    expected.sort_unstable();
    assert_eq!(roles, expected);
    let ordinary = context
        .program()
        .resolve_code(context.program().symbols().get("operation").unwrap())
        .unwrap();
    assert_eq!(unit.primitives().role(ordinary), None);
    let headers = source.declarations().query().unwrap();
    let (current, _) = nocter_test_support::trusted_error_module_fixture();
    let reordered = profile
        .clone()
        .with_primitive_roles(profile.primitive_roles().iter().rev().cloned().collect());
    assert!(
        headers
            .bind(&current.input().with_toolchain(reordered))
            .is_ok()
    );
    let swapped = profile.clone().with_primitive_roles(vec![
        PrimitiveRoleLocator::new(
            PrimitiveRole::ErrorContext,
            profile.primitive_roles()[0].module().clone(),
            "operation",
        ),
        PrimitiveRoleLocator::new(
            PrimitiveRole::NewError,
            profile.primitive_roles()[1].module().clone(),
            "operation",
        ),
    ]);
    assert!(matches!(
        headers.bind(&current.input().with_toolchain(swapped)),
        Err(declarations::ModuleSourcesFailure::Access(
            declarations::Failure::ChangedHeader
        ))
    ));
    let rebound = headers
        .bind(&current.input().with_toolchain(profile))
        .unwrap();
    let rebound = rebound.current().unwrap();
    assert!(unit.bind(&rebound).is_ok());
    assert!(matches!(
        headers.bind(&current.input()),
        Err(declarations::ModuleSourcesFailure::Access(
            declarations::Failure::Primitive(_)
        ))
    ));
}

fn headers_fixture(create: &str, context: &str) -> Fixture {
    Fixture::new(
        &[
            ("/app/index.nct", Root, ""),
            ("/app/error/index.nct", Root, create),
            ("/app/context/index.nct", Root, context),
        ],
        &[],
    )
    .with_modules(&[(&[], &[0]), (&["error"], &[1]), (&["context"], &[2])])
}

#[test]
fn actual_module_signatures_and_missing_markers_reject_before_primitive_body_admission() {
    let (_, profile) = nocter_test_support::trusted_error_module_fixture();
    let context = "primitive func operation(take value: error, message: str): error";
    for (create, rule) in [
        (
            "primitive func operation(take code: str, message: str): error",
            declarations::PrimitiveRule::Input(0),
        ),
        (
            "primitive func operation(code: str, message: i32): error",
            declarations::PrimitiveRule::Input(1),
        ),
        (
            "primitive func operation(code: str): error",
            declarations::PrimitiveRule::InputCount,
        ),
        (
            "primitive func operation(code: str, message: str): i32",
            declarations::PrimitiveRule::Result,
        ),
    ] {
        let fixture = headers_fixture(create, context);
        let Err(SourceFailure::Declarations(declarations::Failure::Primitive(diagnostic))) =
            ModuleSourceUnit::prepare(&fixture.input().with_toolchain(profile.clone()))
        else {
            panic!("{create}");
        };
        assert_eq!(diagnostic.rule(), rule);
        assert_eq!(
            diagnostic.source().primary().source(),
            fixture.tree(1).source()
        );
    }
    let fixture = headers_fixture("func operation(code: str, message: str): error", context);
    assert!(matches!(
        ModuleSourceUnit::prepare(&fixture.input().with_toolchain(profile)),
        Err(SourceFailure::Modules(
            declarations::ModuleSourcesFailure::Toolchain(
                nocter_declaration_lowering::ToolchainError::MissingPrimitiveDeclaration(
                    PrimitiveRole::NewError
                )
            )
        ))
    ));
}

#[test]
fn wrong_package_locators_and_duplicate_role_requests_never_grant_module_authority() {
    let (fixture, profile) = nocter_test_support::trusted_error_module_fixture();
    let wrong = ToolchainInput::new(
        profile.standard_package().clone(),
        profile.prelude().clone(),
        Vec::new(),
        Vec::new(),
    )
    .with_primitive_roles(vec![PrimitiveRoleLocator::new(
        PrimitiveRole::NewError,
        ModuleIdentity::new(PackageIdentity::new("untrusted:app"), ["error"]),
        "operation",
    )]);
    assert!(matches!(ModuleSourceUnit::prepare(&fixture.input().with_toolchain(wrong)), Err(SourceFailure::Modules(declarations::ModuleSourcesFailure::Toolchain(nocter_declaration_lowering::ToolchainError::DeclarationModuleOutsideStandardPackage(_))))));
    let duplicate = profile.clone().with_primitive_roles(vec![
        profile.primitive_roles()[0].clone(),
        profile.primitive_roles()[0].clone(),
    ]);
    assert!(matches!(
        ModuleSourceUnit::prepare(&fixture.input().with_toolchain(duplicate)),
        Err(SourceFailure::Modules(
            declarations::ModuleSourcesFailure::PrimitiveAuthority(
                declarations::PrimitiveAuthorityError::RepeatedRole(PrimitiveRole::NewError)
            )
        ))
    ));
}

#[test]
fn failed_module_signatures_never_upgrade_old_snapshots_or_seal_as_checked_roles() {
    let (_, profile) = nocter_test_support::trusted_error_module_fixture();
    let profile = profile
        .clone()
        .with_primitive_roles(vec![profile.primitive_roles()[0].clone()]);
    let fixture = headers_fixture(
        "primitive func operation(take code: str, message: str): error",
        "",
    );
    let mut headers =
        declarations::ModuleSources::prepare(&fixture.input().with_toolchain(profile))
            .unwrap()
            .pair()
            .unwrap()
            .reserve_names()
            .unwrap();
    let binding = headers.source_binding();
    let before = binding.current(&headers).unwrap();
    let (physical, _) = headers
        .paired()
        .inventory()
        .source_entries()
        .find(|(_, source)| source.canonical_path() == "/app/error/index.nct")
        .unwrap();
    let code = headers
        .program()
        .resolve_code_in(
            headers.scope(physical).unwrap(),
            before.program().symbols().get("operation").unwrap(),
        )
        .unwrap();
    for _ in 0..2 {
        assert!(
            matches!(headers.advance_header_with_values(code, |_| None), Err(declarations::ModuleSourcesFailure::Access(declarations::Failure::Primitive(error))) if error.rule() == declarations::PrimitiveRule::Input(0))
        );
        assert_eq!(
            binding.current(&headers).unwrap().primitives().role(code),
            None
        );
        assert_eq!(before.primitives().role(code), None);
    }
    assert!(headers.program().headers_complete());
    assert!(headers.seal().is_err());
}

#[test]
fn module_locators_never_fall_back_to_familiar_names_or_choose_between_duplicate_sources() {
    let (fixture, profile) = nocter_test_support::trusted_error_module_fixture();
    let wrong_module = profile.clone().with_primitive_roles(vec![
        PrimitiveRoleLocator::new(
            PrimitiveRole::NewError,
            profile.prelude().clone(),
            "operation",
        ),
        profile.primitive_roles()[1].clone(),
    ]);
    assert!(matches!(
        ModuleSourceUnit::prepare(&fixture.input().with_toolchain(wrong_module)),
        Err(SourceFailure::Modules(
            declarations::ModuleSourcesFailure::Toolchain(
                nocter_declaration_lowering::ToolchainError::MissingPrimitiveDeclaration(
                    PrimitiveRole::NewError
                )
            )
        ))
    ));
    let create = "primitive func operation(code: str, message: str): error";
    let duplicate = Fixture::new(
        &[
            ("/app/index.nct", Root, ""),
            ("/app/error/index.nct", Root, create),
            (
                "/app/context/index.nct",
                Root,
                "primitive func operation(take value: error, message: str): error",
            ),
            ("/app/error/body.nct", Implementation, create),
        ],
        &[],
    )
    .with_modules(&[(&[], &[0]), (&["error"], &[1, 3]), (&["context"], &[2])]);
    assert!(matches!(
        ModuleSourceUnit::prepare(&duplicate.input().with_toolchain(profile)),
        Err(SourceFailure::Modules(
            declarations::ModuleSourcesFailure::Toolchain(
                nocter_declaration_lowering::ToolchainError::DuplicatePrimitiveDeclaration(
                    PrimitiveRole::NewError
                )
            )
        ))
    ));
}
