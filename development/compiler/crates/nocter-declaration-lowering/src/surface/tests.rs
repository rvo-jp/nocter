use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::{SurfaceDeclarationKind, SurfaceError, collect_declaration_surface};
use crate::test_support::{module_use, source_see};
use crate::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageIdentity, PackageInput, PackageMode,
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

fn package(_manifest: &SyntaxTree) -> PackageInput {
    PackageInput::new(
        PackageIdentity::new("workspace:app"),
        "app",
        PackageMode::Declared,
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
    let manifest_id = add_source(
        &mut sources,
        "/app/index.nct",
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
    );
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g007-g012-declarations.nct"
        )),
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let input = CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
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
    let manifest_id = add_source(
        &mut sources,
        "/app/index.nct",
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
    );
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/syntax/g002-g006-module.nct"
        )),
    );
    let parser_id = add_source(&mut sources, "/app/parser/index.nct", "");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let parser = parse_source(&sources, parser_id, ParseGoal::SourceFile);
    let parser_identity = ModuleIdentity::new(PackageIdentity::new("workspace:app"), ["parser"]);
    let input = CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
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
    let manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "see ./a.nct\nsee ./z.nct\n\nfunc root(): void {}\n",
    );
    let a_id = add_source(&mut sources, "/app/a.nct", "func alpha(): void {}\n");
    let z_id = add_source(&mut sources, "/app/z.nct", "func omega(): void {}\n");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let a = parse_source(&sources, a_id, ParseGoal::SourceFile);
    let z = parse_source(&sources, z_id, ParseGoal::SourceFile);
    let module_sources = vec![
        ModuleSourceInput::new("/app/z.nct", ModuleSourceKind::Implementation, &z),
        ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
        ModuleSourceInput::new("/app/a.nct", ModuleSourceKind::Implementation, &a),
    ];
    let resolutions = vec![
        source_see(&root, 0, "/app/a.nct"),
        source_see(&root, 1, "/app/z.nct"),
    ];
    let forward = CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        &sources,
        vec![package(&manifest)],
        vec![root_module(module_sources.clone())],
        Vec::new(),
    )
    .with_source_visibility_resolutions(resolutions.clone());
    let reverse = CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        &sources,
        vec![package(&manifest)],
        vec![root_module(module_sources.into_iter().rev().collect())],
        Vec::new(),
    )
    .with_source_visibility_resolutions(resolutions);

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
fn implementation_sources_are_private_but_may_define_nominal_representation() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "see ./implementation.nct\n\nfunc root(): void {}\n",
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
    let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let public = parse_source(&sources, public_id, ParseGoal::SourceFile);
    let field = parse_source(&sources, field_id, ParseGoal::SourceFile);

    for (implementation, should_fail) in [(&public, true), (&field, false)] {
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
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
            Vec::new(),
        )
        .with_source_visibility_resolutions(vec![source_see(
            &root,
            0,
            "/app/implementation.nct",
        )]);

        let result = collect_declaration_surface(&input);
        assert_eq!(result.is_err(), should_fail);
        if let Err(error) = result {
            assert!(matches!(error, SurfaceError::ImplementationVisibility(_)));
        }
    }
}

#[test]
fn target_selection_excludes_the_complete_inactive_item_from_frontend_inputs() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "#target: \"arm64-darwin\"\n\
         func platform(): void {}\n\
         #target: \"x64-linux\"\n\
         func platform(): void {\n\
             use ghost.missing\n\n\
             dormant_symbol()\n\
         }\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    assert!(!root.has_errors(), "{:#?}", root.diagnostics());
    let input = CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        &sources,
        vec![package(&manifest)],
        vec![root_module(vec![ModuleSourceInput::new(
            "/app/index.nct",
            ModuleSourceKind::Root,
            &root,
        )])],
        vec![module_use(
            &root,
            0,
            ModuleIdentity::new(PackageIdentity::new("workspace:app"), ["ghost"]),
        )],
    );

    let surface = collect_declaration_surface(&input).unwrap();

    assert_eq!(
        surface.target(),
        nocter_model::CompilationTarget::Arm64Darwin
    );
    assert_eq!(surface.declarations().len(), 1);
    assert!(surface.declarations()[0].target_gate().is_some());
    assert!(surface.symbols().get("platform").is_some());
    assert!(surface.symbols().get("dormant_symbol").is_none());
    assert!(surface.symbols().get("x64-linux").is_none());
}

#[test]
fn unknown_target_gate_is_an_authored_surface_error() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "#target: \"mips-templeos\"\nfunc platform(): void {}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    assert!(!root.has_errors(), "{:#?}", root.diagnostics());
    let input = CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        &sources,
        vec![package(&manifest)],
        vec![root_module(vec![ModuleSourceInput::new(
            "/app/index.nct",
            ModuleSourceKind::Root,
            &root,
        )])],
        Vec::new(),
    );

    assert!(matches!(
        collect_declaration_surface(&input),
        Err(SurfaceError::UnknownTargetGate(_))
    ));
}
