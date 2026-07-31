use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_owned_struct_fields_recursively_at_scope_end() {
    let project = TempProject::new("cli-run-recursive-struct-field-drop");
    write_process_exit_home(&project);
    let source = project.write_source(
        "recursive_struct_field_drop.nct",
        r#"use std/process.exit

struct Detail {
    code: i32
}

impl Detail {
    drop &+self {
        exit(self.code)
    }
}

struct Wrapper {
    detail: Detail
}

func main(): i32 {
    let wrapper = Wrapper { detail: Detail { code: 48 } }
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(48),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_i32_normal_call_with_borrow_argument_exit_code() {
    let project = TempProject::new("cli-run-borrow-normal-call");
    let source = project.write_source(
        "borrow_arg.nct",
        r#"func main(): i32 {
    let value = 7
    let result = choose(&value, 42)
    return result
}

func choose(value: &i32, code: i32): i32 {
    return code
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_i32_normal_call_with_readonly_temporary_borrow_argument_exit_code() {
    let project = TempProject::new("cli-run-readonly-temporary-borrow-normal-call");
    let source = project.write_source(
        "readonly_temporary_borrow_arg.nct",
        r#"func main(): i32 {
    return choose(&answer(), 42)
}

func answer(): i32 {
    return 7
}

func choose(value: &i32, code: i32): i32 {
    return code
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_i32_normal_call_with_readwrite_borrow_argument_exit_code() {
    let project = TempProject::new("cli-run-readwrite-borrow-normal-call");
    let source = project.write_source(
        "readwrite_borrow_arg.nct",
        r#"func main(): i32 {
    var value = 7
    let result = choose(&+value, 42)
    return result
}

func choose(value: &+i32, code: i32): i32 {
    return code
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_explicit_move_terminal_if_condition_exit_code() {
    let project = TempProject::new("cli-run-move-terminal-if-condition");
    let source = project.write_source(
        "move_terminal_if_condition.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func consume(file: File): bool {
    return file.fd == 42
}

func main(): i32 {
    var file = File { fd: 42 }
    if consume(move file) {
        return 42
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_runs_concrete_generic_scope_end_drop() {
    let project = TempProject::new("cli-run-concrete-generic-scope-end-drop");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "concrete_generic_scope_end_drop.nct",
        r#"use std/log.write

struct Box<T> {
    value: T
}

impl Box<i32> {
    drop &+self {
        write("drop\n")!
        return
    }
}

func main(): i32! {
    var box = Box<i32> { value: 42 }
    return box.value
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"drop\n");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_runs_replacement_and_scope_end_drops() {
    let project = TempProject::new("cli-run-replacement-drop");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "replacement_drop.nct",
        r#"use std/log.write

struct File {
    fd: i32
}

impl File {
    drop &+self {
        write("drop\n")!
        return
    }
}

func main(): i32! {
    var file = File { fd: 1 }
    file = File { fd: 42 }
    return file.fd
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"drop\ndrop\n");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_runs_reinitialization_after_explicit_drop() {
    let project = TempProject::new("cli-run-reinitialize-after-explicit-drop");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "reinitialize_after_explicit_drop.nct",
        r#"use std/log.write

struct File {
    fd: i32
}

impl File {
    drop &+self {
        write("drop\n")!
        return
    }
}

func main(): i32! {
    var file = File { fd: 1 }
    drop file
    file = File { fd: 42 }
    return file.fd
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"drop\ndrop\n");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_preserves_terminal_if_return_value_after_scope_drop() {
    let project = TempProject::new("cli-run-terminal-if-return-drop");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "terminal_if_return_drop.nct",
        r#"use std/log.write

struct File {
    fd: i32
}

impl File {
    drop &+self {
        write("drop\n")!
        return
    }
}

func main(): i32! {
    var file = File { fd: 42 }
    if true {
        return file.fd
    } else {
        return 7
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"drop\n");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_runs_outer_explicit_drop_before_nonterminal_if_return_once() {
    let project = TempProject::new("cli-run-nonterminal-if-outer-drop-return");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "nonterminal_if_outer_drop_return.nct",
        r#"use std/log.write

struct File {
    fd: i32
}

impl File {
    drop &+self {
        write("drop\n")!
        return
    }
}

func main(): i32! {
    var file = File { fd: 7 }
    if true {
        drop file
        return 7
    }
    return 0
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
    assert_eq!(output.stdout, b"drop\n");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_passes_scalar_parameter_borrow_argument() {
    let project = TempProject::new("cli-run-scalar-parameter-borrow-argument");
    let source = project.write_source(
        "scalar_parameter_borrow_argument.nct",
        r#"func main(): i32 {
    return caller(7)
}

func caller(value: i32): i32 {
    return choose(&value, 42)
}

func choose(value: &i32, code: i32): i32 {
    return code
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_passes_stack_scalar_parameter_borrow_argument() {
    let project = TempProject::new("cli-run-stack-scalar-parameter-borrow-argument");
    let source = project.write_source(
        "stack_scalar_parameter_borrow_argument.nct",
        r#"func main(): i32 {
    return caller(1, 2, 3, 4, 5, 6, 7, 8, 9)
}

func caller(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, value: i32): i32 {
    return choose(&value, 42)
}

func choose(value: &i32, code: i32): i32 {
    return code
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}
