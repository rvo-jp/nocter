use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn cryptographic_randomness_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        r"
use std/random
use std/vec.Vec

func main(): i32! {
    var output: Vec<u8> = Vec.with_capacity(257)
    var index: usize = 0
    while index < 257 {
        output.push(0)
        index += 1
    }
    random.fill(&+output)?
    let _wide = random.next_u64()?
    let _native = random.next_usize()?
    let bounded = random.below_usize(17)?
    if bounded >= 17 { return 1 }
    var values: Vec<i32> = Vec [1, 2, 3, 4]
    random.shuffle(&+values as &+[i32])?
    if values[0] + values[1] + values[2] + values[3] != 10 { return 2 }
    return 0
}
",
    );
    execute_native_status(&image, &package_root.0, "cryptographic-randomness", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_random_choice_example_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        include_str!("../../../../../../examples/random-choice.nct"),
    );
    execute_native_status(&image, &package_root.0, "random-choice", 0);
}

#[test]
fn standard_random_contract_crosses_native_tests() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"random\", module: \"./random\" }\n");
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["random"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard random tests failed native compilation")
    };
    assert_eq!(cases.len(), 9);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}
