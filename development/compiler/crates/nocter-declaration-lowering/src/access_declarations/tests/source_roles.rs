//! File roles constrain one grammar; neither role checking nor omitted bodies publish a Unit.
use super::*;
use nocter_compile_input::ModuleSourceKind;

#[test]
fn contract_shapes_are_recognized_without_implementation_or_type_evidence() {
    for text in [
        "func answer(value: Missing): Missing",
        "const VALUE: Missing\nstatic ROOT: Missing",
        "struct Opaque\ncopy struct Visible { value: Missing }",
        "func sized(): [i32; if true { 1 } else { 2 }]",
        "primitive func selected(code: str, message: str): error",
    ] {
        let (sources, tree) = input(text, ParseGoal::V2CoreUnit);
        let syntax = bound(&sources, &tree);
        assert!(!tree.has_errors(), "{text}");
        validate_source_role(syntax, ModuleSourceKind::Root).unwrap();
    }
}

#[test]
fn implementation_sources_require_definitions_before_type_normalization() {
    for role in [
        ModuleSourceKind::Implementation,
        ModuleSourceKind::SingleFile,
    ] {
        for (text, expected) in [
            (
                "func answer(value: Missing): Missing",
                SourceRoleRule::MissingBody,
            ),
            ("const VALUE: Missing", SourceRoleRule::MissingInitializer),
            ("static ROOT: Missing", SourceRoleRule::MissingInitializer),
            ("struct Opaque", SourceRoleRule::MissingRepresentation),
        ] {
            let (sources, tree) = input(text, ParseGoal::V2CoreUnit);
            let Err(Failure::SourceRole(diagnostic)) =
                validate_source_role(bound(&sources, &tree), role)
            else {
                panic!("{text}")
            };
            assert_eq!(diagnostic.rule(), expected);
            assert!(!diagnostic.source().primary().span().range().is_empty());
        }
    }
    let (sources, tree) = input("func answer(): i32", ParseGoal::V2CoreUnit);
    assert!(
        matches!(Query::prepare(bound(&sources, &tree)), Err(Failure::SourceRole(diagnostic)) if diagnostic.rule() == SourceRoleRule::MissingBody)
    );
}

#[test]
fn contract_roots_reject_executable_bodies_and_value_initializers() {
    for (text, expected, subject) in [
        (
            "func answer(value: Missing): Missing { 42 }",
            SourceRoleRule::BodyInContract,
            "{ 42 }",
        ),
        (
            "const VALUE: Missing = 42",
            SourceRoleRule::InitializerInContract,
            "42",
        ),
        (
            "static ROOT: Missing = 42",
            SourceRoleRule::InitializerInContract,
            "42",
        ),
    ] {
        let (sources, tree) = input(text, ParseGoal::V2CoreUnit);
        let syntax = bound(&sources, &tree);
        let Err(Failure::SourceRole(diagnostic)) =
            validate_source_role(syntax, ModuleSourceKind::Root)
        else {
            panic!("{text}")
        };
        assert_eq!(diagnostic.rule(), expected);
        assert_eq!(diagnostic.source().code(), "E0237");
        assert_eq!(
            syntax.text_at(diagnostic.source().primary().span().range()),
            Some(subject)
        );
    }
}

#[test]
fn role_checks_do_not_replace_primitive_authority_or_change_role_from_filename() {
    let (sources, tree) = input(
        "primitive func selected(code: str, message: str): error",
        ParseGoal::V2CoreUnit,
    );
    let syntax = bound(&sources, &tree);
    validate_source_role(syntax, ModuleSourceKind::Implementation).unwrap();
    assert!(
        matches!(Query::prepare(syntax), Err(Failure::Primitive(diagnostic)) if diagnostic.rule() == PrimitiveRule::Unauthorized)
    );
    let mut sources = SourceMap::new();
    let id = sources
        .add_bytes(SourceName::new("index.nct"), b"func answer(): i32 { 42 }")
        .unwrap();
    let tree = parse(sources.get(id).unwrap(), ParseGoal::V2CoreUnit);
    let syntax = bound(&sources, &tree);
    validate_source_role(syntax, ModuleSourceKind::Implementation).unwrap();
    assert!(matches!(
        validate_source_role(syntax, ModuleSourceKind::Root),
        Err(Failure::SourceRole(_))
    ));
}

#[test]
fn destructor_bodies_belong_in_implementation_sources() {
    let (sources, tree) = input("impl Handle { drop {} }", ParseGoal::V2CoreUnit);
    assert!(!tree.has_errors());
    let syntax = bound(&sources, &tree);
    validate_source_role(syntax, ModuleSourceKind::Implementation).unwrap();
    assert!(matches!(
        validate_source_role(syntax, ModuleSourceKind::Root),
        Err(Failure::SourceRole(diagnostic)) if diagnostic.rule() == SourceRoleRule::BodyInContract
    ));
}

#[test]
fn visibility_is_a_root_contract_directive_not_an_implementation_or_single_file_mode() {
    for text in [
        "#visibility: \"/\"\nfunc run(): i32 { 42 }",
        "struct Data {\n#visibility: \"/\"\nvalue: i32\n}",
        "#visibility: \"/\"\nuse ./codec",
    ] {
        let (sources, tree) = input(text, ParseGoal::V2CoreUnit);
        assert!(!tree.has_errors(), "{text}: {:?}", tree.diagnostics());
        for role in [
            ModuleSourceKind::Implementation,
            ModuleSourceKind::SingleFile,
        ] {
            let Err(Failure::SourceRole(diagnostic)) =
                validate_source_role(bound(&sources, &tree), role)
            else {
                panic!("visibility placement")
            };
            assert_eq!(diagnostic.rule(), SourceRoleRule::VisibilityOutsideContract);
            assert_eq!(diagnostic.source().code(), "E0238");
        }
    }
    let (sources, tree) = input("#visibility: \"/\"\nfunc run(): i32", ParseGoal::V2CoreUnit);
    validate_source_role(bound(&sources, &tree), ModuleSourceKind::Root).unwrap();
}

#[test]
fn package_prefix_does_not_authorize_single_source_or_executable_contracts() {
    let prefix = "#package: { name: \"app\", version: \"2.0.0\" }\n";
    let (sources, tree) = input(prefix, ParseGoal::V2CoreUnit);
    let syntax = bound(&sources, &tree);
    validate_source_role(syntax, ModuleSourceKind::Root).unwrap();
    for role in [
        ModuleSourceKind::Implementation,
        ModuleSourceKind::SingleFile,
    ] {
        let Err(Failure::SourceRole(error)) = validate_source_role(syntax, role) else {
            panic!("package placement")
        };
        assert_eq!(error.rule(), SourceRoleRule::PackageMetadataOutsideContract);
        assert_eq!(error.source().code(), "E0276");
    }
    assert!(matches!(
        Query::prepare(syntax),
        Err(Failure::SourceRole(_))
    ));
    for body in ["func run(): i32 { 42 }", "const RESULT: i32 = 42"] {
        let (sources, tree) = input(&format!("{prefix}{body}"), ParseGoal::V2CoreUnit);
        assert!(matches!(
            validate_source_role(bound(&sources, &tree), ModuleSourceKind::Root),
            Err(Failure::SourceRole(_))
        ));
    }
}
