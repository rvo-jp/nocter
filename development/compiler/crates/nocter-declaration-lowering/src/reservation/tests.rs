use nocter_source::{SourceMap, SourceName};
use nocter_source_index::SemanticEntity;
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::{ReservedEntity, reserve_declaration_identities};
use crate::test_support::source_see;
use crate::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageIdentity, PackageInput, PackageMode, SourceVisibilityResolutionInput,
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

fn reserve<'syntax>(
    sources: &'syntax SourceMap,
    _manifest: &'syntax SyntaxTree,
    module_sources: Vec<ModuleSourceInput<'syntax>>,
    source_visibility_resolutions: Vec<SourceVisibilityResolutionInput>,
) -> super::ReservedDeclarations<'syntax> {
    let package = PackageInput::new(
        PackageIdentity::new("workspace:app"),
        "app",
        PackageMode::Declared,
    );
    let module_identity =
        ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new());
    let toolchain = crate::test_support::empty_toolchain(module_identity.clone());
    let module = ModuleInput::new(module_identity, module_sources);
    let surface = collect_declaration_surface(
        &CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            sources,
            vec![package],
            vec![module],
            Vec::new(),
        )
        .with_source_visibility_resolutions(source_visibility_resolutions),
    )
    .unwrap();
    reserve_declaration_identities(surface, &toolchain).unwrap()
}

#[test]
fn reserves_every_recursive_identity_domain_before_header_resolution() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let root_text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/syntax/g007-g012-declarations.nct"
    ))
    .replace(
        "method &self.item(): &T from self",
        "method &self.item(): &T from self {}",
    );
    let root_id = add_source(&mut sources, "/app/index.nct", &root_text);
    let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);

    let reserved = reserve(
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
    let manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "see ./parse.nct\n\npub func parse(text: &str): usize\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/parse.nct",
        "see ./index.nct\n\nfunc parse(text: &str): usize { 0 }\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);

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
        vec![
            source_see(&root, 0, "/app/parse.nct"),
            source_see(&implementation, 0, "/app/index.nct"),
        ],
    );

    assert_eq!(reserved.entities()[0], reserved.entities()[1]);
    assert!(matches!(
        reserved.entities()[0],
        Some(ReservedEntity::Callable(_))
    ));
    assert_eq!(
        reserved.declaration_for_entity(reserved.entities()[0].unwrap()),
        Some(SurfaceDeclarationId::from_index(0))
    );
}

#[test]
fn public_file_documentation_has_one_semantic_owner() {
    let mut sources = SourceMap::new();
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "//! Package documentation.\n\nsee ./detail.nct\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/detail.nct",
        "//! Implementation source documentation.\n",
    );
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);

    let reserved = reserve(
        &sources,
        &root,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/detail.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![source_see(&root, 0, "/app/detail.nct")],
    );
    let package = reserved.package_ids()[0];
    let module = reserved.module_ids()[0];
    let (_, source_index, _) = reserved
        .source_index
        .finish(reserved.source_map, &reserved.sources)
        .unwrap();

    assert_eq!(
        source_index.documentation(SemanticEntity::Package(package)),
        Some("Package documentation.")
    );
    assert_eq!(
        source_index.documentation(SemanticEntity::Module(module)),
        None,
        "the package root source has one documentation owner"
    );
    assert_eq!(
        implementation.file_documentation(),
        Some("Implementation source documentation."),
        "implementation documentation stays available only on its syntax snapshot"
    );
}

#[test]
fn reservation_ids_do_not_depend_on_implementation_discovery_order() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "see ./a.nct\nsee ./z.nct\n\nfunc root(): void {}\n",
    );
    let first_id = add_source(&mut sources, "/app/a.nct", "func alpha(): void {}\n");
    let second_id = add_source(&mut sources, "/app/z.nct", "func omega(): void {}\n");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let first = parse_source(&sources, first_id, ParseGoal::SourceFile);
    let second = parse_source(&sources, second_id, ParseGoal::SourceFile);
    let source_order = vec![
        ModuleSourceInput::new("/app/z.nct", ModuleSourceKind::Implementation, &second),
        ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
        ModuleSourceInput::new("/app/a.nct", ModuleSourceKind::Implementation, &first),
    ];

    let resolutions = vec![
        source_see(&root, 0, "/app/a.nct"),
        source_see(&root, 1, "/app/z.nct"),
    ];
    let forward = reserve(
        &sources,
        &manifest,
        source_order.clone(),
        resolutions.clone(),
    );
    let reverse = reserve(
        &sources,
        &manifest,
        source_order.into_iter().rev().collect(),
        resolutions,
    );

    assert_eq!(forward.entities(), reverse.entities());
}
