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
    return 0
}
",
    );
    execute_native_status(&image, &package_root.0, "cryptographic-randomness", 0);
}
