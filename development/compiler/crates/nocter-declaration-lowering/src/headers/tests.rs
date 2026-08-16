use nocter_declarations::Visibility;
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::{HeaderError, prepare_declaration_headers};
use crate::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode, SurfaceDeclarationId,
    collect_declaration_surface, reserve_declaration_identities,
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

fn package(manifest: &SyntaxTree) -> PackageInput<'_> {
    PackageInput::new(
        PackageIdentity::new("workspace:app"),
        "app",
        PackageMode::Declared,
        Some(PackageDeclarationInput::new("/app/nocter.nct", manifest)),
    )
}

fn module<'syntax>(
    path: &[&str],
    sources: Vec<ModuleSourceInput<'syntax>>,
) -> ModuleInput<'syntax> {
    ModuleInput::new(
        ModuleIdentity::new(PackageIdentity::new("workspace:app"), path.iter().copied()),
        sources,
    )
}

#[test]
fn resolves_exact_name_tokens_and_creates_sites_for_fields() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "pub copy struct Value {\n    pub item: usize\n}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let input = CompileUnitInput::new(
        &sources,
        vec![package(&manifest)],
        vec![module(
            &[],
            vec![ModuleSourceInput::new(
                "/app/index.nct",
                ModuleSourceKind::Root,
                &root,
            )],
        )],
    );
    let reserved =
        reserve_declaration_identities(collect_declaration_surface(&input).unwrap()).unwrap();

    let headers = prepare_declaration_headers(reserved).unwrap();
    let value = headers.name(SurfaceDeclarationId::from_index(0)).unwrap();
    let item = headers.name(SurfaceDeclarationId::from_index(1)).unwrap();

    assert_eq!(headers.reserved().symbols().spelling(value), Some("Value"));
    assert_eq!(headers.reserved().symbols().spelling(item), Some("item"));
    assert!(headers.site(SurfaceDeclarationId::from_index(0)).is_some());
    assert!(headers.site(SurfaceDeclarationId::from_index(1)).is_some());
    assert_eq!(headers.reserved().source_binding_count(), 5);
}

#[test]
fn duplicate_module_names_are_order_independent() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "func duplicate(): void {}\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/other.nct",
        "func duplicate(value: usize): void {}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::ModuleSource);
    let input = CompileUnitInput::new(
        &sources,
        vec![package(&manifest)],
        vec![module(
            &[],
            vec![
                ModuleSourceInput::new(
                    "/app/other.nct",
                    ModuleSourceKind::Implementation,
                    &implementation,
                ),
                ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ],
        )],
    );
    let reserved =
        reserve_declaration_identities(collect_declaration_surface(&input).unwrap()).unwrap();

    assert!(matches!(
        prepare_declaration_headers(reserved),
        Err(HeaderError::DuplicateModuleName { .. })
    ));
}

#[test]
fn visibility_scopes_resolve_to_semantic_package_and_module_boundaries() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(&mut sources, "/app/index.nct", "func root(): void {}\n");
    let child_id = add_source(
        &mut sources,
        "/app/parser/index.nct",
        "pub(../) func ancestor(): void {}\npub(./) enum Local { item }\npub(/) func package(): void {}\npub func global(): void {}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let child = parse_source(&sources, child_id, ParseGoal::ModuleSource);
    let input = CompileUnitInput::new(
        &sources,
        vec![package(&manifest)],
        vec![
            module(
                &[],
                vec![ModuleSourceInput::new(
                    "/app/index.nct",
                    ModuleSourceKind::Root,
                    &root,
                )],
            ),
            module(
                &["parser"],
                vec![ModuleSourceInput::new(
                    "/app/parser/index.nct",
                    ModuleSourceKind::Root,
                    &child,
                )],
            ),
        ],
    );
    let reserved =
        reserve_declaration_identities(collect_declaration_surface(&input).unwrap()).unwrap();
    let root_module = reserved.module_ids()[0];
    let child_module = reserved.module_ids()[1];
    let package = reserved.package_ids()[0];

    let headers = prepare_declaration_headers(reserved).unwrap();

    assert_eq!(
        headers.visibility(SurfaceDeclarationId::from_index(1)),
        Some(Visibility::Descendants(root_module))
    );
    assert_eq!(
        headers.visibility(SurfaceDeclarationId::from_index(2)),
        Some(Visibility::Descendants(child_module))
    );
    assert_eq!(
        headers.visibility(SurfaceDeclarationId::from_index(3)),
        Some(Visibility::Descendants(child_module))
    );
    assert_eq!(
        headers.visibility(SurfaceDeclarationId::from_index(4)),
        Some(Visibility::Package(package))
    );
    assert_eq!(
        headers.visibility(SurfaceDeclarationId::from_index(5)),
        Some(Visibility::Public)
    );
}
