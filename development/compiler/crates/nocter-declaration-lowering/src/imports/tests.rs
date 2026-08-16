use nocter_declarations::ExportedEntity;
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::{
    ImportError, PreludeError, PreparedImports, apply_standard_prelude, prepare_authored_imports,
};
use crate::test_support::{module_use, source_use};
use crate::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode, UseResolutionInput,
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

fn package<'syntax>(
    identity: &str,
    display_name: &str,
    path: &str,
    manifest: &'syntax SyntaxTree,
) -> PackageInput<'syntax> {
    PackageInput::new(
        PackageIdentity::new(identity),
        display_name,
        PackageMode::Declared,
        Some(PackageDeclarationInput::new(path, manifest)),
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

fn prepare<'syntax>(
    sources: &'syntax SourceMap,
    packages: Vec<PackageInput<'syntax>>,
    modules: Vec<ModuleInput<'syntax>>,
    uses: Vec<UseResolutionInput>,
) -> Result<PreparedImports<'syntax>, ImportError> {
    let input = CompileUnitInput::new(sources, packages, modules, uses);
    let surface = collect_declaration_surface(&input).unwrap();
    let reserved = reserve_declaration_identities(surface).unwrap();
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

#[test]
fn resolves_selected_aliases_and_namespace_imports_without_exposing_private_names() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let dep_manifest_id = add_source(&mut sources, "/dep/nocter.nct", "");
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
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let dep_manifest = parse_source(&sources, dep_manifest_id, ParseGoal::PackageFile);
    let app = parse_source(&sources, app_id, ParseGoal::ModuleSource);
    let dep = parse_source(&sources, dep_id, ParseGoal::ModuleSource);
    let app_identity =
        ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new());
    let dep_identity =
        ModuleIdentity::new(PackageIdentity::new("resolved:dep"), Vec::<&str>::new());

    let imports = prepare(
        &sources,
        vec![
            package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
            package("resolved:dep", "dep", "/dep/nocter.nct", &dep_manifest),
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
    let dep_module = module_id(&imports, &dep_identity);
    let symbols = imports.generics().headers().reserved().symbols();

    assert!(matches!(
        imports.lookup_local(app_module, symbols.get("Item").unwrap()),
        Some(ExportedEntity::NominalType(_))
    ));
    assert_eq!(
        imports.lookup_local(app_module, symbols.get("dep").unwrap()),
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
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let dep_manifest_id = add_source(&mut sources, "/dep/nocter.nct", "");
    let app_id = add_source(
        &mut sources,
        "/app/index.nct",
        "use dep.Value as Item\n\nstruct Item {}\n",
    );
    let dep_id = add_source(&mut sources, "/dep/index.nct", "pub struct Value {}\n");
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let dep_manifest = parse_source(&sources, dep_manifest_id, ParseGoal::PackageFile);
    let app = parse_source(&sources, app_id, ParseGoal::ModuleSource);
    let dep = parse_source(&sources, dep_id, ParseGoal::ModuleSource);
    let dep_identity =
        ModuleIdentity::new(PackageIdentity::new("resolved:dep"), Vec::<&str>::new());

    let error = prepare(
        &sources,
        vec![
            package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
            package("resolved:dep", "dep", "/dep/nocter.nct", &dep_manifest),
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

    assert!(matches!(error, ImportError::DuplicateName { .. }));
}

#[test]
fn selected_imports_reject_private_targets() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let dep_manifest_id = add_source(&mut sources, "/dep/nocter.nct", "");
    let app_id = add_source(&mut sources, "/app/index.nct", "use dep.Hidden\n");
    let dep_id = add_source(&mut sources, "/dep/index.nct", "struct Hidden {}\n");
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let dep_manifest = parse_source(&sources, dep_manifest_id, ParseGoal::PackageFile);
    let app = parse_source(&sources, app_id, ParseGoal::ModuleSource);
    let dep = parse_source(&sources, dep_id, ParseGoal::ModuleSource);
    let dep_identity =
        ModuleIdentity::new(PackageIdentity::new("resolved:dep"), Vec::<&str>::new());

    let error = prepare(
        &sources,
        vec![
            package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
            package("resolved:dep", "dep", "/dep/nocter.nct", &dep_manifest),
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

    assert!(matches!(error, ImportError::InaccessibleImportedName(_)));
}

#[test]
fn chained_reexports_cannot_widen_a_descendant_boundary() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
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
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let core = parse_source(&sources, core_id, ParseGoal::ModuleSource);
    let facade = parse_source(&sources, facade_id, ParseGoal::ModuleSource);
    let package_identity = PackageIdentity::new("workspace:app");
    let core_identity = ModuleIdentity::new(package_identity.clone(), ["core"]);
    let facade_identity = ModuleIdentity::new(package_identity.clone(), ["core", "facade"]);

    let error = prepare(
        &sources,
        vec![package(
            "workspace:app",
            "app",
            "/app/nocter.nct",
            &manifest,
        )],
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

    assert!(matches!(error, ImportError::WideningReexport(_)));
}

#[test]
fn selected_reexports_resolve_in_dependency_order() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let dep_manifest_id = add_source(&mut sources, "/dep/nocter.nct", "");
    let root_id = add_source(&mut sources, "/app/index.nct", "use ./facade.Item\n");
    let facade_id = add_source(
        &mut sources,
        "/app/facade/index.nct",
        "pub use dep.Value as Item\n",
    );
    let dep_id = add_source(&mut sources, "/dep/index.nct", "pub struct Value {}\n");
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let dep_manifest = parse_source(&sources, dep_manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let facade = parse_source(&sources, facade_id, ParseGoal::ModuleSource);
    let dep = parse_source(&sources, dep_id, ParseGoal::ModuleSource);
    let app_identity =
        ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new());
    let facade_identity = ModuleIdentity::new(PackageIdentity::new("workspace:app"), ["facade"]);
    let dep_identity =
        ModuleIdentity::new(PackageIdentity::new("resolved:dep"), Vec::<&str>::new());

    let imports = prepare(
        &sources,
        vec![
            package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
            package("resolved:dep", "dep", "/dep/nocter.nct", &dep_manifest),
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
    let root_module = module_id(&imports, &app_identity);
    let item = imports
        .generics()
        .headers()
        .reserved()
        .symbols()
        .get("Item")
        .unwrap();

    assert!(matches!(
        imports.lookup_local(root_module, item),
        Some(ExportedEntity::NominalType(_))
    ));
}

#[test]
fn source_imports_add_no_semantic_import_but_share_the_module_namespace() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(&mut sources, "/app/index.nct", "use ./implementation\n");
    let implementation_id = add_source(
        &mut sources,
        "/app/implementation.nct",
        "func helper(): void {}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::ModuleSource);
    let identity = ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new());

    let imports = prepare(
        &sources,
        vec![package(
            "workspace:app",
            "app",
            "/app/nocter.nct",
            &manifest,
        )],
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
        vec![source_use(&root, 0, "/app/implementation.nct")],
    )
    .unwrap();
    let module = module_id(&imports, &identity);
    let helper = imports
        .generics()
        .headers()
        .reserved()
        .symbols()
        .get("helper")
        .unwrap();

    assert!(matches!(
        imports.lookup_local(module, helper),
        Some(ExportedEntity::Callable(_))
    ));
    assert_eq!(imports.import_id(0), None);
}

#[test]
fn standard_prelude_is_a_shadowable_fallback_and_not_an_implicit_reexport() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
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
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::PackageFile);
    let app_root = parse_source(&sources, app_root_id, ParseGoal::ModuleSource);
    let app_child = parse_source(&sources, app_child_id, ParseGoal::ModuleSource);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::ModuleSource);
    let std_string = parse_source(&sources, std_string_id, ParseGoal::ModuleSource);
    let std_prelude = parse_source(&sources, std_prelude_id, ParseGoal::ModuleSource);
    let app_package = PackageIdentity::new("workspace:app");
    let std_package = PackageIdentity::new("builtin:std");
    let app_root_identity = ModuleIdentity::new(app_package.clone(), Vec::<&str>::new());
    let app_child_identity = ModuleIdentity::new(app_package, ["child"]);
    let std_root_identity = ModuleIdentity::new(std_package.clone(), Vec::<&str>::new());
    let std_string_identity = ModuleIdentity::new(std_package.clone(), ["string"]);
    let std_prelude_identity = ModuleIdentity::new(std_package, ["prelude"]);

    let imports = prepare(
        &sources,
        vec![
            package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
            package("builtin:std", "std", "/std/nocter.nct", &std_manifest),
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
    let std_root_module = module_id(&imports, &std_root_identity);
    let std_string_module = module_id(&imports, &std_string_identity);
    let string = imports
        .generics()
        .headers()
        .reserved()
        .symbols()
        .get("String")
        .unwrap();
    let app_string = imports.lookup_local(app_root_module, string).unwrap();
    let standard_string = imports.lookup_local(std_string_module, string).unwrap();

    let namespaces = apply_standard_prelude(imports, &std_prelude_identity).unwrap();

    assert_eq!(
        namespaces.lookup_local(app_root_module, string),
        Some(app_string)
    );
    assert_eq!(
        namespaces.lookup_local(app_child_module, string),
        Some(standard_string)
    );
    assert_eq!(namespaces.lookup_local(std_root_module, string), None);
    assert_eq!(
        namespaces.lookup_export(app_root_module, app_child_module, string),
        None
    );
}

#[test]
fn source_code_cannot_import_the_compiler_managed_prelude() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
    let app_id = add_source(&mut sources, "/app/index.nct", "use std/prelude.String\n");
    let std_root_id = add_source(&mut sources, "/std/index.nct", "");
    let prelude_id = add_source(
        &mut sources,
        "/std/prelude/index.nct",
        "pub struct String {}\n",
    );
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::PackageFile);
    let app = parse_source(&sources, app_id, ParseGoal::ModuleSource);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::ModuleSource);
    let prelude = parse_source(&sources, prelude_id, ParseGoal::ModuleSource);
    let prelude_identity = ModuleIdentity::new(PackageIdentity::new("builtin:std"), ["prelude"]);
    let imports = prepare(
        &sources,
        vec![
            package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
            package("builtin:std", "std", "/std/nocter.nct", &std_manifest),
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

    let error = apply_standard_prelude(imports, &prelude_identity).unwrap_err();

    assert!(matches!(error, PreludeError::AuthoredPreludeImport(_)));
}
