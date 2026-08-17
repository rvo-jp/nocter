use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::{LoweringError, lower_compile_unit_topology};
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

fn declared_package<'syntax>(
    identity: &str,
    name: &str,
    path: &str,
    declaration: &'syntax SyntaxTree,
) -> PackageInput<'syntax> {
    PackageInput::new(
        PackageIdentity::new(identity),
        name,
        PackageMode::Declared,
        Some(PackageDeclarationInput::new(path, declaration)),
    )
}

fn root_module<'syntax>(
    package: &str,
    sources: Vec<ModuleSourceInput<'syntax>>,
) -> ModuleInput<'syntax> {
    ModuleInput::new(
        ModuleIdentity::new(PackageIdentity::new(package), Vec::<&str>::new()),
        sources,
    )
}

fn child_module<'syntax>(
    package: &str,
    path: &[&str],
    sources: Vec<ModuleSourceInput<'syntax>>,
) -> ModuleInput<'syntax> {
    ModuleInput::new(
        ModuleIdentity::new(PackageIdentity::new(package), path.iter().copied()),
        sources,
    )
}

#[test]
fn input_order_does_not_change_semantic_topology() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "#name: \"app\"\n");
    let app_root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "use ./support\n\npub func run(): void { return }\n",
    );
    let app_impl_id = add_source(
        &mut sources,
        "/app/support.nct",
        "func support(value: usize): usize { value }\n",
    );
    let dep_manifest_id = add_source(&mut sources, "/dep/nocter.nct", "#name: \"dep\"\n");
    let dep_root_id = add_source(
        &mut sources,
        "/dep/index.nct",
        "pub struct Item { value: usize }\n",
    );

    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let app_root = parse_source(&sources, app_root_id, ParseGoal::ModuleSource);
    let app_impl = parse_source(&sources, app_impl_id, ParseGoal::ModuleSource);
    let dep_manifest = parse_source(&sources, dep_manifest_id, ParseGoal::PackageFile);
    let dep_root = parse_source(&sources, dep_root_id, ParseGoal::ModuleSource);

    let packages = vec![
        declared_package("resolved:dep", "dep", "/dep/nocter.nct", &dep_manifest),
        declared_package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
    ];
    let modules = vec![
        root_module(
            "resolved:dep",
            vec![ModuleSourceInput::new(
                "/dep/index.nct",
                ModuleSourceKind::Root,
                &dep_root,
            )],
        ),
        root_module(
            "workspace:app",
            vec![
                ModuleSourceInput::new(
                    "/app/support.nct",
                    ModuleSourceKind::Implementation,
                    &app_impl,
                ),
                ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &app_root),
            ],
        ),
    ];
    let resolutions = vec![source_use(&app_root, 0, "/app/support.nct")];

    let forward = lower_compile_unit_topology(&CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        &sources,
        packages.clone(),
        modules.clone(),
        resolutions.clone(),
    ))
    .unwrap();
    let reverse = lower_compile_unit_topology(&CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        &sources,
        packages.into_iter().rev().collect(),
        modules
            .into_iter()
            .rev()
            .map(|module| {
                let mut source_order = module.sources().to_vec();
                source_order.reverse();
                ModuleInput::new(module.identity().clone(), source_order)
            })
            .collect(),
        resolutions,
    ))
    .unwrap();

    assert_eq!(forward.program().symbols(), reverse.program().symbols());
    assert_eq!(
        forward.program().target(),
        nocter_model::CompilationTarget::Arm64Darwin
    );
    assert_eq!(forward.program().packages(), reverse.program().packages());
    assert_eq!(forward.program().modules(), reverse.program().modules());
    assert_eq!(forward.source_index(), reverse.source_index());
}

#[test]
fn rejects_a_physical_source_claimed_by_manifest_and_module() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "#name: \"app\"\n");
    let root_id = add_source(&mut sources, "/app/index.nct", "");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let package = declared_package("workspace:app", "app", "/same.nct", &manifest);
    let module = root_module(
        "workspace:app",
        vec![ModuleSourceInput::new(
            "/same.nct",
            ModuleSourceKind::Root,
            &root,
        )],
    );

    let error = lower_compile_unit_topology(&CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        &sources,
        vec![package],
        vec![module],
        Vec::new(),
    ))
    .unwrap_err();

    assert_eq!(
        error,
        LoweringError::DuplicateSourcePath("/same.nct".into())
    );
}

#[test]
fn single_file_package_has_one_root_module_without_a_manifest() {
    let mut sources = SourceMap::new();
    let source_id = add_source(
        &mut sources,
        "/tmp/example.nct",
        "func main(): void { return }\n",
    );
    let syntax = parse_source(&sources, source_id, ParseGoal::ModuleSource);
    let package = PackageInput::new(
        PackageIdentity::new("single:/tmp/example.nct"),
        "example",
        PackageMode::SingleFile,
        None,
    );
    let module = root_module(
        "single:/tmp/example.nct",
        vec![ModuleSourceInput::new(
            "/tmp/example.nct",
            ModuleSourceKind::SingleFile,
            &syntax,
        )],
    );

    let lowered = lower_compile_unit_topology(&CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        &sources,
        vec![package],
        vec![module],
        Vec::new(),
    ))
    .unwrap();

    assert_eq!(lowered.program().packages().len(), 1);
    assert_eq!(lowered.program().modules().len(), 1);
    assert_eq!(lowered.source_index().len(), 1);
}

#[test]
fn package_mode_cannot_be_smuggled_through_another_source_layout() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "#name: \"app\"\n");
    let source_id = add_source(&mut sources, "/app/index.nct", "");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let syntax = parse_source(&sources, source_id, ParseGoal::ModuleSource);
    let package = declared_package("workspace:app", "app", "/app/nocter.nct", &manifest);
    let module = root_module(
        "workspace:app",
        vec![ModuleSourceInput::new(
            "/app/index.nct",
            ModuleSourceKind::SingleFile,
            &syntax,
        )],
    );

    let error = lower_compile_unit_topology(&CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        &sources,
        vec![package],
        vec![module],
        Vec::new(),
    ))
    .unwrap_err();

    assert!(matches!(error, LoweringError::InvalidModuleLayout(_)));
}

#[test]
fn every_authored_use_requires_one_discovery_owned_resolution() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(&mut sources, "/app/index.nct", "use ./parser\n");
    let parser_id = add_source(&mut sources, "/app/parser/index.nct", "");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let parser = parse_source(&sources, parser_id, ParseGoal::ModuleSource);
    let package = declared_package("workspace:app", "app", "/app/nocter.nct", &manifest);
    let modules = vec![
        root_module(
            "workspace:app",
            vec![ModuleSourceInput::new(
                "/app/index.nct",
                ModuleSourceKind::Root,
                &root,
            )],
        ),
        child_module(
            "workspace:app",
            &["parser"],
            vec![ModuleSourceInput::new(
                "/app/parser/index.nct",
                ModuleSourceKind::Root,
                &parser,
            )],
        ),
    ];

    let error = lower_compile_unit_topology(&CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        &sources,
        vec![package],
        modules,
        Vec::new(),
    ))
    .unwrap_err();

    assert!(matches!(error, LoweringError::MissingUseResolution(_)));
}

#[test]
fn source_imports_must_be_private_bare_edges_within_one_module() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(&mut sources, "/app/index.nct", "use ./search.find\n");
    let search_id = add_source(&mut sources, "/app/search.nct", "func find(): void {}\n");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let search = parse_source(&sources, search_id, ParseGoal::ModuleSource);
    let package = declared_package("workspace:app", "app", "/app/nocter.nct", &manifest);
    let module = root_module(
        "workspace:app",
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new("/app/search.nct", ModuleSourceKind::Implementation, &search),
        ],
    );

    let error = lower_compile_unit_topology(&CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        &sources,
        vec![package],
        vec![module],
        vec![source_use(&root, 0, "/app/search.nct")],
    ))
    .unwrap_err();

    assert!(matches!(
        error,
        LoweringError::Rule(violation)
            if violation.rule() == crate::TopologyRule::InvalidSourceImport
    ));
}

#[test]
fn source_cycles_are_valid_but_every_implementation_must_be_root_reachable() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(&mut sources, "/app/index.nct", "use ./a\n");
    let a_id = add_source(&mut sources, "/app/a.nct", "use ./b\n");
    let b_id = add_source(&mut sources, "/app/b.nct", "use ./a\n");
    let orphan_id = add_source(&mut sources, "/app/orphan.nct", "func orphan(): void {}\n");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let a = parse_source(&sources, a_id, ParseGoal::ModuleSource);
    let b = parse_source(&sources, b_id, ParseGoal::ModuleSource);
    let orphan = parse_source(&sources, orphan_id, ParseGoal::ModuleSource);
    let package = declared_package("workspace:app", "app", "/app/nocter.nct", &manifest);
    let module = root_module(
        "workspace:app",
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new("/app/a.nct", ModuleSourceKind::Implementation, &a),
            ModuleSourceInput::new("/app/b.nct", ModuleSourceKind::Implementation, &b),
            ModuleSourceInput::new("/app/orphan.nct", ModuleSourceKind::Implementation, &orphan),
        ],
    );
    let resolutions = vec![
        source_use(&root, 0, "/app/a.nct"),
        source_use(&a, 0, "/app/b.nct"),
        source_use(&b, 0, "/app/a.nct"),
    ];

    let error = lower_compile_unit_topology(&CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        &sources,
        vec![package],
        vec![module],
        resolutions,
    ))
    .unwrap_err();

    assert_eq!(
        error,
        LoweringError::UnreachableImplementationSource("/app/orphan.nct".into())
    );
}

#[test]
fn resolved_module_graph_rejects_cycles_without_path_reinterpretation() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(&mut sources, "/app/index.nct", "use ./a\n");
    let a_id = add_source(&mut sources, "/app/a/index.nct", "use /b\n");
    let b_id = add_source(&mut sources, "/app/b/index.nct", "use /a\n");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let a = parse_source(&sources, a_id, ParseGoal::ModuleSource);
    let b = parse_source(&sources, b_id, ParseGoal::ModuleSource);
    let package_identity = PackageIdentity::new("workspace:app");
    let a_identity = ModuleIdentity::new(package_identity.clone(), ["a"]);
    let b_identity = ModuleIdentity::new(package_identity.clone(), ["b"]);
    let packages = vec![declared_package(
        "workspace:app",
        "app",
        "/app/nocter.nct",
        &manifest,
    )];
    let modules = vec![
        root_module(
            "workspace:app",
            vec![ModuleSourceInput::new(
                "/app/index.nct",
                ModuleSourceKind::Root,
                &root,
            )],
        ),
        child_module(
            "workspace:app",
            &["a"],
            vec![ModuleSourceInput::new(
                "/app/a/index.nct",
                ModuleSourceKind::Root,
                &a,
            )],
        ),
        child_module(
            "workspace:app",
            &["b"],
            vec![ModuleSourceInput::new(
                "/app/b/index.nct",
                ModuleSourceKind::Root,
                &b,
            )],
        ),
    ];
    let resolutions = vec![
        module_use(&root, 0, a_identity.clone()),
        module_use(&a, 0, b_identity),
        module_use(&b, 0, a_identity.clone()),
    ];

    let forward = lower_compile_unit_topology(&CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        &sources,
        packages.clone(),
        modules.clone(),
        resolutions.clone(),
    ))
    .unwrap_err();
    let reverse = lower_compile_unit_topology(&CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        &sources,
        packages.into_iter().rev().collect(),
        modules.into_iter().rev().collect(),
        resolutions.into_iter().rev().collect(),
    ))
    .unwrap_err();

    assert_eq!(forward, reverse);
    let LoweringError::Rule(violation) = forward else {
        panic!("module cycle did not select a topology rule");
    };
    assert_eq!(violation.rule(), crate::TopologyRule::ModuleImportCycle);
    assert!(matches!(
        violation.primary(),
        nocter_source_index::SyntaxOrigin::Node(node) if node.source() == a_id
    ));
    assert_eq!(violation.related().len(), 1);
    assert!(matches!(
        violation.related()[0],
        nocter_source_index::SyntaxOrigin::Node(node) if node.source() == b_id
    ));
}
