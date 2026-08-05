use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_invokes_builtin_callable_directly() {
    let project = TempProject::new("cli-run-builtin-callable");
    let source = project.write_source(
        "builtin_callable.nct",
        r#"func apply<F: &func(i32): i32>(callback: F): i32 {
    return callback(3)
}

func main(): i32 {
    return apply((value) { value * 2 })
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(6),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[test]
fn check_command_rejects_consuming_closure_for_repeated_callback_contract() {
    let project = TempProject::new("cli-check-consuming-callback-capability");
    let source = project.write_source(
        "consuming_callback_capability.nct",
        r#"struct Token {
    value: i32
}

impl Token {
    drop &+self {
        return
    }
}

func consume(token: Token): i32 {
    return token.value
}

func apply<F: &+func(i32): i32>(callback: F): i32 {
    var current = move callback
    return current(3)
}

func main(): i32 {
    let token = Token { value: 4 }
    return apply((move token; value) { consume(move token) + value })
}
"#,
    );

    let output = nocter(&project, ["check", source.to_str().unwrap()]);
    let stderr = text(&output.stderr);

    assert!(!output.status.success(), "stderr:\n{stderr}");
    assert!(stderr.contains("error[E0453]"), "stderr:\n{stderr}");
    assert!(
        stderr.contains("requires a consuming callback"),
        "stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("use a `func(...): ...` bound"),
        "stderr:\n{stderr}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_invokes_zero_capture_closure() {
    let project = TempProject::new("cli-run-zero-capture-closure");
    let source = project.write_source(
        "zero_capture_closure.nct",
        r#"func apply<F: &+func(i32): i32>(callback: F): i32 {
    var current = move callback
    return current(3)
}

func main(): i32 {
    return apply((value) { value * 2 })
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(6),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_preserves_borrow_capture_across_return_call() {
    let project = TempProject::new("cli-run-borrow-capture-closure");
    let source = project.write_source(
        "borrow_capture_closure.nct",
        r#"func apply<F: &+func(i32): i32>(callback: F): i32 {
    var current = move callback
    return current(3)
}

func main(): i32 {
    let factor = 4
    return apply((&factor; value) { value * factor })
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(12),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_invokes_move_capture_closure() {
    let project = TempProject::new("cli-run-move-capture-closure");
    let source = project.write_source(
        "move_capture_closure.nct",
        r#"func apply<F: &+func(i32): i32>(callback: F): i32 {
    var current = move callback
    return current(3)
}

func main(): i32 {
    let factor = 4
    let transform = (move factor; value: i32): i32 { value * factor }
    return apply(transform)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(12),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_mutates_readwrite_capture() {
    let project = TempProject::new("cli-run-readwrite-capture-closure");
    let source = project.write_source(
        "readwrite_capture_closure.nct",
        r#"func apply<F: &+func(i32): i32>(callback: F): i32 {
    var current = move callback
    return current(3)
}

func main(): i32 {
    var total = 4
    return apply((&+total; value) {
        total = total + value
        total
    })
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_loads_and_mutates_all_scalar_capture_kinds() {
    let project = TempProject::new("cli-run-scalar-capture-closure");
    let source = project.write_source(
        "scalar_capture_closure.nct",
        r#"func apply_byte<F: &+func(u8): u8>(callback: F): u8 {
    var current = move callback
    return current(3)
}

func apply_size<F: &+func(usize): usize>(callback: F): usize {
    var current = move callback
    return current(4)
}

func apply_flag<F: &+func(bool): bool>(callback: F): bool {
    var current = move callback
    return current(true)
}

func main(): i32 {
    var byte: u8 = 2
    var size: usize = 5
    var flag = false
    let new_byte = apply_byte((&+byte; value) {
        byte = byte + value
        byte
    })
    let new_size = apply_size((&+size; value) {
        size = size + value
        size
    })
    let new_flag = apply_flag((&+flag; value) {
        flag = value
        flag
    })
    if new_flag && new_byte == 5 && new_size == 9 {
        return 14
    }
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(14),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_moved_capture_exactly_with_closure_environment() {
    let project = TempProject::new("cli-run-owned-capture-drop");
    write_process_exit_home(&project);
    let source = project.write_source(
        "owned_capture_drop.nct",
        r#"use std/process.exit

struct Guard {
    code: i32
}

impl Guard {
    drop &+self {
        exit(self.code)
    }
}

func apply<F: &+func(i32): i32>(callback: F): i32 {
    var current = move callback
    return current(3)
}

func main(): i32 {
    let guard = Guard { code: 61 }
    let callback = (move guard; value: i32): i32 { value }
    return apply(move callback)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(61),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}
