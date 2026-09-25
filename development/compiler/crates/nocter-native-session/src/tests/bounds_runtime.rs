use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn checked_dynamic_index_proofs_cross_the_native_backend() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        r"
use std/vec.Vec

notrap func read_fixed(values: [i32; 2], index: usize): i32 {
    if index < 2 { return values[index] }
    return 0
}

notrap func read_view(values: &[i32], index: usize): i32 {
    if index < values.len() { return values[index] }
    return 0
}

func main(): i32 {
    let fixed_values: [i32; 2] = [7, 9]
    if read_fixed(fixed_values, 1) != 9 { return 1 }
    let view_values = Vec [7, 9]
    if read_view(&view_values, 0) != 7 { return 2 }
    return 0
}
",
    );
    execute_native_status(&image, &package_root.0, "checked-bounds-safe", 0);
}
