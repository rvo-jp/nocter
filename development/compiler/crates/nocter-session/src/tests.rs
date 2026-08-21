use std::fs;
use std::path::{Path, PathBuf};

use nocter_compile_input::{ModuleIdentity, PackageIdentity};
use nocter_discovery::{DiscoveryRequest, ResolvedPackage, discover};
use nocter_model::CompilationTarget;
use nocter_target_program::PrimitiveRole;

use super::{bundled_standard_toolchain, compile_target};

#[test]
fn bundled_standard_library_crosses_the_complete_target_session() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std");
    let package = PackageIdentity::new("toolchain:std");
    let resolved = resolved_standard(&root, &package);
    let roots = module_roots(&root)
        .into_iter()
        .map(|path| ModuleIdentity::new(package.clone(), path))
        .collect();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        vec![resolved],
        roots,
        bundled_standard_toolchain(&package),
    ))
    .unwrap();
    let compiled = compile_target(&unit).unwrap();

    assert_eq!(
        compiled.program().toolchain().primitives().bindings().len(),
        PrimitiveRole::ALL.len()
    );
    assert_eq!(
        compiled.program().checked().bodies().len(),
        compiled
            .program()
            .checked()
            .graph()
            .declarations()
            .bodies()
            .len()
    );
}

#[test]
fn every_public_single_file_example_crosses_the_complete_target_session() {
    let compiler = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler.join("../std");
    let examples = compiler.join("../../examples");
    let package = PackageIdentity::new("toolchain:std");
    let mut sources = fs::read_dir(&examples)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "nct"))
        .collect::<Vec<_>>();
    sources.sort();
    assert!(!sources.is_empty());

    for source in sources {
        let unit = discover(DiscoveryRequest::single_file(
            CompilationTarget::Arm64Darwin,
            &source,
            vec![resolved_standard(&standard_root, &package)],
            bundled_standard_toolchain(&package),
        ))
        .unwrap_or_else(|error| panic!("{} failed discovery: {error:?}", source.display()));
        compile_target(&unit)
            .unwrap_or_else(|error| panic!("{} failed compilation: {error:?}", source.display()));
    }
}

fn resolved_standard(root: &Path, package: &PackageIdentity) -> ResolvedPackage {
    ResolvedPackage::new(package.clone(), "std", root).with_dependency("std", package.clone())
}

fn module_roots(root: &Path) -> Vec<Vec<Box<str>>> {
    let mut pending = vec![(root.to_path_buf(), Vec::new())];
    let mut modules = Vec::new();
    while let Some((directory, path)) = pending.pop() {
        if directory.join("index.nct").is_file() {
            modules.push(path.clone());
        }
        let mut children = fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_dir())
            .collect::<Vec<PathBuf>>();
        children.sort();
        for child in children.into_iter().rev() {
            let mut child_path = path.clone();
            child_path.push(
                child
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
                    .into(),
            );
            pending.push((child, child_path));
        }
    }
    modules.sort();
    modules
}
