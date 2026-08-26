use nocter_declarations::ExportedEntity;
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::{ImportError, PreparedImports, apply_toolchain_profile, prepare_authored_imports};
use crate::test_support::{module_use, source_see};
use crate::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageIdentity, PackageInput, PackageMode, ToolchainError, ToolchainInput, UseResolutionInput,
    collect_declaration_surface, prepare_declaration_headers, prepare_generic_binders,
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

fn package(
    identity: &str,
    display_name: &str,
    _path: &str,
    _manifest: &SyntaxTree,
) -> PackageInput {
    PackageInput::new(
        PackageIdentity::new(identity),
        display_name,
        PackageMode::Declared,
    )
}

fn module<'syntax>(
    package: &str,
    path: &[&str],
    sources: Vec<ModuleSourceInput<'syntax>>,
) -> ModuleInput<'syntax> {
    ModuleInput::new(
        ModuleIdentity::new(PackageIdentity::new(package), path.iter().copied()),
        sources,
    )
}

fn root_source<'syntax>(path: &str, syntax: &'syntax SyntaxTree) -> ModuleSourceInput<'syntax> {
    ModuleSourceInput::new(path, ModuleSourceKind::Root, syntax)
}

fn toolchain(prelude: ModuleIdentity) -> ToolchainInput {
    ToolchainInput::new(prelude.package().clone(), prelude, Vec::new(), Vec::new())
}

fn prepare<'syntax>(
    sources: &'syntax SourceMap,
    packages: Vec<PackageInput>,
    modules: Vec<ModuleInput<'syntax>>,
    uses: Vec<UseResolutionInput>,
) -> Result<PreparedImports<'syntax>, ImportError> {
    let profile = modules
        .iter()
        .find(|module| {
            module
                .identity()
                .path()
                .iter()
                .map(AsRef::as_ref)
                .eq(["prelude"])
        })
        .or_else(|| modules.first())
        .map(|module| toolchain(module.identity().clone()))
        .expect("import fixture has no module");
    let input = CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        sources,
        packages,
        modules,
        uses,
    )
    .with_toolchain(profile.clone());
    let surface = collect_declaration_surface(&input).unwrap();
    let reserved = reserve_declaration_identities(surface, &profile).unwrap();
    let headers = prepare_declaration_headers(reserved).unwrap();
    let generics = prepare_generic_binders(headers).unwrap();
    prepare_authored_imports(generics)
}

fn prepare_with_source_visibility<'syntax>(
    sources: &'syntax SourceMap,
    packages: Vec<PackageInput>,
    modules: Vec<ModuleInput<'syntax>>,
    source_visibilities: Vec<crate::SourceVisibilityResolutionInput>,
) -> Result<PreparedImports<'syntax>, ImportError> {
    let profile = modules
        .iter()
        .find(|module| {
            module
                .identity()
                .path()
                .iter()
                .map(AsRef::as_ref)
                .eq(["prelude"])
        })
        .or_else(|| modules.first())
        .map(|module| toolchain(module.identity().clone()))
        .expect("import fixture has no module");
    let input = CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        sources,
        packages,
        modules,
        Vec::new(),
    )
    .with_source_visibility_resolutions(source_visibilities)
    .with_toolchain(profile.clone());
    let surface = collect_declaration_surface(&input).unwrap();
    let reserved = reserve_declaration_identities(surface, &profile).unwrap();
    let headers = prepare_declaration_headers(reserved).unwrap();
    let generics = prepare_generic_binders(headers).unwrap();
    prepare_authored_imports(generics)
}

fn module_id(imports: &PreparedImports<'_>, identity: &ModuleIdentity) -> nocter_model::ModuleId {
    let reserved = imports.generics().headers().reserved();
    let index = reserved
        .modules()
        .iter()
        .position(|candidate| candidate == identity)
        .unwrap();
    reserved.module_ids()[index]
}

fn source_id(imports: &PreparedImports<'_>, path: &str) -> crate::SurfaceSourceId {
    imports
        .generics()
        .headers()
        .reserved()
        .sources()
        .iter()
        .position(|source| source.canonical_path() == path)
        .map(crate::SurfaceSourceId::from_index)
        .unwrap()
}

#[test]
fn resolves_selected_aliases_and_namespace_imports_without_exposing_private_names() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let dep_manifest_id = add_source(&mut sources, "/dep/index.nct", "");
    let app_id = add_source(
        &mut sources,
        "/app/index.nct",
        "use dep.Value as Item\nuse dep\n\nfunc consume(value: Item): void {}\n",
    );
    let dep_id = add_source(
        &mut sources,
        "/dep/index.nct",
        "pub struct Value {}\npub func make(): Value {}\nfunc hidden(): void {}\n",
    );
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
    let dep_manifest = parse_source(&sources, dep_manifest_id, ParseGoal::SourceFile);
    let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
    let dep = parse_source(&sources, dep_id, ParseGoal::SourceFile);
    let app_identity =
        ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new());
    let dep_identity =
        ModuleIdentity::new(PackageIdentity::new("resolved:dep"), Vec::<&str>::new());

    let imports = prepare(
        &sources,
        vec![
            package("workspace:app", "app", "/app/index.nct", &app_manifest),
            package("resolved:dep", "dep", "/dep/index.nct", &dep_manifest),
        ],
        vec![
            module(
                "workspace:app",
                &[],
                vec![ModuleSourceInput::new(
                    "/app/index.nct",
                    ModuleSourceKind::Root,
                    &app,
                )],
            ),
            module(
                "resolved:dep",
                &[],
                vec![ModuleSourceInput::new(
                    "/dep/index.nct",
                    ModuleSourceKind::Root,
                    &dep,
                )],
            ),
        ],
        vec![
            module_use(&app, 0, dep_identity.clone()),
            module_use(&app, 1, dep_identity.clone()),
        ],
    )
    .unwrap();
    let app_module = module_id(&imports, &app_identity);
    let app_source = source_id(&imports, "/app/index.nct");
    let dep_module = module_id(&imports, &dep_identity);
    let symbols = imports.generics().headers().reserved().symbols();

    assert!(matches!(
        imports.lookup_local(app_source, symbols.get("Item").unwrap()),
        Some(ExportedEntity::NominalType(_))
    ));
    assert_eq!(
        imports.lookup_local(app_source, symbols.get("dep").unwrap()),
        Some(ExportedEntity::Module(dep_module))
    );
    assert!(matches!(
        imports.lookup_export(app_module, dep_module, symbols.get("Value").unwrap()),
        Some(ExportedEntity::NominalType(_))
    ));
    assert_eq!(
        imports.lookup_export(app_module, dep_module, symbols.get("hidden").unwrap()),
        None
    );
    assert!(imports.import_id(0).is_some());
    assert!(imports.import_id(1).is_some());
}

#[test]
fn imported_names_cannot_collide_with_module_declarations() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let dep_manifest_id = add_source(&mut sources, "/dep/index.nct", "");
    let app_id = add_source(
        &mut sources,
        "/app/index.nct",
        "use dep.Value as Item\n\nstruct Item {}\n",
    );
    let dep_id = add_source(&mut sources, "/dep/index.nct", "pub struct Value {}\n");
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
    let dep_manifest = parse_source(&sources, dep_manifest_id, ParseGoal::SourceFile);
    let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
    let dep = parse_source(&sources, dep_id, ParseGoal::SourceFile);
    let dep_identity =
        ModuleIdentity::new(PackageIdentity::new("resolved:dep"), Vec::<&str>::new());

    let error = prepare(
        &sources,
        vec![
            package("workspace:app", "app", "/app/index.nct", &app_manifest),
            package("resolved:dep", "dep", "/dep/index.nct", &dep_manifest),
        ],
        vec![
            module(
                "workspace:app",
                &[],
                vec![ModuleSourceInput::new(
                    "/app/index.nct",
                    ModuleSourceKind::Root,
                    &app,
                )],
            ),
            module(
                "resolved:dep",
                &[],
                vec![ModuleSourceInput::new(
                    "/dep/index.nct",
                    ModuleSourceKind::Root,
                    &dep,
                )],
            ),
        ],
        vec![module_use(&app, 0, dep_identity)],
    )
    .unwrap_err();

    assert!(matches!(
        error,
        ImportError::Namespace(violation)
            if violation.rule() == crate::NamespaceRule::NameCollision
    ));
}

#[test]
fn selected_imports_reject_private_targets() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let dep_manifest_id = add_source(&mut sources, "/dep/index.nct", "");
    let app_id = add_source(&mut sources, "/app/index.nct", "use dep.Hidden\n");
    let dep_id = add_source(&mut sources, "/dep/index.nct", "struct Hidden {}\n");
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
    let dep_manifest = parse_source(&sources, dep_manifest_id, ParseGoal::SourceFile);
    let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
    let dep = parse_source(&sources, dep_id, ParseGoal::SourceFile);
    let dep_identity =
        ModuleIdentity::new(PackageIdentity::new("resolved:dep"), Vec::<&str>::new());

    let error = prepare(
        &sources,
        vec![
            package("workspace:app", "app", "/app/index.nct", &app_manifest),
            package("resolved:dep", "dep", "/dep/index.nct", &dep_manifest),
        ],
        vec![
            module(
                "workspace:app",
                &[],
                vec![ModuleSourceInput::new(
                    "/app/index.nct",
                    ModuleSourceKind::Root,
                    &app,
                )],
            ),
            module(
                "resolved:dep",
                &[],
                vec![ModuleSourceInput::new(
                    "/dep/index.nct",
                    ModuleSourceKind::Root,
                    &dep,
                )],
            ),
        ],
        vec![module_use(&app, 0, dep_identity)],
    )
    .unwrap_err();

    assert!(matches!(
        error,
        ImportError::Rule(violation)
            if violation.rule() == crate::ImportRule::InaccessibleImportedName
    ));
}

#[test]
fn chained_reexports_cannot_widen_a_descendant_boundary() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let root_id = add_source(&mut sources, "/app/index.nct", "use ./core/facade\n");
    let core_id = add_source(
        &mut sources,
        "/app/core/index.nct",
        "pub(./) struct Internal {}\n",
    );
    let facade_id = add_source(
        &mut sources,
        "/app/core/facade/index.nct",
        "pub use /core.Internal\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let core = parse_source(&sources, core_id, ParseGoal::SourceFile);
    let facade = parse_source(&sources, facade_id, ParseGoal::SourceFile);
    let package_identity = PackageIdentity::new("workspace:app");
    let core_identity = ModuleIdentity::new(package_identity.clone(), ["core"]);
    let facade_identity = ModuleIdentity::new(package_identity.clone(), ["core", "facade"]);

    let error = prepare(
        &sources,
        vec![package("workspace:app", "app", "/app/index.nct", &manifest)],
        vec![
            module(
                "workspace:app",
                &[],
                vec![ModuleSourceInput::new(
                    "/app/index.nct",
                    ModuleSourceKind::Root,
                    &root,
                )],
            ),
            module(
                "workspace:app",
                &["core"],
                vec![ModuleSourceInput::new(
                    "/app/core/index.nct",
                    ModuleSourceKind::Root,
                    &core,
                )],
            ),
            module(
                "workspace:app",
                &["core", "facade"],
                vec![ModuleSourceInput::new(
                    "/app/core/facade/index.nct",
                    ModuleSourceKind::Root,
                    &facade,
                )],
            ),
        ],
        vec![
            module_use(&root, 0, facade_identity),
            module_use(&facade, 0, core_identity),
        ],
    )
    .unwrap_err();

    let ImportError::Rule(violation) = error else {
        panic!("widening re-export did not select an import rule");
    };
    assert_eq!(violation.rule(), crate::ImportRule::WideningReexport);
    assert!(matches!(
        violation.primary(),
        nocter_syntax::SyntaxOrigin::Node(node) if node.source() == facade_id
    ));
    assert!(matches!(
        violation.related(),
        Some(nocter_syntax::SyntaxOrigin::Token(token)) if token.source() == core_id
    ));
}

#[test]
fn selected_reexports_resolve_in_dependency_order() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let dep_manifest_id = add_source(&mut sources, "/dep/index.nct", "");
    let root_id = add_source(&mut sources, "/app/index.nct", "use ./facade.Item\n");
    let facade_id = add_source(
        &mut sources,
        "/app/facade/index.nct",
        "pub use dep.Value as Item\n",
    );
    let dep_id = add_source(&mut sources, "/dep/index.nct", "pub struct Value {}\n");
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
    let dep_manifest = parse_source(&sources, dep_manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let facade = parse_source(&sources, facade_id, ParseGoal::SourceFile);
    let dep = parse_source(&sources, dep_id, ParseGoal::SourceFile);
    let facade_identity = ModuleIdentity::new(PackageIdentity::new("workspace:app"), ["facade"]);
    let dep_identity =
        ModuleIdentity::new(PackageIdentity::new("resolved:dep"), Vec::<&str>::new());

    let imports = prepare(
        &sources,
        vec![
            package("workspace:app", "app", "/app/index.nct", &app_manifest),
            package("resolved:dep", "dep", "/dep/index.nct", &dep_manifest),
        ],
        vec![
            module(
                "workspace:app",
                &[],
                vec![ModuleSourceInput::new(
                    "/app/index.nct",
                    ModuleSourceKind::Root,
                    &root,
                )],
            ),
            module(
                "workspace:app",
                &["facade"],
                vec![ModuleSourceInput::new(
                    "/app/facade/index.nct",
                    ModuleSourceKind::Root,
                    &facade,
                )],
            ),
            module(
                "resolved:dep",
                &[],
                vec![ModuleSourceInput::new(
                    "/dep/index.nct",
                    ModuleSourceKind::Root,
                    &dep,
                )],
            ),
        ],
        vec![
            module_use(&root, 0, facade_identity),
            module_use(&facade, 0, dep_identity),
        ],
    )
    .unwrap();
    let root_source = source_id(&imports, "/app/index.nct");
    let item = imports
        .generics()
        .headers()
        .reserved()
        .symbols()
        .get("Item")
        .unwrap();

    assert!(matches!(
        imports.lookup_local(root_source, item),
        Some(ExportedEntity::NominalType(_))
    ));
}

#[test]
fn direct_source_sees_add_no_import_and_do_not_publish_implementation_names() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let root_id = add_source(&mut sources, "/app/index.nct", "see ./implementation.nct\n");
    let implementation_id = add_source(
        &mut sources,
        "/app/implementation.nct",
        "func helper(): void {}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let identity = ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new());

    let imports = prepare_with_source_visibility(
        &sources,
        vec![package("workspace:app", "app", "/app/index.nct", &manifest)],
        vec![module(
            "workspace:app",
            &[],
            vec![
                ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
                ModuleSourceInput::new(
                    "/app/implementation.nct",
                    ModuleSourceKind::Implementation,
                    &implementation,
                ),
            ],
        )],
        vec![source_see(&root, 0, "/app/implementation.nct")],
    )
    .unwrap();
    let module = module_id(&imports, &identity);
    let root_source = source_id(&imports, "/app/index.nct");
    let helper = imports
        .generics()
        .headers()
        .reserved()
        .symbols()
        .get("helper")
        .unwrap();

    assert!(matches!(
        imports.lookup_local(root_source, helper),
        Some(ExportedEntity::Callable(_))
    ));
    assert_eq!(imports.lookup_export(module, module, helper), None);
    assert_eq!(imports.import_id(0), None);
}

#[test]
fn source_sees_expose_only_the_direct_target_namespace() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let root_id = add_source(&mut sources, "/app/index.nct", "see ./a.nct\n");
    let a_id = add_source(
        &mut sources,
        "/app/a.nct",
        "see ./b.nct\nfunc from_a(): void {}\n",
    );
    let b_id = add_source(&mut sources, "/app/b.nct", "func from_b(): void {}\n");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let a = parse_source(&sources, a_id, ParseGoal::SourceFile);
    let b = parse_source(&sources, b_id, ParseGoal::SourceFile);

    let imports = prepare_with_source_visibility(
        &sources,
        vec![package("workspace:app", "app", "/app/index.nct", &manifest)],
        vec![module(
            "workspace:app",
            &[],
            vec![
                root_source("/app/index.nct", &root),
                ModuleSourceInput::new("/app/a.nct", ModuleSourceKind::Implementation, &a),
                ModuleSourceInput::new("/app/b.nct", ModuleSourceKind::Implementation, &b),
            ],
        )],
        vec![
            source_see(&root, 0, "/app/a.nct"),
            source_see(&a, 0, "/app/b.nct"),
        ],
    )
    .unwrap();
    let root_source = source_id(&imports, "/app/index.nct");
    let a_source = source_id(&imports, "/app/a.nct");
    let symbols = imports.generics().headers().reserved().symbols();
    let from_a = symbols.get("from_a").unwrap();
    let from_b = symbols.get("from_b").unwrap();

    assert!(imports.lookup_local(root_source, from_a).is_some());
    assert_eq!(imports.lookup_local(root_source, from_b), None);
    assert!(imports.lookup_local(a_source, from_b).is_some());
}

#[test]
fn standard_prelude_is_a_shadowable_fallback_and_not_an_implicit_reexport() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
    let app_root_id = add_source(&mut sources, "/app/index.nct", "struct String {}\n");
    let app_child_id = add_source(&mut sources, "/app/child/index.nct", "");
    let std_root_id = add_source(&mut sources, "/std/index.nct", "");
    let std_string_id = add_source(
        &mut sources,
        "/std/string/index.nct",
        "pub struct String {}\n",
    );
    let std_prelude_id = add_source(
        &mut sources,
        "/std/prelude/index.nct",
        "pub use /string.String\n",
    );
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::SourceFile);
    let app_root = parse_source(&sources, app_root_id, ParseGoal::SourceFile);
    let app_child = parse_source(&sources, app_child_id, ParseGoal::SourceFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::SourceFile);
    let std_string = parse_source(&sources, std_string_id, ParseGoal::SourceFile);
    let std_prelude = parse_source(&sources, std_prelude_id, ParseGoal::SourceFile);
    let app_package = PackageIdentity::new("workspace:app");
    let std_package = PackageIdentity::new("builtin:std");
    let app_root_identity = ModuleIdentity::new(app_package.clone(), Vec::<&str>::new());
    let app_child_identity = ModuleIdentity::new(app_package, ["child"]);
    let std_string_identity = ModuleIdentity::new(std_package.clone(), ["string"]);

    let imports = prepare(
        &sources,
        vec![
            package("workspace:app", "app", "/app/index.nct", &app_manifest),
            package("builtin:std", "std", "/std/index.nct", &std_manifest),
        ],
        vec![
            module(
                "workspace:app",
                &[],
                vec![root_source("/app/index.nct", &app_root)],
            ),
            module(
                "workspace:app",
                &["child"],
                vec![root_source("/app/child/index.nct", &app_child)],
            ),
            module(
                "builtin:std",
                &[],
                vec![root_source("/std/index.nct", &std_root)],
            ),
            module(
                "builtin:std",
                &["string"],
                vec![root_source("/std/string/index.nct", &std_string)],
            ),
            module(
                "builtin:std",
                &["prelude"],
                vec![root_source("/std/prelude/index.nct", &std_prelude)],
            ),
        ],
        vec![module_use(&std_prelude, 0, std_string_identity.clone())],
    )
    .unwrap();
    let app_root_module = module_id(&imports, &app_root_identity);
    let app_child_module = module_id(&imports, &app_child_identity);
    let app_root_source = source_id(&imports, "/app/index.nct");
    let app_child_source = source_id(&imports, "/app/child/index.nct");
    let std_root_source = source_id(&imports, "/std/index.nct");
    let std_string_source = source_id(&imports, "/std/string/index.nct");
    let string = imports
        .generics()
        .headers()
        .reserved()
        .symbols()
        .get("String")
        .unwrap();
    let app_string = imports.lookup_local(app_root_source, string).unwrap();
    let standard_string = imports.lookup_local(std_string_source, string).unwrap();

    let namespaces = apply_toolchain_profile(imports).unwrap();

    assert_eq!(
        namespaces.lookup_local(app_root_source, string),
        Some(app_string)
    );
    assert_eq!(
        namespaces.lookup_local(app_child_source, string),
        Some(standard_string)
    );
    assert_eq!(namespaces.lookup_local(std_root_source, string), None);
    assert_eq!(
        namespaces.lookup_export(app_root_module, app_child_module, string),
        None
    );
}

#[test]
fn source_code_cannot_import_the_compiler_managed_prelude() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
    let app_id = add_source(&mut sources, "/app/index.nct", "use std/prelude.String\n");
    let std_root_id = add_source(&mut sources, "/std/index.nct", "");
    let prelude_id = add_source(
        &mut sources,
        "/std/prelude/index.nct",
        "pub struct String {}\n",
    );
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::SourceFile);
    let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::SourceFile);
    let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
    let prelude_identity = ModuleIdentity::new(PackageIdentity::new("builtin:std"), ["prelude"]);
    let imports = prepare(
        &sources,
        vec![
            package("workspace:app", "app", "/app/index.nct", &app_manifest),
            package("builtin:std", "std", "/std/index.nct", &std_manifest),
        ],
        vec![
            module(
                "workspace:app",
                &[],
                vec![ModuleSourceInput::new(
                    "/app/index.nct",
                    ModuleSourceKind::Root,
                    &app,
                )],
            ),
            module(
                "builtin:std",
                &[],
                vec![ModuleSourceInput::new(
                    "/std/index.nct",
                    ModuleSourceKind::Root,
                    &std_root,
                )],
            ),
            module(
                "builtin:std",
                &["prelude"],
                vec![ModuleSourceInput::new(
                    "/std/prelude/index.nct",
                    ModuleSourceKind::Root,
                    &prelude,
                )],
            ),
        ],
        vec![module_use(&app, 0, prelude_identity.clone())],
    )
    .unwrap();

    let error = apply_toolchain_profile(imports).unwrap_err();

    assert!(matches!(
        error,
        ToolchainError::Rule(violation)
            if violation.rule() == crate::ImportRule::CompilerManagedPreludeImport
    ));
}
