use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::{DeclarationContractError, analyze_declaration_contracts};
use crate::test_support::source_include;
use crate::{
    CompileUnitInput, IncludeResolutionInput, ModuleIdentity, ModuleInput, ModuleSourceInput,
    ModuleSourceKind, PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode,
    SurfaceDeclarationId, collect_declaration_surface,
};

fn add_source(sources: &mut SourceMap, name: &str, text: &str) -> nocter_source::SourceId {
    sources
        .add_bytes(SourceName::new(name), text.as_bytes())
        .unwrap()
}

fn parse_source(
    sources: &SourceMap,
    source: nocter_source::SourceId,
    goal: ParseGoal,
) -> SyntaxTree {
    parse(sources.get(source).unwrap(), goal)
}

fn surface<'syntax>(
    sources: &'syntax SourceMap,
    manifest: &'syntax SyntaxTree,
    module_sources: Vec<ModuleSourceInput<'syntax>>,
    include_resolutions: Vec<IncludeResolutionInput>,
) -> crate::DeclarationSurface<'syntax> {
    let package = PackageInput::new(
        PackageIdentity::new("workspace:app"),
        "app",
        PackageMode::Declared,
        Some(PackageDeclarationInput::new("/app/nocter.nct", manifest)),
    );
    let module = ModuleInput::new(
        ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new()),
        module_sources,
    );
    collect_declaration_surface(
        &CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            sources,
            vec![package],
            vec![module],
            Vec::new(),
        )
        .with_include_resolutions(include_resolutions),
    )
    .unwrap()
}

#[test]
fn exact_contracts_and_bodies_share_the_contract_identity() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "include ./parse.nct\n\npub func parse(\n    text: &str\n): usize\n\ninstance Text {\n    pub method &self.len(): usize\n}\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/parse.nct",
        "include ./index.nct\n\nfunc parse(text: &str): usize { 0 }\n\ninstance Text {\n    method &self.len(): usize { 0 }\n}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/parse.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/parse.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    let contracts = analyze_declaration_contracts(&surface).unwrap();

    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(3)),
        SurfaceDeclarationId::from_index(0)
    );
    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(5)),
        SurfaceDeclarationId::from_index(2)
    );
}

#[test]
fn same_callable_label_with_a_different_header_is_a_mismatch() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "include ./parse.nct\n\npub func parse(text: &str): usize\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/parse.nct",
        "include ./index.nct\n\nfunc parse(text: usize): usize { text }\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/parse.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/parse.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    assert!(matches!(
        analyze_declaration_contracts(&surface),
        Err(DeclarationContractError::MismatchedBody { .. })
    ));
}

#[test]
fn duplicate_matching_bodies_are_rejected_independent_of_source_order() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "include ./a.nct\ninclude ./b.nct\n\npub func parse(text: &str): usize\n",
    );
    let first_id = add_source(
        &mut sources,
        "/app/a.nct",
        "include ./index.nct\n\nfunc parse(text: &str): usize { 1 }\n",
    );
    let second_id = add_source(
        &mut sources,
        "/app/b.nct",
        "include ./index.nct\n\nfunc parse(text: &str): usize { 2 }\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let first = parse_source(&sources, first_id, ParseGoal::SourceFile);
    let second = parse_source(&sources, second_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/b.nct", ModuleSourceKind::Implementation, &second),
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new("/app/a.nct", ModuleSourceKind::Implementation, &first),
        ],
        vec![
            source_include(&root, 0, "/app/a.nct"),
            source_include(&root, 1, "/app/b.nct"),
            source_include(&first, 0, "/app/index.nct"),
            source_include(&second, 0, "/app/index.nct"),
        ],
    );

    assert!(matches!(
        analyze_declaration_contracts(&surface),
        Err(DeclarationContractError::DuplicateBody { .. })
    ));
}

#[test]
fn body_omission_is_not_a_general_callable_form() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(&mut sources, "/app/index.nct", "func unfinished(): void\n");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![ModuleSourceInput::new(
            "/app/index.nct",
            ModuleSourceKind::Root,
            &root,
        )],
        Vec::new(),
    );

    assert!(matches!(
        analyze_declaration_contracts(&surface),
        Err(DeclarationContractError::InvalidBodyOmission(_))
    ));
}

#[test]
fn coercion_bodies_use_the_same_contract_joining_rule() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "include ./view.nct\n\ninstance Text {\n    pub coerce &self as &str\n}\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/view.nct",
        "include ./index.nct\n\ninstance Text {\n    coerce &self as &str { self }\n}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/view.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/view.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    let contracts = analyze_declaration_contracts(&surface).unwrap();

    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(3)),
        SurfaceDeclarationId::from_index(1)
    );
    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(2)),
        SurfaceDeclarationId::from_index(0)
    );
}

#[test]
fn construction_body_omits_visibility_but_repeats_default_and_keeps_one_identity() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "include ./value.nct\n\nstruct Value { value: usize }\n\nconstruct Value {\n    pub default func new(): Self\n}\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/value.nct",
        "include ./index.nct\n\nconstruct Value {\n    default func new(): Self { Value { value: 0 } }\n}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    assert!(!implementation.has_errors());
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/value.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/value.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    let contracts = analyze_declaration_contracts(&surface).unwrap();

    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(5)),
        SurfaceDeclarationId::from_index(3)
    );
    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(4)),
        SurfaceDeclarationId::from_index(2)
    );
}

#[test]
fn opaque_nominal_contract_and_private_representation_share_one_identity() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "include ./string.nct\n\npub struct String\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/string.nct",
        "include ./index.nct\n\nstruct String { len: usize }\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/string.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/string.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    let contracts = analyze_declaration_contracts(&surface).unwrap();
    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(1)),
        SurfaceDeclarationId::from_index(0)
    );
}
