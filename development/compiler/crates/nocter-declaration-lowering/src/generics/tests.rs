use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::{GenericError, PreparedGenerics, prepare_generic_binders};
use crate::test_support::source_see;
use crate::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageIdentity, PackageInput, PackageMode, SourceVisibilityResolutionInput,
    SurfaceDeclarationId, collect_declaration_surface, prepare_declaration_headers,
    reserve_declaration_identities,
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
    let tree = parse(sources.get(source).unwrap(), goal);
    assert!(!tree.has_errors());
    tree
}

fn prepare<'syntax>(
    sources: &'syntax SourceMap,
    _manifest: &'syntax SyntaxTree,
    module_sources: Vec<ModuleSourceInput<'syntax>>,
    source_visibility_resolutions: Vec<SourceVisibilityResolutionInput>,
) -> Result<PreparedGenerics<'syntax>, GenericError> {
    let package = PackageInput::new(
        PackageIdentity::new("workspace:app"),
        "app",
        PackageMode::Declared,
    );
    let module_identity =
        ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new());
    let toolchain = crate::test_support::empty_toolchain(module_identity.clone());
    let module = ModuleInput::new(module_identity, module_sources);
    let input = CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        sources,
        vec![package],
        vec![module],
        Vec::new(),
    )
    .with_source_visibility_resolutions(source_visibility_resolutions);
    let surface = collect_declaration_surface(&input).unwrap();
    let reserved = reserve_declaration_identities(surface, &toolchain).unwrap();
    let headers = prepare_declaration_headers(reserved).unwrap();
    prepare_generic_binders(headers)
}

#[test]
fn creates_owner_scopes_and_inherits_them_into_members() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "pub struct Pair<L, R> {\n    pub left: L\n    pub right: R\n}\n\ninstance Pair<T, T> {\n    pub method &self.replace<U>(value: U): U { value }\n}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);

    let generics = prepare(
        &sources,
        &manifest,
        vec![ModuleSourceInput::new(
            "/app/index.nct",
            ModuleSourceKind::Root,
            &root,
        )],
        Vec::new(),
    )
    .unwrap();
    let pair = SurfaceDeclarationId::from_index(0);
    let instance = SurfaceDeclarationId::from_index(3);
    let method = SurfaceDeclarationId::from_index(4);
    let symbols = generics.headers().reserved().symbols();
    let t = symbols.get("T").unwrap();
    let u = symbols.get("U").unwrap();

    assert_eq!(generics.own(pair).unwrap().len(), 2);
    assert_eq!(generics.own(instance).unwrap().len(), 1);
    assert_eq!(generics.own(method).unwrap().len(), 1);
    assert_eq!(
        generics.lookup(method, t),
        Some(generics.own(instance).unwrap()[0])
    );
    assert_eq!(
        generics.lookup(method, u),
        Some(generics.own(method).unwrap()[0])
    );
}

#[test]
fn repeated_pattern_names_reuse_one_identity_and_project_every_occurrence() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "pub interface Compare<T> {}\npub struct Pair<L, R> {\n    pub left: L\n    pub right: R\n}\nconform Compare<T> for Pair<T, T> {}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);

    let generics = prepare(
        &sources,
        &manifest,
        vec![ModuleSourceInput::new(
            "/app/index.nct",
            ModuleSourceKind::Root,
            &root,
        )],
        Vec::new(),
    )
    .unwrap();
    let conformance = SurfaceDeclarationId::from_index(4);

    assert_eq!(generics.own(conformance).unwrap().len(), 1);
    assert_eq!(generics.headers().reserved().source_binding_count(), 16);
}

#[test]
fn duplicate_explicit_binders_and_nested_shadowing_are_rejected() {
    for source_text in [
        "pub struct Broken<T, T> {}\n",
        "pub struct Pair<T> {}\ninstance Pair<T> {\n    pub method &self.identity<T>(value: T): T { value }\n}\n",
    ] {
        let mut sources = SourceMap::new();
        let manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let root_id = add_source(&mut sources, "/app/index.nct", source_text);
        let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
        let root = parse_source(&sources, root_id, ParseGoal::SourceFile);

        let error = prepare(
            &sources,
            &manifest,
            vec![ModuleSourceInput::new(
                "/app/index.nct",
                ModuleSourceKind::Root,
                &root,
            )],
            Vec::new(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            GenericError::Rule(violation)
                if matches!(
                    violation.rule(),
                    crate::GenericRule::DuplicateBinder | crate::GenericRule::ShadowingBinder
                )
        ));
    }
}

#[test]
fn joined_callable_sources_share_generic_identity() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "see ./identity.nct\n\npub func identity<T>(value: T): T\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/identity.nct",
        "see ./index.nct\n\nfunc identity<T>(value: T): T { value }\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);

    let generics = prepare(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/identity.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_see(&root, 0, "/app/identity.nct"),
            source_see(&implementation, 0, "/app/index.nct"),
        ],
    )
    .unwrap();
    let contract = SurfaceDeclarationId::from_index(0);
    let implementation = SurfaceDeclarationId::from_index(1);

    assert_eq!(generics.own(contract), generics.own(implementation));
    assert_eq!(generics.headers().reserved().source_binding_count(), 8);
}

#[test]
fn joined_construction_patterns_reuse_contract_binder_identities() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "see ./make.nct\n\npub struct Pair<L, R> {\n    pub left: L\n    pub right: R\n}\nconstruct Pair<L, R> {\n    pub func make(left: L, right: R): Self\n}\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/make.nct",
        "see ./index.nct\n\nconstruct Pair<L, R> {\n    func make(left: L, right: R): Self {\n        return Pair<L, R> { left: move left, right: move right }\n    }\n}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);

    let generics = prepare(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/make.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_see(&root, 0, "/app/make.nct"),
            source_see(&implementation, 0, "/app/index.nct"),
        ],
    )
    .unwrap();
    let contract = SurfaceDeclarationId::from_index(3);
    let implementation = SurfaceDeclarationId::from_index(5);

    assert_eq!(generics.own(contract), generics.own(implementation));
    assert_eq!(generics.own(contract).unwrap().len(), 2);
}
