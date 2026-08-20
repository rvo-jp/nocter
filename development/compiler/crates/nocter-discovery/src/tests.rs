use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_compile_input::{ModuleIdentity, PackageIdentity, UseTargetInput};
use nocter_model::CompilationTarget;

use crate::{DiscoveryError, DiscoveryRequest, ImportFailure, ResolvedPackage, discover};

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
    let unit = discover(DiscoveryRequest::new(
        CompilationTarget::Arm64Darwin,
        vec![app, dep],
        vec![module("workspace:app", &[])],
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

    let error = discover(DiscoveryRequest::new(
        CompilationTarget::Arm64Darwin,
        vec![package("workspace:app", "app", &tree.path().join("app"))],
        Vec::new(),
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

    let unit = discover(DiscoveryRequest::new(
        CompilationTarget::Arm64Darwin,
        vec![package("workspace:app", "app", &tree.path().join("app"))],
        Vec::new(),
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

    let forward = discover(DiscoveryRequest::new(
        CompilationTarget::Arm64Darwin,
        vec![a.clone(), b.clone()],
        vec![module("workspace:b", &[]), module("workspace:a", &[])],
    ))
    .unwrap();
    let reverse = discover(DiscoveryRequest::new(
        CompilationTarget::Arm64Darwin,
        vec![b, a],
        vec![module("workspace:a", &[]), module("workspace:b", &[])],
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

    let unit = discover(DiscoveryRequest::new(
        CompilationTarget::Arm64Darwin,
        vec![standard],
        roots,
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
    let prelude = ModuleIdentity::new(standard_identity, ["prelude"]);
    let input = unit.compile_input().unwrap();
    nocter_declaration_lowering::lower_compile_unit_declarations(&input, &prelude).unwrap();
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
