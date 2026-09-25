use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn checked_integer_arithmetic_executes_boundary_safe_values() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        r#"
func add_i8(left: i8, right: i8): i8 { return left + right }
func multiply_i16(left: i16, right: i16): i16 { return left * right }
func divide_i32(left: i32, right: i32): i32 { return left / right }
func shift_u32(value: u32, amount: u32): u32 { return value << amount }

func main(): i32 {
    if add_i8(100, 20) != 120 { return 1 }
    if multiply_i16(-100, 200) != -20000 { return 2 }
    if divide_i32(-20, 4) != -5 { return 3 }
    if shift_u32(1, 31) != 2147483648 { return 4 }
    return 0
}
"#,
    );
    execute_native_status(&image, &package_root.0, "checked-arithmetic-safe", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn checked_integer_overflow_traps_in_a_native_image() {
    assert_native_arithmetic_trap(
        "checked-add-overflow",
        "func add(left: i8, right: i8): i8 { return left + right }\nfunc main(): i32 { let _ = add(100, 100)\nreturn 0 }\n",
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn checked_integer_division_by_zero_traps_in_a_native_image() {
    assert_native_arithmetic_trap(
        "checked-divide-zero",
        "func divide(left: i32, right: i32): i32 { return left / right }\nfunc main(): i32 { let _ = divide(1, 0)\nreturn 0 }\n",
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn checked_integer_arithmetic_trap_matrix_reaches_native_traps() {
    for (name, source) in [
        (
            "checked-signed-add-overflow",
            "func calculate(left: i32, right: i32): i32 { return left + right }\nfunc main(): i32 { let _ = calculate(2147483647, 1)\nreturn 0 }\n",
        ),
        (
            "checked-unsigned-subtract-overflow",
            "func calculate(left: u32, right: u32): u32 { return left - right }\nfunc main(): i32 { let _ = calculate(0, 1)\nreturn 0 }\n",
        ),
        (
            "checked-signed-multiply-overflow",
            "func calculate(left: i64, right: i64): i64 { return left * right }\nfunc main(): i32 { let _ = calculate(9223372036854775807, 2)\nreturn 0 }\n",
        ),
        (
            "checked-unsigned-multiply-overflow",
            "func calculate(left: u64, right: u64): u64 { return left * right }\nfunc main(): i32 { let _ = calculate(18446744073709551615, 2)\nreturn 0 }\n",
        ),
        (
            "checked-negate-overflow",
            "func calculate(value: i64): i64 { return -value }\nfunc main(): i32 { let _ = calculate(-9223372036854775808)\nreturn 0 }\n",
        ),
        (
            "checked-divide-overflow",
            "func calculate(left: i64, right: i64): i64 { return left / right }\nfunc main(): i32 { let _ = calculate(-9223372036854775808, -1)\nreturn 0 }\n",
        ),
        (
            "checked-remainder-overflow",
            "func calculate(left: i64, right: i64): i64 { return left % right }\nfunc main(): i32 { let _ = calculate(-9223372036854775808, -1)\nreturn 0 }\n",
        ),
        (
            "checked-shift-count",
            "func calculate(value: u32, amount: u32): u32 { return value << amount }\nfunc main(): i32 { let _ = calculate(1, 32)\nreturn 0 }\n",
        ),
    ] {
        assert_native_arithmetic_trap(name, source);
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn assert_native_arithmetic_trap(name: &str, source: &str) {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Command;

    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(&package_root, &standard_root, source);
    let executable = package_root.0.join(name);
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let status = Command::new(&executable)
        .current_dir(&package_root.0)
        .status()
        .unwrap();
    // Darwin reports the compiler-emitted `brk` instruction as SIGTRAP (signal 5).
    assert_eq!(
        status.signal(),
        Some(5),
        "native image ended with {status:?}"
    );
}
