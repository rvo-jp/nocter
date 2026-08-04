use super::*;

const CALLABLE: &str = r#"pub interface Call<Input, Output> {
    pub method &self.call(value: Input): Output
}

pub interface CallMut<Input, Output> {
    pub method &+self.call_mut(value: Input): Output
}

pub interface CallOnce<Input, Output> {
    pub method self.call_once(value: Input): Output
}
"#;

fn write_callable_home(project: &TempProject) {
    project.write_nocter_home_file("std/callable.nct", CALLABLE);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_invokes_zero_capture_closure() {
    let project = TempProject::new("cli-run-zero-capture-closure");
    write_callable_home(&project);
    let source = project.write_source(
        "zero_capture_closure.nct",
        r#"use std/callable.CallMut

func apply<F: CallMut<i32, i32>>(callback: F): i32 {
    var current = move callback
    return current.call_mut(3)
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
    write_callable_home(&project);
    let source = project.write_source(
        "borrow_capture_closure.nct",
        r#"use std/callable.CallMut

func apply<F: CallMut<i32, i32>>(callback: F): i32 {
    var current = move callback
    return current.call_mut(3)
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
    write_callable_home(&project);
    let source = project.write_source(
        "move_capture_closure.nct",
        r#"use std/callable.CallMut

func apply<F: CallMut<i32, i32>>(callback: F): i32 {
    var current = move callback
    return current.call_mut(3)
}

func main(): i32 {
    let factor = 4
    return apply((move factor; value) { value * factor })
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
    write_callable_home(&project);
    let source = project.write_source(
        "readwrite_capture_closure.nct",
        r#"use std/callable.CallMut

func apply<F: CallMut<i32, i32>>(callback: F): i32 {
    var current = move callback
    return current.call_mut(3)
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
