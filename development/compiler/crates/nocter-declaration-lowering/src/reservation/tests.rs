use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::{ReservedEntity, reserve_declaration_identities};
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

fn reserve<'syntax>(
    sources: &'syntax SourceMap,
    manifest: &'syntax SyntaxTree,
    module_sources: Vec<ModuleSourceInput<'syntax>>,
) -> super::ReservedDeclarations<'syntax> {
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
    let surface =
        collect_declaration_surface(&CompileUnitInput::new(sources, vec![package], vec![module]))
            .unwrap();
    reserve_declaration_identities(surface).unwrap()
}

#[test]
fn reserves_every_recursive_identity_domain_before_header_resolution() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/syntax/g007-g012-declarations.nct"
    ))
    .replace(
        "method &self.item(): &T from self",
        "method &self.item(): &T from self {}",
    );
    let root_id = add_source(&mut sources, "/app/index.nct", &root_text);
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);

    let reserved = reserve(
        &sources,
        &manifest,
        vec![ModuleSourceInput::new(
            "/app/index.nct",
            ModuleSourceKind::Root,
            &root,
        )],
    );

    assert!(matches!(
        reserved.entity(SurfaceDeclarationId::from_index(0)),
        Some(ReservedEntity::NominalType(_))
    ));
    assert!(matches!(
        reserved.entity(SurfaceDeclarationId::from_index(4)),
        Some(ReservedEntity::AssociatedType(_))
    ));
    assert!(matches!(
        reserved.entity(SurfaceDeclarationId::from_index(5)),
        Some(ReservedEntity::Callable(_))
    ));
    assert!(matches!(
        reserved.entity(SurfaceDeclarationId::from_index(6)),
        Some(ReservedEntity::Construction(_))
    ));
    assert!(reserved.entities().iter().all(Option::is_some));
}

#[test]
fn contract_and_implementation_receive_one_callable_identity() {
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
        "func parse(text: &str): usize { 0 }\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::ModuleSource);

    let reserved = reserve(
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

    assert_eq!(reserved.entities()[0], reserved.entities()[1]);
    assert!(matches!(
        reserved.entities()[0],
        Some(ReservedEntity::Callable(_))
    ));
}

#[test]
fn reservation_ids_do_not_depend_on_implementation_discovery_order() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(&mut sources, "/app/index.nct", "func root(): void {}\n");
    let first_id = add_source(&mut sources, "/app/a.nct", "func alpha(): void {}\n");
    let second_id = add_source(&mut sources, "/app/z.nct", "func omega(): void {}\n");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let first = parse_source(&sources, first_id, ParseGoal::ModuleSource);
    let second = parse_source(&sources, second_id, ParseGoal::ModuleSource);
    let source_order = vec![
        ModuleSourceInput::new("/app/z.nct", ModuleSourceKind::Implementation, &second),
        ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
        ModuleSourceInput::new("/app/a.nct", ModuleSourceKind::Implementation, &first),
    ];

    let forward = reserve(&sources, &manifest, source_order.clone());
    let reverse = reserve(
        &sources,
        &manifest,
        source_order.into_iter().rev().collect(),
    );

    assert_eq!(forward.entities(), reverse.entities());
}
