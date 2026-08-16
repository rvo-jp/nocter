use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::{SurfaceDeclarationKind, SurfaceError, collect_declaration_surface};
use crate::test_support::{module_use, source_use};
use crate::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode,
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

fn root_module(sources: Vec<ModuleSourceInput<'_>>) -> ModuleInput<'_> {
    ModuleInput::new(
        ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new()),
        sources,
    )
}

#[test]
fn inventories_every_reservable_declaration_with_its_exact_owner() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "#name: \"app\"\n");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g007-g012-declarations.nct"
        )),
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let input = CompileUnitInput::new(
        &sources,
        vec![package(&manifest)],
        vec![root_module(vec![ModuleSourceInput::new(
            "/app/index.nct",
            ModuleSourceKind::Root,
            &root,
        )])],
        Vec::new(),
    );

    let surface = collect_declaration_surface(&input).unwrap();
    let actual: Vec<_> = surface
        .declarations()
        .iter()
        .map(|declaration| {
            (
                declaration.kind(),
                declaration.owner().map(super::SurfaceDeclarationId::index),
            )
        })
        .collect();

    assert_eq!(
        actual,
        [
            (SurfaceDeclarationKind::Enum, None),
            (SurfaceDeclarationKind::Variant, Some(0)),
            (SurfaceDeclarationKind::Variant, Some(0)),
            (SurfaceDeclarationKind::Interface, None),
            (SurfaceDeclarationKind::AssociatedType, Some(3)),
            (SurfaceDeclarationKind::InterfaceMethod, Some(3)),
            (SurfaceDeclarationKind::Construction, None),
            (SurfaceDeclarationKind::ConstructionFunction, Some(6)),
            (SurfaceDeclarationKind::Instance, None),
            (SurfaceDeclarationKind::InherentMethod, Some(8)),
            (SurfaceDeclarationKind::Conformance, None),
            (SurfaceDeclarationKind::ConformanceMethod, Some(10)),
            (SurfaceDeclarationKind::Drop, None),
            (SurfaceDeclarationKind::Test, None),
        ]
    );
}

#[test]
fn retains_resolved_import_edges_and_unresolved_item_target_syntax() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "#name: \"app\"\n");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g002-g006-module.nct"
        )),
    );
    let parser_id = add_source(&mut sources, "/app/parser/index.nct", "");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let parser = parse_source(&sources, parser_id, ParseGoal::ModuleSource);
    let parser_identity = ModuleIdentity::new(PackageIdentity::new("workspace:app"), ["parser"]);
    let input = CompileUnitInput::new(
        &sources,
        vec![package(&manifest)],
        vec![
            root_module(vec![ModuleSourceInput::new(
                "/app/index.nct",
                ModuleSourceKind::Root,
                &root,
            )]),
            ModuleInput::new(
                parser_identity.clone(),
                vec![ModuleSourceInput::new(
                    "/app/parser/index.nct",
                    ModuleSourceKind::Root,
                    &parser,
                )],
            ),
        ],
        vec![module_use(&root, 0, parser_identity)],
    );

    let surface = collect_declaration_surface(&input).unwrap();

    assert_eq!(surface.imports().len(), 1);
    assert_eq!(surface.declarations().len(), 1);
    assert_eq!(
        surface.declarations()[0].kind(),
        SurfaceDeclarationKind::Function
    );
    assert!(surface.declarations()[0].target_gate().is_some());
}

#[test]
fn canonical_source_order_is_independent_of_discovery_order() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "use ./a\nuse ./z\n\nfunc root(): void {}\n",
    );
    let a_id = add_source(&mut sources, "/app/a.nct", "func alpha(): void {}\n");
    let z_id = add_source(&mut sources, "/app/z.nct", "func omega(): void {}\n");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let a = parse_source(&sources, a_id, ParseGoal::ModuleSource);
    let z = parse_source(&sources, z_id, ParseGoal::ModuleSource);
    let module_sources = vec![
        ModuleSourceInput::new("/app/z.nct", ModuleSourceKind::Implementation, &z),
        ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
        ModuleSourceInput::new("/app/a.nct", ModuleSourceKind::Implementation, &a),
    ];
    let resolutions = vec![
        source_use(&root, 0, "/app/a.nct"),
        source_use(&root, 1, "/app/z.nct"),
    ];
    let forward = CompileUnitInput::new(
        &sources,
        vec![package(&manifest)],
        vec![root_module(module_sources.clone())],
        resolutions.clone(),
    );
    let reverse = CompileUnitInput::new(
        &sources,
        vec![package(&manifest)],
        vec![root_module(module_sources.into_iter().rev().collect())],
        resolutions,
    );

    let forward = collect_declaration_surface(&forward).unwrap();
    let reverse = collect_declaration_surface(&reverse).unwrap();

    assert_eq!(forward.declarations(), reverse.declarations());
    assert_eq!(
        forward
            .sources()
            .iter()
            .map(super::SurfaceSource::canonical_path)
            .collect::<Vec<_>>(),
        ["/app/index.nct", "/app/a.nct", "/app/z.nct"]
    );
}

#[test]
fn implementation_sources_cannot_expand_the_module_surface() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "use ./implementation\n\nfunc root(): void {}\n",
    );
    let public_id = add_source(
        &mut sources,
        "/app/public.nct",
        "pub func exposed(): void {}\n",
    );
    let field_id = add_source(
        &mut sources,
        "/app/record.nct",
        "struct Hidden { value: usize }\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let public = parse_source(&sources, public_id, ParseGoal::ModuleSource);
    let field = parse_source(&sources, field_id, ParseGoal::ModuleSource);

    for implementation in [&public, &field] {
        let input = CompileUnitInput::new(
            &sources,
            vec![package(&manifest)],
            vec![root_module(vec![
                ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
                ModuleSourceInput::new(
                    "/app/implementation.nct",
                    ModuleSourceKind::Implementation,
                    implementation,
                ),
            ])],
            vec![source_use(&root, 0, "/app/implementation.nct")],
        );

        let error = collect_declaration_surface(&input).unwrap_err();
        assert!(matches!(
            error,
            SurfaceError::ImplementationVisibility(_) | SurfaceError::ImplementationMember(_)
        ));
    }
}
