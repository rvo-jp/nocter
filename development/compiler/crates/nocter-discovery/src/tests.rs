use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_compile_input::{
    BuiltinAttachmentInput, ModuleIdentity, ModuleSourceKind, PackageIdentity, PackageMode,
    UseTargetInput,
};
use nocter_declarations::{BuiltinAttachment, PrimitiveRole, StandardDeclarationRole};
use nocter_model::CompilationTarget;
use nocter_syntax::NodeKind;

use crate::{
    DiscoveryError, DiscoveryRequest, ImportFailure, PrimitiveRoleLocator, ResolvedPackage,
    StandardRoleLocator, ToolchainRequest, discover,
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

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
        fs::write(path, text).unwrap();
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn package(identity: &str, name: &str, root: &Path) -> ResolvedPackage {
    ResolvedPackage::new(PackageIdentity::new(identity), name, root)
}

fn module(package: &str, path: &[&str]) -> ModuleIdentity {
    ModuleIdentity::new(PackageIdentity::new(package), path.iter().copied())
}

fn minimal_toolchain(package: &str) -> ToolchainRequest {
    let package = PackageIdentity::new(package);
    ToolchainRequest::new(
        package.clone(),
        ModuleIdentity::new(package, Vec::<&str>::new()),
        Vec::new(),
        Vec::new(),
    )
}

#[test]
fn explicit_single_file_converges_on_the_common_compile_unit() {
    let tree = TempTree::new();
    tree.source(
        "app.nct",
        "use std/value.Value\n\nfunc main(): void { return }\n",
    );
    tree.source("std/nocter.nct", "#name: \"std\"\n");
    tree.source("std/index.nct", "//! Standard root.\n");
    tree.source("std/value/index.nct", "pub struct Value {}\n");
    let standard = package("toolchain:std", "std", &tree.path().join("std"));

    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        tree.path().join("app.nct"),
        vec![standard],
        minimal_toolchain("toolchain:std"),
    ))
    .unwrap();
    let input = unit.compile_input().unwrap();
    let single = input
        .packages()
        .iter()
        .find(|package| package.mode() == PackageMode::SingleFile)
        .unwrap();
    assert_eq!(single.display_name(), "app");
    assert!(single.declaration().is_none());
    let module = input
        .modules()
        .iter()
        .find(|module| module.identity().package() == single.identity())
        .unwrap();
    assert!(module.identity().path().is_empty());
    assert_eq!(module.sources().len(), 1);
    assert_eq!(module.sources()[0].kind(), ModuleSourceKind::SingleFile);
    assert!(matches!(
        input.use_resolutions()[0].target(),
        UseTargetInput::Module(module) if module.path() == [Box::<str>::from("value")]
    ));

    let lowered = nocter_declaration_lowering::lower_compile_unit_declarations(&input).unwrap();
    assert_eq!(lowered.program().package_targets().len(), 1);
}

#[test]
fn single_file_cannot_open_a_parallel_local_source_graph() {
    let tree = TempTree::new();
    tree.source("app.nct", "use ./helper\n\nfunc main(): void { return }\n");
    tree.source("helper.nct", "func helper(): void { return }\n");
    tree.source("std/nocter.nct", "#name: \"std\"\n");
    tree.source("std/index.nct", "//! Standard root.\n");

    let error = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        tree.path().join("app.nct"),
        vec![package("toolchain:std", "std", &tree.path().join("std"))],
        minimal_toolchain("toolchain:std"),
    ))
    .unwrap_err();
    assert!(matches!(
        error,
        DiscoveryError::Import {
            failure: ImportFailure::SingleFileLocalImport,
            ..
        }
    ));
}

#[test]
fn closes_source_folder_module_and_dependency_edges_once() {
    let tree = TempTree::new();
    tree.source("app/nocter.nct", "#name: \"app\"\n");
    tree.source(
        "app/index.nct",
        "use ./internal/search\nuse ./parser\nuse dep/value.Value\n\nfunc root(): void { return }\n",
    );
    tree.source(
        "app/internal/search.nct",
        "func private_search(): void { return }\n",
    );
    tree.source("app/parser/index.nct", "pub struct Parser {}\n");
    tree.source("dep/nocter.nct", "#name: \"dep\"\n");
    tree.source("dep/index.nct", "//! Dependency root.\n");
    tree.source("dep/value/index.nct", "pub struct Value {}\n");

    let app = package("workspace:app", "app", &tree.path().join("app"))
        .with_dependency("dep", PackageIdentity::new("resolved:dep"));
    let dep = package("resolved:dep", "dep", &tree.path().join("dep"));
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        vec![app, dep],
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
    assert_eq!(input.use_resolutions().len(), 3);
    assert!(matches!(
        input.use_resolutions()[0].target(),
        UseTargetInput::Source(path) if path.ends_with("/app/internal/search.nct")
    ));
    nocter_declaration_lowering::lower_compile_unit_topology(&input).unwrap();
}

#[test]
fn rejects_a_relative_path_with_both_source_and_module_candidates() {
    let tree = TempTree::new();
    tree.source("app/nocter.nct", "");
    tree.source("app/index.nct", "use ./search\n");
    tree.source("app/search.nct", "func search(): void { return }\n");
    tree.source(
        "app/search/index.nct",
        "pub func search(): void { return }\n",
    );

    let error = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        vec![package("workspace:app", "app", &tree.path().join("app"))],
        Vec::new(),
        minimal_toolchain("workspace:app"),
    ))
    .unwrap_err();

    assert!(matches!(
        error,
        DiscoveryError::Import {
            failure: ImportFailure::Ambiguous { .. },
            ..
        }
    ));
}

#[test]
fn inactive_target_imports_do_not_probe_the_filesystem() {
    let tree = TempTree::new();
    tree.source("app/nocter.nct", "");
    tree.source(
        "app/index.nct",
        "#target: \"x64-linux\"\nfunc inactive(): void {\n    use ./missing\n    return\n}\nfunc active(): void { return }\n",
    );

    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        vec![package("workspace:app", "app", &tree.path().join("app"))],
        Vec::new(),
        minimal_toolchain("workspace:app"),
    ))
    .unwrap();
    assert!(unit.compile_input().unwrap().use_resolutions().is_empty());
}

#[test]
fn canonical_output_does_not_depend_on_request_order() {
    let tree = TempTree::new();
    for name in ["a", "b"] {
        tree.source(&format!("{name}/nocter.nct"), "");
        tree.source(&format!("{name}/index.nct"), "");
    }
    let a = package("workspace:a", "a", &tree.path().join("a"));
    let b = package("workspace:b", "b", &tree.path().join("b"));

    let forward = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        vec![a.clone(), b.clone()],
        vec![module("workspace:b", &[]), module("workspace:a", &[])],
        minimal_toolchain("workspace:a"),
    ))
    .unwrap();
    let reverse = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        vec![b, a],
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
}

#[test]
fn authored_standard_library_is_one_discoverable_declaration_unit() {
    let standard_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std");
    let standard_identity = PackageIdentity::new("toolchain:std");
    let standard = package("toolchain:std", "std", &standard_root)
        .with_dependency("std", standard_identity.clone());
    let roots = module_root_paths(&standard_root)
        .into_iter()
        .map(|path| ModuleIdentity::new(standard_identity.clone(), path))
        .collect();

    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        vec![standard],
        roots,
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
    let input = unit.compile_input().unwrap();
    let lowered = nocter_declaration_lowering::lower_compile_unit_declarations(&input).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared =
        nocter_checking::prepare_program_checking(&input, program, source_index).unwrap();
    nocter_checking::check_prepared_program(&input, prepared).unwrap_or_else(|error| {
        let source = error
            .source_diagnostic()
            .and_then(|diagnostic| unit.sources().get(diagnostic.primary().source()))
            .map(|source| source.name().to_string());
        panic!("standard source {source:?} failed body checking: {error:?}")
    });
}

fn standard_toolchain(package: &PackageIdentity) -> ToolchainRequest {
    let module = |path: &[&str]| ModuleIdentity::new(package.clone(), path.iter().copied());
    let attachments = [
        (BuiltinAttachment::Scalar, "num"),
        (BuiltinAttachment::Str, "str"),
        (BuiltinAttachment::Error, "error"),
        (BuiltinAttachment::Slice, "slice"),
    ]
    .into_iter()
    .map(|(attachment, path)| BuiltinAttachmentInput::new(attachment, module(&[path])))
    .collect();
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
            let (path, name) = nocter_target_program::primitive_source_location(role);
            PrimitiveRoleLocator::new(role, module(path), NodeKind::PrimitiveDeclaration, name)
        })
        .collect();
    ToolchainRequest::new(package.clone(), module(&["prelude"]), attachments, roles)
        .with_primitive_roles(primitives)
}

fn module_root_paths(root: &Path) -> Vec<Vec<Box<str>>> {
    let mut result = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let mut entries: Vec<_> = fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for entry in entries.into_iter().rev() {
            if entry.is_dir() {
                pending.push(entry);
            }
        }
        if directory.join("index.nct").is_file() {
            let relative = directory.strip_prefix(root).unwrap();
            result.push(
                relative
                    .components()
                    .map(|component| {
                        component
                            .as_os_str()
                            .to_str()
                            .expect("authored standard module path is Unicode")
                            .into()
                    })
                    .collect(),
            );
        }
    }
    result.sort();
    result
}
