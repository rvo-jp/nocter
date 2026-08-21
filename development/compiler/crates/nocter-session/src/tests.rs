use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_compile_input::{ModuleIdentity, PackageIdentity};
use nocter_discovery::{DiscoveryRequest, ResolvedPackage, discover};
use nocter_model::CompilationTarget;
use nocter_target_program::PrimitiveRole;

use super::{
    ExecutableCompileRequest, NativeImageSetCompileRequest, bundled_standard_toolchain,
    compile_native_image, compile_native_images, compile_target,
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempPackage(PathBuf);

impl TempPackage {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "nocter-session-package-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn source(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

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
        compile_native_image(ExecutableCompileRequest::only(&unit))
            .unwrap_or_else(|error| panic!("{} failed compilation: {error:?}", source.display()));
    }
}

#[test]
fn public_package_example_crosses_the_complete_target_session() {
    let compiler = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler.join("../std");
    let example_root = compiler.join("../../examples/file-summary");
    let standard_package = PackageIdentity::new("toolchain:std");
    let example_package = PackageIdentity::new("workspace:file-summary");
    let example = ResolvedPackage::new(example_package.clone(), "file-summary", &example_root)
        .with_dependency("std", standard_package.clone());
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        vec![
            example,
            resolved_standard(&standard_root, &standard_package),
        ],
        vec![ModuleIdentity::new(
            example_package.clone(),
            Vec::<&str>::new(),
        )],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let target =
        compile_native_image(ExecutableCompileRequest::named(&unit, "file-summary")).unwrap();

    assert_eq!(target.identity().name(), "file-summary");
    assert_eq!(target.identity().package(), &example_package);
    assert!(!target.image().bytes().is_empty());
}

#[test]
fn all_root_executables_share_one_target_compilation_and_keep_declaration_order() {
    let compiler = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "nocter.nct",
        "#name: \"multi\"\n#executable: { name: \"first\", module: \"./first\" }\n#executable: { name: \"second\", module: \"./second\" }\n",
    );
    package_root.source("index.nct", "//! Multi executable package.\n");
    package_root.source("first/index.nct", "func main(): void { return }\n");
    package_root.source("second/index.nct", "func main(): void { return }\n");
    let standard_package = PackageIdentity::new("toolchain:std");
    let package = PackageIdentity::new("workspace:multi");
    let resolved = ResolvedPackage::new(package.clone(), "multi", &package_root.0)
        .with_dependency("std", standard_package.clone());
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        vec![
            resolved,
            resolved_standard(&standard_root, &standard_package),
        ],
        vec![
            ModuleIdentity::new(package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(package.clone(), ["first"]),
            ModuleIdentity::new(package.clone(), ["second"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let image_set = compile_native_images(NativeImageSetCompileRequest::all(&unit)).unwrap();
    assert_eq!(
        image_set
            .entries()
            .iter()
            .map(|entry| entry.identity().name())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert!(
        image_set
            .entries()
            .iter()
            .all(|entry| entry.identity().package() == &package)
    );
    assert!(
        image_set
            .entries()
            .iter()
            .all(|entry| entry.image().bytes().starts_with(&[0xcf, 0xfa, 0xed, 0xfe]))
    );
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
