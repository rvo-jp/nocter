use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::{CallableContractError, analyze_callable_contracts};
use crate::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode, SurfaceDeclarationId,
    collect_declaration_surface,
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
    collect_declaration_surface(&CompileUnitInput::new(sources, vec![package], vec![module]))
        .unwrap()
}

#[test]
fn exact_contracts_and_bodies_share_the_contract_identity() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "pub func parse(\n    text: &str\n): usize\n\ninstance Text {\n    pub method &self.len(): usize\n}\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/parse.nct",
        "func parse(text: &str): usize { 0 }\n\ninstance Text {\n    method &self.len(): usize { 0 }\n}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::ModuleSource);
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
    );

    let contracts = analyze_callable_contracts(&surface).unwrap();

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
        "pub func parse(text: &str): usize\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/parse.nct",
        "func parse(text: usize): usize { text }\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::ModuleSource);
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
    );

    assert!(matches!(
        analyze_callable_contracts(&surface),
        Err(CallableContractError::MismatchedBody { .. })
    ));
}

#[test]
fn duplicate_matching_bodies_are_rejected_independent_of_source_order() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "pub func parse(text: &str): usize\n",
    );
    let first_id = add_source(
        &mut sources,
        "/app/a.nct",
        "func parse(text: &str): usize { 1 }\n",
    );
    let second_id = add_source(
        &mut sources,
        "/app/b.nct",
        "func parse(text: &str): usize { 2 }\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let first = parse_source(&sources, first_id, ParseGoal::ModuleSource);
    let second = parse_source(&sources, second_id, ParseGoal::ModuleSource);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/b.nct", ModuleSourceKind::Implementation, &second),
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new("/app/a.nct", ModuleSourceKind::Implementation, &first),
        ],
    );

    assert!(matches!(
        analyze_callable_contracts(&surface),
        Err(CallableContractError::DuplicateBody { .. })
    ));
}

#[test]
fn body_omission_is_not_a_general_callable_form() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(&mut sources, "/app/index.nct", "func unfinished(): void\n");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let surface = surface(
        &sources,
        &manifest,
        vec![ModuleSourceInput::new(
            "/app/index.nct",
            ModuleSourceKind::Root,
            &root,
        )],
    );

    assert!(matches!(
        analyze_callable_contracts(&surface),
        Err(CallableContractError::InvalidBodyOmission(_))
    ));
}

#[test]
fn coercion_bodies_use_the_same_contract_joining_rule() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "instance Text {\n    pub coerce &self as &str\n}\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/view.nct",
        "instance Text {\n    coerce &self as &str { self }\n}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::ModuleSource);
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
    );

    let contracts = analyze_callable_contracts(&surface).unwrap();

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
fn construction_body_omits_visibility_and_default_but_keeps_one_identity() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "struct Value { value: usize }\n\nconstruct Value {\n    pub default func new(): Self\n}\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/value.nct",
        "construct Value {\n    func new(): Self { Value { value: 0 } }\n}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::ModuleSource);
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
    );

    let contracts = analyze_callable_contracts(&surface).unwrap();

    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(5)),
        SurfaceDeclarationId::from_index(3)
    );
    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(4)),
        SurfaceDeclarationId::from_index(2)
    );
}
