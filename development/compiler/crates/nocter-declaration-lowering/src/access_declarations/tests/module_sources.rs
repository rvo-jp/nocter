//! Compile-input evidence is required even when a body never refers to an authored dependency.
use super::*;
use crate::test_support::{module_use, source_see};
use crate::{LoweringError, SurfaceError, SurfaceSource};
use nocter_compile_input::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageInput, PackageMode,
};
use nocter_model::{CompilationTarget, PackageIdentity};

fn module<'syntax>(
    sources: Vec<ModuleSourceInput<'syntax>>,
    path: &[&str],
) -> ModuleInput<'syntax> {
    ModuleInput::new(
        ModuleIdentity::new(PackageIdentity::new("workspace:app"), path.iter().copied()),
        sources,
    )
}
fn package() -> PackageInput {
    PackageInput::new(
        PackageIdentity::new("workspace:app"),
        "app",
        PackageMode::Declared,
    )
}
fn tree(sources: &mut SourceMap, path: &str, text: &str) -> nocter_syntax::SyntaxTree {
    let id = sources
        .add_bytes(SourceName::new(path), text.as_bytes())
        .unwrap();
    let tree = nocter_syntax::parse(sources.get(id).unwrap(), ParseGoal::V2CoreUnit);
    assert!(!tree.has_errors(), "{text}: {:?}", tree.diagnostics());
    tree
}

#[test]
fn inventories_every_source_without_flattening_duplicate_private_names_or_resolving_types() {
    let mut sources = SourceMap::new();
    let root = tree(
        &mut sources,
        "/app/index.nct",
        "see ./a.nct\nfunc public(value: Missing): Missing\nstruct Opaque",
    );
    let a = tree(
        &mut sources,
        "/app/a.nct",
        "see ./index.nct\nfunc helper(): i32 { 1 }\nfunc public(value: Missing): Missing { value }",
    );
    let b = tree(&mut sources, "/app/b.nct", "func helper(): i32 { 2 }");
    let input = CompileUnitInput::for_goal(
        ParseGoal::V2CoreUnit,
        CompilationTarget::Arm64Darwin,
        &sources,
        vec![package()],
        vec![module(
            vec![
                ModuleSourceInput::new("/app/b.nct", ModuleSourceKind::Implementation, &b),
                ModuleSourceInput::new("/app/a.nct", ModuleSourceKind::Implementation, &a),
                ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ],
            &[],
        )],
        Vec::new(),
    )
    .with_source_visibility_resolutions(vec![
        source_see(&root, 0, "/app/a.nct"),
        source_see(&a, 0, "/app/index.nct"),
    ]);
    let inventory = ModuleSources::prepare(&input).unwrap();
    assert_eq!(
        inventory
            .sources()
            .iter()
            .map(SurfaceSource::canonical_path)
            .collect::<Vec<_>>(),
        ["/app/index.nct", "/app/a.nct", "/app/b.nct"]
    );
    assert_eq!(inventory.declarations().len(), 5);
    assert_eq!(inventory.source_visibilities().len(), 2);
    for declaration in inventory.declarations() {
        let owner = &inventory.sources()[declaration.source().index()];
        assert_eq!(owner.syntax().source(), declaration.node().source());
    }
    assert!(matches!(
        crate::collect_declaration_surface(&input),
        Err(SurfaceError::Topology(LoweringError::InvalidModuleSource(
            _
        )))
    ));
}

#[test]
fn dependencies_cannot_be_silently_dropped_by_the_single_source_semantic_query() {
    for dependency in [
        "see ./missing.nct",
        "use ./codec",
        "#target: \"arm64-darwin\"",
    ] {
        let (sources, tree) = input(
            &format!("{dependency}\nfunc run(): i32 {{ 42 }}"),
            ParseGoal::V2CoreUnit,
        );
        assert!(matches!(
            Query::prepare(bound(&sources, &tree)),
            Err(Failure::ModuleInputRequired)
        ));
    }
    let (sources, tree) = input("func run(): i32 { use ./codec\n42 }", ParseGoal::V2CoreUnit);
    assert!(matches!(
        Query::prepare(bound(&sources, &tree)),
        Err(Failure::ModuleInputRequired)
    ));
}

#[test]
fn source_roles_are_checked_for_unused_inventory_members() {
    let mut sources = SourceMap::new();
    let root = tree(&mut sources, "/app/index.nct", "func run(): i32");
    let unused = tree(&mut sources, "/app/unused.nct", "func omitted(): Missing");
    let input = CompileUnitInput::for_goal(
        ParseGoal::V2CoreUnit,
        CompilationTarget::Arm64Darwin,
        &sources,
        vec![package()],
        vec![module(
            vec![
                ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
                ModuleSourceInput::new(
                    "/app/unused.nct",
                    ModuleSourceKind::Implementation,
                    &unused,
                ),
            ],
            &[],
        )],
        Vec::new(),
    );
    assert!(
        matches!(ModuleSources::prepare(&input), Err(ModuleSourcesFailure::Access(Failure::SourceRole(diagnostic))) if diagnostic.rule() == SourceRoleRule::MissingBody)
    );
}

#[test]
fn exact_dependency_resolutions_preserve_module_boundaries_and_use_identity() {
    let mut sources = SourceMap::new();
    let root = tree(&mut sources, "/app/index.nct", "use ./codec");
    let codec = tree(&mut sources, "/app/codec/index.nct", "");
    let codec_identity = ModuleIdentity::new(PackageIdentity::new("workspace:app"), ["codec"]);
    let modules = vec![
        module(
            vec![ModuleSourceInput::new(
                "/app/index.nct",
                ModuleSourceKind::Root,
                &root,
            )],
            &[],
        ),
        module(
            vec![ModuleSourceInput::new(
                "/app/codec/index.nct",
                ModuleSourceKind::Root,
                &codec,
            )],
            &["codec"],
        ),
    ];
    let input = CompileUnitInput::for_goal(
        ParseGoal::V2CoreUnit,
        CompilationTarget::Arm64Darwin,
        &sources,
        vec![package()],
        modules,
        vec![module_use(&root, 0, codec_identity.clone())],
    );
    let inventory = ModuleSources::prepare(&input).unwrap();
    assert_eq!(inventory.imports().len(), 1);
    assert_eq!(inventory.imports()[0].target(), &codec_identity);
    let scoped = inventory.pair().unwrap().reserve_names().unwrap();
    let program = scoped.program();
    assert!(program.has_namespace_in(
        nocter_declarations::value_access::ScopeId::ROOT,
        program.symbols().get("codec").unwrap()
    ));
    assert_eq!(program.codes().len(), 0);
    assert_eq!(program.nominals().len(), 0);
    let see = tree(&mut sources, "/app/index.nct", "see ./codec/index.nct");
    let input = CompileUnitInput::for_goal(
        ParseGoal::V2CoreUnit,
        CompilationTarget::Arm64Darwin,
        &sources,
        vec![package()],
        vec![
            module(
                vec![ModuleSourceInput::new(
                    "/app/index.nct",
                    ModuleSourceKind::Root,
                    &see,
                )],
                &[],
            ),
            module(
                vec![ModuleSourceInput::new(
                    "/app/codec/index.nct",
                    ModuleSourceKind::Root,
                    &codec,
                )],
                &["codec"],
            ),
        ],
        Vec::new(),
    )
    .with_source_visibility_resolutions(vec![source_see(&see, 0, "/app/codec/index.nct")]);
    assert!(matches!(
        ModuleSources::prepare(&input),
        Err(ModuleSourcesFailure::Surface(SurfaceError::Topology(
            LoweringError::Rule(violation)
        ))) if violation.rule() == crate::TopologyRule::InvalidSourceVisibility
    ));
}

#[test]
fn physical_dependencies_need_closed_resolutions_and_released_sources_cannot_enter_v2() {
    let mut sources = SourceMap::new();
    let root = tree(&mut sources, "/app/index.nct", "see ./missing.nct");
    let input = CompileUnitInput::for_goal(
        ParseGoal::V2CoreUnit,
        CompilationTarget::Arm64Darwin,
        &sources,
        vec![package()],
        vec![module(
            vec![ModuleSourceInput::new(
                "/app/index.nct",
                ModuleSourceKind::Root,
                &root,
            )],
            &[],
        )],
        Vec::new(),
    );
    assert!(matches!(
        ModuleSources::prepare(&input),
        Err(ModuleSourcesFailure::Surface(SurfaceError::Topology(
            LoweringError::MissingSourceVisibilityResolution(_)
        )))
    ));
    let default_input = CompileUnitInput::new(
        CompilationTarget::Arm64Darwin,
        &sources,
        vec![package()],
        vec![module(
            vec![ModuleSourceInput::new(
                "/app/index.nct",
                ModuleSourceKind::Root,
                &root,
            )],
            &[],
        )],
        Vec::new(),
    );
    assert!(
        matches!(ModuleSources::prepare(&default_input), Err(ModuleSourcesFailure::Surface(SurfaceError::Topology(LoweringError::InconsistentSyntax(source)))) if source == root.source())
    );
    let released = nocter_syntax::parse(sources.get(root.source()).unwrap(), ParseGoal::SourceFile);
    let input = CompileUnitInput::new(
        CompilationTarget::Arm64Darwin,
        &sources,
        vec![package()],
        vec![module(
            vec![ModuleSourceInput::new(
                "/app/index.nct",
                ModuleSourceKind::Root,
                &released,
            )],
            &[],
        )],
        Vec::new(),
    );
    assert!(matches!(
        ModuleSources::prepare(&input),
        Err(ModuleSourcesFailure::Surface(SurfaceError::Topology(
            LoweringError::InvalidModuleSource(_)
        )))
    ));
}

#[test]
fn body_local_imports_bind_only_in_their_exact_lexical_scope() {
    let mut sources = SourceMap::new();
    let source = tree(
        &mut sources,
        "/app/main.nct",
        "func run(): i32 { use ./codec as scoped\nlet body_only = 42\n42 }",
    );
    let root = tree(&mut sources, "/app/index.nct", "");
    let codec = tree(&mut sources, "/app/codec/index.nct", "");
    let input = CompileUnitInput::for_goal(
        ParseGoal::V2CoreUnit,
        CompilationTarget::Arm64Darwin,
        &sources,
        vec![package()],
        vec![
            module(
                vec![
                    ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
                    ModuleSourceInput::new(
                        "/app/main.nct",
                        ModuleSourceKind::Implementation,
                        &source,
                    ),
                ],
                &[],
            ),
            module(
                vec![ModuleSourceInput::new(
                    "/app/codec/index.nct",
                    ModuleSourceKind::Root,
                    &codec,
                )],
                &["codec"],
            ),
        ],
        vec![module_use(
            &source,
            0,
            ModuleIdentity::new(PackageIdentity::new("workspace:app"), ["codec"]),
        )],
    );
    let inventory = ModuleSources::prepare(&input).unwrap();
    assert!(inventory.imports().is_empty());
    assert_eq!(inventory.block_imports().len(), 1);
    let node = inventory.block_imports()[0].node();
    let scoped = inventory.pair().unwrap().reserve_names().unwrap();
    let (physical, _) = scoped
        .paired()
        .inventory()
        .source_entries()
        .find(|(_, source)| source.canonical_path() == "/app/main.nct")
        .unwrap();
    let program = scoped.program();
    let source_scope = scoped.scope(physical).unwrap();
    let run = program
        .resolve_code_in(source_scope, program.symbols().get("run").unwrap())
        .unwrap();
    let binding = scoped.source_binding();
    let context = binding.context_for_code(&scoped, run).unwrap();
    let scope = context
        .lookup_scope_at(nocter_syntax::SyntaxOrigin::Node(node))
        .unwrap();
    assert_ne!(scope, source_scope);
    let codec = program.symbols().get("scoped").unwrap();
    assert!(program.has_namespace_in(scope, codec));
    assert!(!program.has_namespace_in(source_scope, codec));
    assert!(program.symbols().get("body_only").is_none());
    assert!(
        context
            .lookup_scope_at(nocter_syntax::SyntaxOrigin::Node(root.root_id()))
            .is_err()
    );
    let reparsed =
        nocter_syntax::parse(sources.get(source.source()).unwrap(), ParseGoal::V2CoreUnit);
    assert!(
        context
            .lookup_scope_at(nocter_syntax::SyntaxOrigin::Node(reparsed.root_id()))
            .is_err()
    );
}

fn assert_selected_module_order(order: &[ModuleIdentity]) {
    let identity = |path: &[&str]| {
        ModuleIdentity::new(PackageIdentity::new("workspace:app"), path.iter().copied())
    };
    assert_eq!(order.len(), 4);
    assert_eq!(
        order
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4
    );
    let position = |path: &[&str]| {
        order
            .iter()
            .position(|module| *module == identity(path))
            .unwrap()
    };
    assert!(position(&["b"]) < position(&["a"]));
    assert!(position(&["c"]) < position(&["a"]));
    assert!(position(&["a"]) < position(&[]));
}

#[test]
fn one_topology_order_includes_block_imports_and_excludes_inactive_items() {
    let mut sources = SourceMap::new();
    let root = tree(&mut sources, "/app/index.nct", "use ./a");
    let a = tree(&mut sources, "/app/a/index.nct", "use ../b");
    let a_body = tree(
        &mut sources,
        "/app/a/body.nct",
        "func unused(): void { if false { use ../c } }\n#target: \"x64-linux\"\nfunc ignored(): void { use ../missing as inactive_alias }",
    );
    let b = tree(&mut sources, "/app/b/index.nct", "");
    let c = tree(&mut sources, "/app/c/index.nct", "");
    let identity = |path: &[&str]| {
        ModuleIdentity::new(PackageIdentity::new("workspace:app"), path.iter().copied())
    };
    let modules = vec![
        module(
            vec![ModuleSourceInput::new(
                "/app/index.nct",
                ModuleSourceKind::Root,
                &root,
            )],
            &[],
        ),
        module(
            vec![
                ModuleSourceInput::new("/app/a/index.nct", ModuleSourceKind::Root, &a),
                ModuleSourceInput::new(
                    "/app/a/body.nct",
                    ModuleSourceKind::Implementation,
                    &a_body,
                ),
            ],
            &["a"],
        ),
        module(
            vec![ModuleSourceInput::new(
                "/app/b/index.nct",
                ModuleSourceKind::Root,
                &b,
            )],
            &["b"],
        ),
        module(
            vec![ModuleSourceInput::new(
                "/app/c/index.nct",
                ModuleSourceKind::Root,
                &c,
            )],
            &["c"],
        ),
    ];
    let resolutions = vec![
        module_use(&root, 0, identity(&["a"])),
        module_use(&a, 0, identity(&["b"])),
        module_use(&a_body, 0, identity(&["c"])),
    ];
    let mut expected = None;
    for reverse in [false, true] {
        let mut modules = modules.clone();
        let mut resolutions = resolutions.clone();
        if reverse {
            modules.reverse();
            resolutions.reverse();
        }
        let input = CompileUnitInput::for_goal(
            ParseGoal::V2CoreUnit,
            CompilationTarget::Arm64Darwin,
            &sources,
            vec![package()],
            modules,
            resolutions,
        );
        let inventory = ModuleSources::prepare(&input).unwrap();
        let order = inventory
            .modules_in_dependency_order()
            .cloned()
            .collect::<Vec<_>>();
        assert_selected_module_order(&order);
        assert_eq!(inventory.block_imports().len(), 1);
        assert!(inventory.surface.symbols().get("inactive_alias").is_none());
        if let Some(expected) = &expected {
            assert_eq!(&order, expected);
        } else {
            expected = Some(order);
        }
    }
}

#[test]
fn unreachable_block_imports_participate_in_the_same_module_cycle_diagnostic() {
    let mut sources = SourceMap::new();
    let root = tree(&mut sources, "/app/index.nct", "use ./a");
    let a = tree(&mut sources, "/app/a/index.nct", "");
    let body = tree(
        &mut sources,
        "/app/a/body.nct",
        "func unused(): void { if false { use / as app } }",
    );
    let input = CompileUnitInput::for_goal(
        ParseGoal::V2CoreUnit,
        CompilationTarget::Arm64Darwin,
        &sources,
        vec![package()],
        vec![
            module(
                vec![ModuleSourceInput::new(
                    "/app/index.nct",
                    ModuleSourceKind::Root,
                    &root,
                )],
                &[],
            ),
            module(
                vec![
                    ModuleSourceInput::new("/app/a/index.nct", ModuleSourceKind::Root, &a),
                    ModuleSourceInput::new(
                        "/app/a/body.nct",
                        ModuleSourceKind::Implementation,
                        &body,
                    ),
                ],
                &["a"],
            ),
        ],
        vec![
            module_use(
                &root,
                0,
                ModuleIdentity::new(PackageIdentity::new("workspace:app"), ["a"]),
            ),
            module_use(
                &body,
                0,
                ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new()),
            ),
        ],
    );
    assert!(
        matches!(ModuleSources::prepare(&input), Err(ModuleSourcesFailure::Surface(SurfaceError::Topology(LoweringError::Rule(violation)))) if violation.rule() == crate::TopologyRule::ModuleImportCycle)
    );
}
