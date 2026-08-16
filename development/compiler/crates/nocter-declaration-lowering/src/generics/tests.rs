use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::{GenericError, PreparedGenerics, prepare_generic_binders};
use crate::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode, SurfaceDeclarationId,
    collect_declaration_surface, prepare_declaration_headers, reserve_declaration_identities,
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
    manifest: &'syntax SyntaxTree,
    module_sources: Vec<ModuleSourceInput<'syntax>>,
) -> Result<PreparedGenerics<'syntax>, GenericError> {
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
    let input = CompileUnitInput::new(sources, vec![package], vec![module]);
    let surface = collect_declaration_surface(&input).unwrap();
    let reserved = reserve_declaration_identities(surface).unwrap();
    let headers = prepare_declaration_headers(reserved).unwrap();
    prepare_generic_binders(headers)
}

#[test]
fn creates_owner_scopes_and_inherits_them_into_members() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "pub struct Pair<L, R> {\n    pub left: L\n    pub right: R\n}\n\ninstance Pair<T, T> {\n    pub method &self.replace<U>(value: U): U { value }\n}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);

    let generics = prepare(
        &sources,
        &manifest,
        vec![ModuleSourceInput::new(
            "/app/index.nct",
            ModuleSourceKind::Root,
            &root,
        )],
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
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "pub interface Compare<T> {}\npub struct Pair<L, R> {\n    pub left: L\n    pub right: R\n}\nconform Compare<T> for Pair<T, T> {}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);

    let generics = prepare(
        &sources,
        &manifest,
        vec![ModuleSourceInput::new(
            "/app/index.nct",
            ModuleSourceKind::Root,
            &root,
        )],
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
        let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let root_id = add_source(&mut sources, "/app/index.nct", source_text);
        let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
        let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);

        assert!(matches!(
            prepare(
                &sources,
                &manifest,
                vec![ModuleSourceInput::new(
                    "/app/index.nct",
                    ModuleSourceKind::Root,
                    &root,
                )],
            ),
            Err(GenericError::DuplicateBinder(_))
        ));
    }
}

#[test]
fn joined_callable_sources_share_generic_identity() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "pub func identity<T>(value: T): T\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/identity.nct",
        "func identity<T>(value: T): T { value }\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::ModuleSource);

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
    )
    .unwrap();
    let contract = SurfaceDeclarationId::from_index(0);
    let implementation = SurfaceDeclarationId::from_index(1);

    assert_eq!(generics.own(contract), generics.own(implementation));
    assert_eq!(generics.headers().reserved().source_binding_count(), 8);
}
