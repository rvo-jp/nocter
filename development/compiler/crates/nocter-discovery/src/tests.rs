use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_compile_input::{
    BuiltinTypeLocator, ModuleIdentity, ModuleSourceKind, PackageMode, PrimitiveRoleLocator,
    StandardRoleLocator, StructuralAttachmentInput, ToolchainInput,
};
use nocter_filesystem::{DocumentVersion, OpenDocument, SourceOverlay};
use nocter_model::{BuiltinType, CompilationTarget, PackageIdentity};
use nocter_package::{PackageRootCatalog, ResolvedPackageGraph, ResolvedPackageSpec};
use nocter_runtime_contract::PrimitiveRole;
use nocter_syntax::{DirectSourceSyntax, NodeKind};
use nocter_toolchain_contract::{StandardDeclarationRole, StructuralAttachment};

use crate::{
    DiscoveredUnit, DiscoveryError, DiscoveryFailure, DiscoveryRequest, UseFailure,
    discover_with_source_syntax,
};

#[path = "tests/standard_contract.rs"]
mod standard_contract;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn discover(request: DiscoveryRequest) -> Result<DiscoveredUnit, DiscoveryFailure> {
    discover_with_source_syntax(request, &mut DirectSourceSyntax)
}

const TEST_BUILTIN_SOURCE: &str = "\
pub primitive type bool\n\
pub primitive type i8\n\
pub primitive type i16\n\
pub primitive type i32\n\
pub primitive type i64\n\
pub primitive type u8\n\
pub primitive type u16\n\
pub primitive type u32\n\
pub primitive type u64\n\
pub primitive type usize\n\
pub primitive type isize\n\
pub primitive type str\n\
pub primitive type error\n\
pub primitive type void\n\
pub primitive type never\n";

struct TempTree(PathBuf);

impl TempTree {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("nocter-discovery-{}-{serial}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn source(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut contents = fs::read_to_string(&path).unwrap_or_default();
        contents.push_str(text);
        fs::write(path, contents).unwrap();
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn package(identity: &str, _name: &str, root: &Path) -> ResolvedPackageSpec {
    ResolvedPackageSpec::new(PackageIdentity::new(identity), root)
}

fn package_graph(packages: Vec<ResolvedPackageSpec>) -> ResolvedPackageGraph {
    package_graph_with_overlay(packages, SourceOverlay::empty())
}

fn package_graph_with_overlay(
    packages: Vec<ResolvedPackageSpec>,
    overlay: SourceOverlay,
) -> ResolvedPackageGraph {
    ResolvedPackageGraph::load_with_root_catalog(
        packages,
        PackageRootCatalog::new(overlay),
        &mut DirectSourceSyntax,
    )
    .unwrap()
}

fn module(package: &str, path: &[&str]) -> ModuleIdentity {
    ModuleIdentity::new(PackageIdentity::new(package), path.iter().copied())
}

fn minimal_toolchain(package: &str) -> ToolchainInput {
    let package = PackageIdentity::new(package);
    ToolchainInput::new(
        package.clone(),
        ModuleIdentity::new(package, Vec::<&str>::new()),
        Vec::new(),
        Vec::new(),
    )
}

fn root_builtin_toolchain(package: &str) -> ToolchainInput {
    let identity = PackageIdentity::new(package);
    let root = ModuleIdentity::new(identity, Vec::<&str>::new());
    minimal_toolchain(package).with_builtin_types(
        BuiltinType::ALL
            .iter()
            .copied()
            .map(|builtin| BuiltinTypeLocator::new(builtin, root.clone(), builtin.spelling()))
            .collect(),
    )
}

#[test]
fn discovery_retains_a_standard_role_locator_without_selecting_syntax() {
    let tree = TempTree::new();
    tree.source(
        "std/index.nct",
        "#package: { name: \"std\", version: \"0.0.0\", }\n",
    );
    tree.source(
        "std/index.nct",
        "see ./defaults.nct\n\npub interface Iterator {\n    pub default method self.count(): usize\n}\n",
    );
    tree.source(
        "std/defaults.nct",
        "see ./index.nct\n\ninterface Iterator {\n    default method self.count(): usize { return 0 }\n}\n",
    );
    let identity = PackageIdentity::new("toolchain:std");
    let standard = package("toolchain:std", "std", &tree.path().join("std"))
        .with_standard_dependency(identity.clone());
    let role = StandardRoleLocator::new(
        StandardDeclarationRole::IteratorInterface,
        ModuleIdentity::new(identity.clone(), Vec::<&str>::new()),
        NodeKind::InterfaceDeclaration,
        "Iterator",
    );

    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![standard]),
        vec![ModuleIdentity::new(identity.clone(), Vec::<&str>::new())],
        ToolchainInput::new(
            identity.clone(),
            ModuleIdentity::new(identity, Vec::<&str>::new()),
            Vec::new(),
            vec![role],
        ),
    ))
    .unwrap();

    assert_eq!(
        unit.compile_input()
            .unwrap()
            .toolchain()
            .unwrap()
            .standard_roles()
            .len(),
        1
    );
}

#[test]
fn discovery_retains_a_primitive_role_locator_without_selecting_syntax() {
    let tree = TempTree::new();
    tree.source(
        "std/index.nct",
        "#package: { name: \"std\", version: \"0.0.0\", }\n",
    );
    tree.source("std/index.nct", "see ./runtime.nct\n");
    tree.source(
        "std/runtime.nct",
        "see ./index.nct\n\nprimitive func new_error(code: &str, message: &str): error\n",
    );
    let identity = PackageIdentity::new("toolchain:std");
    let standard = package("toolchain:std", "std", &tree.path().join("std"))
        .with_standard_dependency(identity.clone());
    let primitive = PrimitiveRoleLocator::new(
        PrimitiveRole::NewError,
        ModuleIdentity::new(identity.clone(), Vec::<&str>::new()),
        "new_error",
    );

    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![standard]),
        vec![ModuleIdentity::new(identity.clone(), Vec::<&str>::new())],
        ToolchainInput::new(
            identity.clone(),
            ModuleIdentity::new(identity, Vec::<&str>::new()),
            Vec::new(),
            Vec::new(),
        )
        .with_primitive_roles(vec![primitive]),
    ))
    .unwrap();

    assert_eq!(
        unit.compile_input()
            .unwrap()
            .toolchain()
            .unwrap()
            .primitive_roles()
            .len(),
        1
    );
}

#[test]
fn discovery_retains_a_builtin_locator_without_selecting_syntax() {
    let tree = TempTree::new();
    tree.source(
        "std/index.nct",
        "#package: { name: \"std\", version: \"0.0.0\", }\n",
    );
    tree.source("std/index.nct", "pub primitive type i32\n");
    let identity = PackageIdentity::new("toolchain:std");
    let standard = package("toolchain:std", "std", &tree.path().join("std"))
        .with_standard_dependency(identity.clone());
    let builtin = BuiltinTypeLocator::new(
        BuiltinType::I32,
        ModuleIdentity::new(identity.clone(), Vec::<&str>::new()),
        "i32",
    );

    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![standard]),
        vec![ModuleIdentity::new(identity.clone(), Vec::<&str>::new())],
        ToolchainInput::new(
            identity.clone(),
            ModuleIdentity::new(identity, Vec::<&str>::new()),
            Vec::new(),
            Vec::new(),
        )
        .with_builtin_types(vec![builtin]),
    ))
    .unwrap();

    let compile_input = unit.compile_input().unwrap();
    let builtins = compile_input.toolchain().unwrap().builtin_types();
    assert_eq!(builtins.len(), 1);
    assert_eq!(builtins[0].builtin(), BuiltinType::I32);
}

#[test]
fn toolchain_standard_layout_catalogs_modules_without_authored_edges() {
    let tree = TempTree::new();
    tree.source(
        "std/index.nct",
        "#package: { name: \"std\", version: \"0.0.0\", }\n",
    );
    tree.source("std/index.nct", "//! Standard root.\n");
    tree.source("std/index.nct", TEST_BUILTIN_SOURCE);
    tree.source(
        "std/unreferenced/index.nct",
        "see ./body.nct\n\npub struct Unreferenced {}\n",
    );
    tree.source(
        "std/unreferenced/body.nct",
        "see ./index.nct\n\nstruct Unreferenced {}\n",
    );
    let identity = PackageIdentity::new("toolchain:std");
    let standard = package("toolchain:std", "std", &tree.path().join("std"))
        .with_standard_dependency(identity.clone());

    let unit = discover(DiscoveryRequest::toolchain_standard(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![standard]),
        root_builtin_toolchain("toolchain:std"),
    ))
    .unwrap();

    let unreferenced = unit
        .modules()
        .iter()
        .find(|candidate| candidate.identity() == &module("toolchain:std", &["unreferenced"]))
        .expect("complete standard catalog reaches an unreferenced child module");
    assert_eq!(unreferenced.sources().len(), 2);
}

#[test]
fn one_open_document_overlay_flows_from_package_data_through_module_discovery() {
    let tree = TempTree::new();
    tree.source(
        "app/index.nct",
        "#package: { name: \"disk-name\", version: \"0.0.0\", }\n",
    );
    tree.source("app/index.nct", "func disk_version(): void { return }\n");
    let manifest = fs::canonicalize(tree.path().join("app/index.nct")).unwrap();
    let root_source = fs::canonicalize(tree.path().join("app/index.nct")).unwrap();
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_document(
            root_source.clone(),
            OpenDocument::new(
                DocumentVersion::new(9),
                &b"#package: { name: \"editor-name\", version: \"0.0.0\", }\nfunc editor_version(): void { return }\n"[..],
            ),
        )
        .unwrap();
    let overlay = overlay.finish();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![package("workspace:app", "app", &tree.path().join("app"))],
            overlay,
        ),
        vec![module("workspace:app", &[])],
        minimal_toolchain("workspace:app"),
    ))
    .unwrap();

    assert_eq!(
        unit.source_overlay().document(&manifest).unwrap().version(),
        DocumentVersion::new(9)
    );
    assert_eq!(
        unit.source_overlay()
            .document(&root_source)
            .unwrap()
            .version(),
        DocumentVersion::new(9)
    );
    let input = unit.compile_input().unwrap();
    assert_eq!(input.packages()[0].display_name(), "editor-name");
    let root = input
        .modules()
        .iter()
        .find(|candidate| candidate.identity() == &module("workspace:app", &[]))
        .unwrap();
    let source = unit
        .sources()
        .get(root.sources()[0].syntax().source())
        .unwrap();
    assert_eq!(
        source.text(),
        "#package: { name: \"editor-name\", version: \"0.0.0\", }\nfunc editor_version(): void { return }\n"
    );
}

#[test]
fn explicit_single_file_converges_on_the_common_compile_unit() {
    let tree = TempTree::new();
    tree.source(
        "app.nct",
        "use std/value.Value\n\nfunc main(): void { return }\n",
    );
    tree.source(
        "std/index.nct",
        "#package: { name: \"std\", version: \"0.0.0\", }\n",
    );
    tree.source("std/index.nct", "//! Standard root.\n");
    tree.source("std/index.nct", TEST_BUILTIN_SOURCE);
    tree.source("std/value/index.nct", "pub struct Value {}\n");
    let standard = package("toolchain:std", "std", &tree.path().join("std"));

    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        tree.path().join("app.nct"),
        package_graph(vec![standard]),
        root_builtin_toolchain("toolchain:std"),
    ))
    .unwrap();
    let input = unit.compile_input().unwrap();
    let single = input
        .packages()
        .iter()
        .find(|package| package.mode() == PackageMode::SingleFile)
        .unwrap();
    assert_eq!(
        input.root_packages(),
        std::slice::from_ref(single.identity())
    );
    assert_eq!(single.display_name(), "app");
    assert_eq!(single.mode(), PackageMode::SingleFile);
    let module = input
        .modules()
        .iter()
        .find(|module| module.identity().package() == single.identity())
        .unwrap();
    assert!(module.identity().path().is_empty());
    assert_eq!(module.sources().len(), 1);
    assert_eq!(module.sources()[0].kind(), ModuleSourceKind::SingleFile);
    assert_eq!(
        input.use_resolutions()[0].target_module().path(),
        [Box::<str>::from("value")]
    );

    let lowered = nocter_declaration_lowering::lower_compile_unit_declarations(&input).unwrap();
    assert_eq!(lowered.program().root_packages().len(), 1);
    assert_eq!(lowered.program().package_targets().len(), 1);
}

#[test]
fn single_file_cannot_open_a_parallel_local_source_graph() {
    let tree = TempTree::new();
    tree.source("app.nct", "use ./helper\n\nfunc main(): void { return }\n");
    tree.source("helper.nct", "func helper(): void { return }\n");
    tree.source(
        "std/index.nct",
        "#package: { name: \"std\", version: \"0.0.0\", }\n",
    );
    tree.source("std/index.nct", "//! Standard root.\n");

    let error = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        tree.path().join("app.nct"),
        package_graph(vec![package(
            "toolchain:std",
            "std",
            &tree.path().join("std"),
        )]),
        minimal_toolchain("toolchain:std"),
    ))
    .unwrap_err();
    assert!(matches!(
        error.error(),
        DiscoveryError::Use {
            failure: UseFailure::SingleFileLocalUse,
            ..
        }
    ));
    assert_eq!(error.diagnostics().len(), 1);
    assert_eq!(error.diagnostics()[0].code(), "E0263");
    assert_eq!(
        error.diagnostics()[0].primary().span().range(),
        nocter_source::TextRange::new(
            nocter_source::ByteOffset::new(4),
            nocter_source::ByteOffset::new(12),
        )
    );
}

#[test]
fn closes_source_folder_module_and_dependency_edges_once() {
    let tree = TempTree::new();
    tree.source(
        "app/index.nct",
        "#package: { name: \"app\", version: \"0.0.0\", }\n#dependencies: { dep: { path: \"../dep\", }, }\n",
    );
    tree.source(
        "app/index.nct",
        "see ./internal/search.nct\nuse ./parser\nuse dep/value.Value\n\nfunc root(): void { return }\n",
    );
    tree.source(
        "app/internal/search.nct",
        "func private_search(): void { return }\n",
    );
    tree.source("app/parser/index.nct", "pub struct Parser {}\n");
    tree.source(
        "dep/index.nct",
        "#package: { name: \"dep\", version: \"0.0.0\", }\n",
    );
    tree.source("dep/index.nct", "//! Dependency root.\n");
    tree.source("dep/value/index.nct", "pub struct Value {}\n");

    let app = package("workspace:app", "app", &tree.path().join("app"))
        .with_dependency("dep", PackageIdentity::new("resolved:dep"));
    let dep = package("resolved:dep", "dep", &tree.path().join("dep"));
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![app, dep]),
        vec![module("workspace:app", &[])],
        minimal_toolchain("workspace:app"),
    ))
    .unwrap();

    let identities: Vec<_> = unit
        .modules()
        .iter()
        .map(|module| module.identity().clone())
        .collect();
    assert_eq!(
        identities,
        vec![
            module("resolved:dep", &[]),
            module("resolved:dep", &["value"]),
            module("workspace:app", &[]),
            module("workspace:app", &["parser"]),
        ]
    );
    let app_root = unit
        .modules()
        .iter()
        .find(|candidate| candidate.identity() == &module("workspace:app", &[]))
        .unwrap();
    assert_eq!(app_root.sources().len(), 2);
    assert!(
        app_root.sources()[0]
            .canonical_path()
            .ends_with("/app/index.nct")
    );
    assert!(
        app_root.sources()[1]
            .canonical_path()
            .ends_with("/app/internal/search.nct")
    );

    let input = unit.compile_input().unwrap();
    assert_eq!(
        input.root_packages(),
        &[PackageIdentity::new("workspace:app")]
    );
    assert_eq!(input.source_visibility_resolutions().len(), 1);
    assert!(
        input.source_visibility_resolutions()[0]
            .target_source()
            .ends_with("/app/internal/search.nct")
    );
    assert_eq!(input.use_resolutions().len(), 2);
    nocter_declaration_lowering::lower_compile_unit_topology(&input).unwrap();
}

#[test]
fn selected_declared_roots_retain_exact_package_target_directives() {
    let tree = TempTree::new();
    tree.source(
        "app/index.nct",
        "#package: { name: \"app\", version: \"0.0.0\", }\n#executable: { name: \"app\" }\n#test: { name: \"unit\", module: \"./tests/unit\" }\n",
    );
    tree.source("app/index.nct", "func main(): void { return }\n");
    tree.source("app/index.nct", TEST_BUILTIN_SOURCE);
    tree.source("app/tests/unit/index.nct", "test works { return }\n");

    let package = PackageIdentity::new("workspace:app");
    let root = ModuleIdentity::new(package.clone(), Vec::<&str>::new());
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![ResolvedPackageSpec::new(
            package.clone(),
            tree.path().join("app"),
        )]),
        vec![
            root.clone(),
            ModuleIdentity::new(package, ["tests", "unit"]),
        ],
        root_builtin_toolchain("workspace:app"),
    ))
    .unwrap();
    let input = unit.compile_input().unwrap();
    assert_eq!(input.package_target_resolutions().len(), 2);
    assert_eq!(input.package_target_resolutions()[0].module(), &root);
    nocter_declaration_lowering::lower_compile_unit_declarations(&input).unwrap();
}

#[test]
fn declared_module_inventory_includes_an_overlay_only_source_with_physical_ownership() {
    let tree = TempTree::new();
    tree.source(
        "app/index.nct",
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
    );
    tree.source("app/index.nct", TEST_BUILTIN_SOURCE);
    let package_root = fs::canonicalize(tree.path().join("app")).unwrap();
    let virtual_source = package_root.join("editor.nct");
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_document(
            virtual_source.clone(),
            OpenDocument::new(
                DocumentVersion::new(1),
                &b"func editor_value(): i32 { return 1 }\n"[..],
            ),
        )
        .unwrap();
    let package = PackageIdentity::new("workspace:app");
    let root = ModuleIdentity::new(package.clone(), Vec::<&str>::new());
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![ResolvedPackageSpec::new(package.clone(), &package_root)],
            overlay.finish(),
        ),
        vec![root.clone()],
        root_builtin_toolchain("workspace:app"),
    ))
    .unwrap();

    let sources = unit
        .modules()
        .iter()
        .find(|module| module.identity() == &root)
        .unwrap()
        .sources();
    assert_eq!(sources.len(), 2);
    assert!(
        sources
            .iter()
            .any(|source| source.canonical_path() == virtual_source.to_str().unwrap())
    );
    nocter_declaration_lowering::lower_compile_unit_declarations(&unit.compile_input().unwrap())
        .unwrap();
}

#[test]
fn use_selects_a_directory_module_even_when_a_same_named_source_exists() {
    let tree = TempTree::new();
    tree.source(
        "app/index.nct",
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
    );
    tree.source("app/index.nct", "use ./search\n");
    tree.source("app/search.nct", "func search(): void { return }\n");
    tree.source(
        "app/search/index.nct",
        "pub func search(): void { return }\n",
    );

    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![package(
            "workspace:app",
            "app",
            &tree.path().join("app"),
        )]),
        Vec::new(),
        minimal_toolchain("workspace:app"),
    ))
    .unwrap();

    let input = unit.compile_input().unwrap();
    assert!(input.source_visibility_resolutions().is_empty());
    assert_eq!(input.use_resolutions().len(), 1);
    assert_eq!(
        input.use_resolutions()[0].target_module(),
        &module("workspace:app", &["search"])
    );
}

#[test]
fn inactive_target_imports_do_not_probe_the_filesystem() {
    let tree = TempTree::new();
    tree.source(
        "app/index.nct",
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
    );
    tree.source(
        "app/index.nct",
        "#target: \"x64-linux\"\nfunc inactive(): void {\n    use ./missing\n    return\n}\nfunc active(): void { return }\n",
    );

    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![package(
            "workspace:app",
            "app",
            &tree.path().join("app"),
        )]),
        Vec::new(),
        minimal_toolchain("workspace:app"),
    ))
    .unwrap();
    assert!(unit.compile_input().unwrap().use_resolutions().is_empty());
}

#[test]
fn incomplete_body_preserves_complete_source_and_module_edges() {
    let tree = TempTree::new();
    tree.source(
        "app/index.nct",
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
    );
    tree.source(
        "app/index.nct",
        concat!(
            "see ./helper.nct\n",
            "use ./value.Value\n",
            "\n",
            "func broken(): void {\n",
            "    let value =\n",
            "}\n",
        ),
    );
    tree.source("app/helper.nct", "func helper(): void { return }\n");
    tree.source("app/value/index.nct", "pub struct Value {}\n");

    let root = module("workspace:app", &[]);
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![package(
            "workspace:app",
            "app",
            &tree.path().join("app"),
        )]),
        vec![root.clone()],
        minimal_toolchain("workspace:app"),
    ))
    .unwrap();

    assert!(unit.has_syntax_errors());
    assert!(unit.compile_input().is_err());
    let input = unit.analysis_input().unwrap();
    assert_eq!(input.source_visibility_resolutions().len(), 1);
    assert_eq!(input.use_resolutions().len(), 1);
    assert_eq!(
        input.use_resolutions()[0].target_module(),
        &module("workspace:app", &["value"])
    );
    let root_sources = input
        .modules()
        .iter()
        .find(|candidate| candidate.identity() == &root)
        .unwrap()
        .sources();
    assert_eq!(root_sources.len(), 2);
}

#[test]
fn incomplete_source_edges_are_not_resolved() {
    let tree = TempTree::new();
    tree.source(
        "app/index.nct",
        concat!(
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
            "see\n",
            "use\n",
        ),
    );

    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![package(
            "workspace:app",
            "app",
            &tree.path().join("app"),
        )]),
        vec![module("workspace:app", &[])],
        minimal_toolchain("workspace:app"),
    ))
    .unwrap();

    assert!(unit.has_syntax_errors());
    let input = unit.analysis_input().unwrap();
    assert!(input.source_visibility_resolutions().is_empty());
    assert!(input.use_resolutions().is_empty());
}

#[test]
fn unknown_target_reaches_the_authored_declaration_diagnostic_boundary() {
    let tree = TempTree::new();
    tree.source(
        "app/index.nct",
        concat!(
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
            "#target: \"mips-templeos\"\n",
            "func unavailable(): void { return }\n",
        ),
    );

    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![package(
            "workspace:app",
            "app",
            &tree.path().join("app"),
        )]),
        vec![module("workspace:app", &[])],
        minimal_toolchain("workspace:app"),
    ))
    .unwrap();
    let input = unit.compile_input().unwrap();

    let Err(nocter_declaration_lowering::DeclarationLoweringError::Surface(diagnostic)) =
        nocter_declaration_lowering::lower_compile_unit_declarations(&input)
    else {
        panic!("unknown target did not reach the authored surface diagnostic");
    };
    assert_eq!(
        diagnostic.rule(),
        nocter_declaration_lowering::SurfaceRule::UnknownTargetGate
    );
}

#[test]
fn canonical_output_does_not_depend_on_request_order() {
    let tree = TempTree::new();
    for name in ["a", "b"] {
        tree.source(
            &format!("{name}/index.nct"),
            &format!("#package: {{ name: \"{name}\", version: \"0.0.0\", }}\n"),
        );
    }
    let a = package("workspace:a", "a", &tree.path().join("a"));
    let b = package("workspace:b", "b", &tree.path().join("b"));

    let forward = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![a.clone(), b.clone()]),
        vec![module("workspace:b", &[]), module("workspace:a", &[])],
        minimal_toolchain("workspace:a"),
    ))
    .unwrap();
    let reverse = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![b, a]),
        vec![module("workspace:a", &[]), module("workspace:b", &[])],
        minimal_toolchain("workspace:a"),
    ))
    .unwrap();

    let shape = |unit: &crate::DiscoveredUnit| {
        unit.modules()
            .iter()
            .map(|module| {
                (
                    module.identity().clone(),
                    module
                        .sources()
                        .iter()
                        .map(|source| source.canonical_path().to_owned())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(shape(&forward), shape(&reverse));
    let expected_roots = [
        PackageIdentity::new("workspace:a"),
        PackageIdentity::new("workspace:b"),
    ];
    assert_eq!(forward.root_packages(), expected_roots.as_slice());
    assert_eq!(reverse.root_packages(), expected_roots.as_slice());
    assert_eq!(
        forward.semantic_topology_surface().unwrap(),
        reverse.semantic_topology_surface().unwrap()
    );
}

#[test]
fn semantic_topology_ignores_body_contents_and_block_uses() {
    let tree = TempTree::new();
    tree.source(
        "app/index.nct",
        "#package: { name: \"app\", version: \"0.0.0\", }\nuse ./first\nuse ./second\n\nfunc main(): void {\n    use ./first\n    return\n}\n",
    );
    tree.source("app/first/index.nct", "pub func first(): void { return }\n");
    tree.source(
        "app/second/index.nct",
        "pub func second(): void { return }\n",
    );
    let identity = PackageIdentity::new("workspace:app");
    let request = || {
        DiscoveryRequest::declared(
            CompilationTarget::Arm64Darwin,
            package_graph(vec![package(
                "workspace:app",
                "app",
                &tree.path().join("app"),
            )]),
            vec![ModuleIdentity::new(identity.clone(), Vec::<&str>::new())],
            minimal_toolchain("workspace:app"),
        )
    };
    let before_unit = discover(request()).unwrap();
    let before = before_unit.semantic_topology_surface().unwrap();
    let before_current = before_unit.current_source_surface().unwrap();

    fs::write(
        tree.path().join("app/index.nct"),
        "#package: { name: \"app\", version: \"0.0.0\", }\nuse ./first\nuse ./second\n\nfunc main(): void {\n    use ./second\n    let changed = 1\n    return\n}\n",
    )
    .unwrap();
    let after_unit = discover(request()).unwrap();
    let after = after_unit.semantic_topology_surface().unwrap();
    let after_current = after_unit.current_source_surface().unwrap();

    assert_eq!(before, after);
    assert_ne!(before_current, after_current);
}

#[test]
fn semantic_topology_tracks_top_level_use_selection() {
    let tree = TempTree::new();
    tree.source(
        "app/index.nct",
        "#package: { name: \"app\", version: \"0.0.0\", }\nuse ./first\n",
    );
    tree.source("app/first/index.nct", "pub func first(): void { return }\n");
    tree.source(
        "app/second/index.nct",
        "pub func second(): void { return }\n",
    );
    let identity = PackageIdentity::new("workspace:app");
    let request = || {
        DiscoveryRequest::declared(
            CompilationTarget::Arm64Darwin,
            package_graph(vec![package(
                "workspace:app",
                "app",
                &tree.path().join("app"),
            )]),
            vec![ModuleIdentity::new(identity.clone(), Vec::<&str>::new())],
            minimal_toolchain("workspace:app"),
        )
    };
    let before = discover(request())
        .unwrap()
        .semantic_topology_surface()
        .unwrap();

    fs::write(
        tree.path().join("app/index.nct"),
        "#package: { name: \"app\", version: \"0.0.0\", }\nuse ./second\n",
    )
    .unwrap();
    let after = discover(request())
        .unwrap()
        .semantic_topology_surface()
        .unwrap();

    assert_ne!(before, after);
}

#[test]
fn authored_standard_library_is_one_discoverable_declaration_unit() {
    let standard_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std");
    let standard_identity = PackageIdentity::new("toolchain:std");
    let standard = package("toolchain:std", "std", &standard_root)
        .with_standard_dependency(standard_identity.clone());

    let unit = discover(DiscoveryRequest::toolchain_standard(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![standard]),
        standard_toolchain(&standard_identity),
    ))
    .unwrap();
    let syntax_errors: Vec<_> = unit
        .syntax_trees()
        .iter()
        .filter(|tree| tree.has_errors())
        .map(|tree| {
            (
                unit.sources()
                    .get(tree.source())
                    .unwrap()
                    .name()
                    .to_string(),
                tree.lexed().diagnostics(),
                tree.diagnostics(),
            )
        })
        .collect();
    assert!(syntax_errors.is_empty(), "{syntax_errors:#?}");
    let rooted_bodies = unit
        .modules()
        .iter()
        .flat_map(crate::DiscoveredModule::sources)
        .filter(|source| source.kind() == ModuleSourceKind::Root)
        .filter(|source| {
            unit.syntax_trees()[source.syntax_index()]
                .nodes()
                .any(|(_, node)| node.kind() == NodeKind::Block)
        })
        .map(crate::DiscoveredSource::canonical_path)
        .collect::<Vec<_>>();
    assert!(
        rooted_bodies.is_empty(),
        "standard module roots must remain contract-only: {rooted_bodies:#?}"
    );
    standard_contract::assert_standard_root_visibility_boundaries(&unit);
    let input = unit.compile_input().unwrap();
    standard_contract::assert_standard_self_uses_are_package_absolute(&input);
    standard_contract::assert_reviewed_standard_dependencies(&input);
    let lowered = nocter_declaration_lowering::lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared = nocter_checking::prepare_program_checking(
        &input,
        program,
        &frontend_bindings,
        source_index,
    )
    .unwrap();
    let checked =
        nocter_checking::check_prepared_program(&input, prepared).unwrap_or_else(|error| {
            let source = error
                .source_diagnostic()
                .and_then(|diagnostic| unit.sources().get(diagnostic.primary().source()))
                .map(|source| source.name().to_string());
            panic!("standard source {source:?} failed body checking: {error:?}")
        });
    standard_contract::assert_package_visible_functions_have_cross_module_references(
        &unit, &checked,
    );
    standard_contract::assert_private_implementation_functions_are_referenced(&unit, &checked);
}

fn standard_toolchain(package: &PackageIdentity) -> ToolchainInput {
    let module = |path: &[&str]| ModuleIdentity::new(package.clone(), path.iter().copied());
    let attachments = vec![StructuralAttachmentInput::new(
        StructuralAttachment::Slice,
        module(&["slice"]),
    )];
    let roles = [
        (
            StandardDeclarationRole::AbortingAllocator,
            &["mem"][..],
            NodeKind::StructDeclaration,
            "Allocator",
        ),
        (
            StandardDeclarationRole::AllocationContext,
            &["mem"][..],
            NodeKind::StructDeclaration,
            "AllocationContext",
        ),
        (
            StandardDeclarationRole::OwnedString,
            &["string"][..],
            NodeKind::StructDeclaration,
            "String",
        ),
        (
            StandardDeclarationRole::InterpolationConstructor,
            &["string"][..],
            NodeKind::ConstructionFunction,
            "empty",
        ),
        (
            StandardDeclarationRole::InterpolationTextAppender,
            &["string"][..],
            NodeKind::InherentMethod,
            "push_str",
        ),
        (
            StandardDeclarationRole::FormatInterface,
            &["fmt"][..],
            NodeKind::InterfaceDeclaration,
            "Format",
        ),
        (
            StandardDeclarationRole::FormatMethod,
            &["fmt"][..],
            NodeKind::InterfaceMethod,
            "format_into",
        ),
        (
            StandardDeclarationRole::IteratorInterface,
            &["iter"][..],
            NodeKind::InterfaceDeclaration,
            "Iterator",
        ),
        (
            StandardDeclarationRole::IteratorItem,
            &["iter"][..],
            NodeKind::AssociatedTypeDeclaration,
            "Item",
        ),
        (
            StandardDeclarationRole::IteratorNextMethod,
            &["iter"][..],
            NodeKind::InterfaceMethod,
            "next",
        ),
        (
            StandardDeclarationRole::ExactSizeIteratorInterface,
            &["iter"][..],
            NodeKind::InterfaceDeclaration,
            "ExactSizeIterator",
        ),
        (
            StandardDeclarationRole::ExactSizeIteratorRemainingLenMethod,
            &["iter"][..],
            NodeKind::InterfaceMethod,
            "remaining_len",
        ),
    ]
    .into_iter()
    .map(|(role, path, kind, name)| StandardRoleLocator::new(role, module(path), kind, name))
    .collect();
    let primitives = PrimitiveRole::ALL
        .iter()
        .copied()
        .map(|role| {
            let (path, name) = nocter_test_support::primitive_source_location(role);
            PrimitiveRoleLocator::new(role, module(path), name)
        })
        .collect();
    ToolchainInput::new(package.clone(), module(&["prelude"]), attachments, roles)
        .with_primitive_roles(primitives)
        .with_builtin_types(standard_builtin_types(package))
}

fn standard_builtin_types(package: &PackageIdentity) -> Vec<BuiltinTypeLocator> {
    let module = |path: &[&str]| ModuleIdentity::new(package.clone(), path.iter().copied());
    BuiltinType::ALL
        .iter()
        .copied()
        .map(|builtin| {
            let path = match builtin {
                BuiltinType::Bool
                | BuiltinType::I8
                | BuiltinType::I16
                | BuiltinType::I32
                | BuiltinType::I64
                | BuiltinType::U8
                | BuiltinType::U16
                | BuiltinType::U32
                | BuiltinType::U64
                | BuiltinType::Usize
                | BuiltinType::Isize => &["num"][..],
                BuiltinType::Str => &["str"][..],
                BuiltinType::Error => &["error"][..],
                BuiltinType::Void | BuiltinType::Never => &["core"][..],
            };
            BuiltinTypeLocator::new(builtin, module(path), builtin.spelling())
        })
        .collect()
}
