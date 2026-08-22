use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_compile_input::{ModuleIdentity, PackageIdentity};
use nocter_discovery::{DiscoveryRequest, discover};
use nocter_model::CompilationTarget;
use nocter_package::{ResolvedPackageGraph, ResolvedPackageSpec};
use nocter_target_program::PrimitiveRole;

use super::{
    ExecutableCompileRequest, NativeImageSetCompileRequest, NativeTestCompileRequest,
    NativeTestTargetOutcome, analyze_incomplete_syntax, analyze_target, bundled_standard_toolchain,
    compile_native_image, compile_native_images, compile_native_tests, compile_target,
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
        package_graph(vec![resolved]),
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
fn body_failure_retains_preparation_and_exact_typed_interruption() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "func helper(): i32 { 1 }\nfunc main(input: i32): void {\n    input.missing()\n    return\n}\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let failure = analyze_target(&unit).unwrap_err();
    assert!(failure.error().source_diagnostic().is_some());
    let prepared = failure.prepared().unwrap();
    assert!(!prepared.graph().declarations().callables().is_empty());
    assert!(!prepared.body_names().is_empty());
    assert!(!prepared.source_index().is_empty());
    let interruption = failure
        .recovery()
        .unwrap()
        .bodies()
        .unwrap()
        .interruption()
        .unwrap();
    assert_eq!(
        interruption.origin().span(),
        failure
            .error()
            .source_diagnostic()
            .unwrap()
            .primary()
            .span()
    );
    assert!(matches!(
        interruption.kind(),
        nocter_checking::TypedBodyInterruptionKind::MemberSelection { .. }
    ));
}

#[test]
fn name_failure_retains_lexical_state_without_claiming_body_preparation() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "func main(input: i32): void {\n    let before = input\n    unknown\n    let after = input\n    return\n}\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let failure = analyze_target(&unit).unwrap_err();
    assert_eq!(failure.error().source_diagnostic().unwrap().code(), "E0340");
    assert!(failure.prepared().is_none());
    let recovery = failure.recovery().unwrap().names().unwrap();
    assert!(!recovery.graph().declarations().callables().is_empty());
    assert!(!recovery.body_names().is_empty());
    assert!(!recovery.source_index().is_empty());
}

#[test]
fn incomplete_member_syntax_retains_typed_receiver_context() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "struct Text { value: i32 }\ninstance Text { pub method &self.len(): usize { 0 } }\nfunc inspect(value: &Text): void {\n    value.\n    return\n}\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(unit.has_syntax_errors());
    let recovery = analyze_incomplete_syntax(&unit).expect("typed syntax recovery");
    assert!(matches!(
        recovery.interruption().unwrap().kind(),
        nocter_checking::TypedBodyInterruptionKind::MemberSelection { .. }
    ));
}

#[test]
fn incomplete_declaration_syntax_cannot_enter_body_recovery() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source("main.nct", "func broken(: void { return }\n");
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(unit.has_syntax_errors());
    assert!(analyze_incomplete_syntax(&unit).is_none());
}

#[test]
fn every_public_single_file_example_crosses_the_complete_target_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let examples = compiler_root.join("../../examples");
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
            package_graph(vec![resolved_standard(&standard_root, &package)]),
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
    let example = ResolvedPackageSpec::new(example_package.clone(), &example_root)
        .with_standard_dependency(standard_package.clone());
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![
            example,
            resolved_standard(&standard_root, &standard_package),
        ]),
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
    let resolved = ResolvedPackageSpec::new(package.clone(), &package_root.0)
        .with_standard_dependency(standard_package.clone());
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![
            resolved,
            resolved_standard(&standard_root, &standard_package),
        ]),
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

#[test]
fn native_test_set_preserves_target_and_case_declaration_identity() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "nocter.nct",
        "#name: \"tests\"\n#test: { name: \"unit\", module: \"./unit\" }\n#test: { name: \"integration\", module: \"./integration\" }\n",
    );
    package_root.source("index.nct", "//! Test package.\n");
    package_root.source(
        "unit/index.nct",
        "test first { return }\ntest second { return }\n",
    );
    package_root.source("integration/index.nct", "test external { return }\n");
    let standard_package = PackageIdentity::new("toolchain:std");
    let package = PackageIdentity::new("workspace:tests");
    let resolved = ResolvedPackageSpec::new(package.clone(), &package_root.0)
        .with_standard_dependency(standard_package.clone());
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![
            resolved,
            resolved_standard(&standard_root, &standard_package),
        ]),
        vec![
            ModuleIdentity::new(package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(package.clone(), ["unit"]),
            ModuleIdentity::new(package.clone(), ["integration"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let compiled = compile_native_tests(NativeTestCompileRequest::all(&unit)).unwrap();
    assert_eq!(
        compiled
            .targets()
            .iter()
            .map(|target| target.identity().name())
            .collect::<Vec<_>>(),
        ["unit", "integration"]
    );
    assert_eq!(
        compiled
            .targets()
            .iter()
            .map(|target| match target.outcome() {
                NativeTestTargetOutcome::Compiled(cases) => cases
                    .iter()
                    .map(|case| case.identity().name())
                    .collect::<Vec<_>>(),
                NativeTestTargetOutcome::CompileFailed(error) => {
                    panic!("test target failed native compilation: {error}")
                }
            })
            .collect::<Vec<_>>(),
        [vec!["first", "second"], vec!["external"]]
    );
    assert!(compiled.targets().iter().all(|target| {
        target.identity().package() == &package
            && match target.outcome() {
                NativeTestTargetOutcome::Compiled(cases) => cases
                    .iter()
                    .all(|case| case.image().bytes().starts_with(&[0xcf, 0xfa, 0xed, 0xfe])),
                NativeTestTargetOutcome::CompileFailed(_) => false,
            }
    }));

    let selected =
        compile_native_tests(NativeTestCompileRequest::case(&unit, "unit", "second")).unwrap();
    let NativeTestTargetOutcome::Compiled(cases) = selected.targets()[0].outcome() else {
        panic!("selected case failed native compilation")
    };
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].identity().name(), "second");
}

fn resolved_standard(root: &Path, package: &PackageIdentity) -> ResolvedPackageSpec {
    ResolvedPackageSpec::new(package.clone(), root).with_standard_dependency(package.clone())
}

fn package_graph(packages: Vec<ResolvedPackageSpec>) -> ResolvedPackageGraph {
    ResolvedPackageGraph::load(packages).unwrap()
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
