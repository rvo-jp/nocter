use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn write_drop_log_home(project: &TempProject) {
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn assert_drop_program(name: &str, source_text: &str, expected_code: i32, expected_drops: usize) {
    let project = TempProject::new(name);
    write_drop_log_home(&project);
    let source = project.write_source("main.nct", source_text);
    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"drop\n".repeat(expected_drops));
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_outer_value_once_on_taken_and_untaken_if_paths() {
    let source = r#"use std/log.write

struct File {
    fd: i32
}

destruct File(&+self) {
    write("drop\n")!
    return
}

func main(): i32! {
    var taken = File { fd: 1 }
    if true {
        drop taken
    }

    var untaken = File { fd: 2 }
    if false {
        drop untaken
    }
    return 7
}
"#;
    assert_drop_program("cli-run-path-drop-if", source, 7, 2);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_transfers_outer_value_once_from_nonterminal_branch() {
    let source = r#"use std/log.write

struct File {
    fd: i32
}

destruct File(&+self) {
    write("drop\n")!
    return
}

func main(): i32! {
    var file = File { fd: 3 }
    if true {
        var moved = move file
    }
    return 8
}
"#;
    assert_drop_program("cli-run-path-move-if", source, 8, 1);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_transfers_outer_value_once_from_value_branch() {
    let source = r#"use std/log.write

struct File {
    fd: i32
}

destruct File(&+self) {
    write("drop\n")!
    return
}

func main(): i32! {
    var file = File { fd: 3 }
    let code = if true {
        var moved = move file
        moved.fd
    } else {
        0
    }
    return code
}
"#;
    assert_drop_program("cli-run-path-move-value-if", source, 3, 1);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_preserves_short_circuit_move_evaluation_state() {
    let source = r#"use std/log.write

struct File {
    fd: i32
}

destruct File(&+self) {
    write("drop\n")!
    return
}

func consume(file: File): bool {
    return true
}

func main(): i32! {
    var skipped = File { fd: 4 }
    let skipped_code = if false && consume(move skipped) {
        1
    } else {
        0
    }
    if skipped_code != 0 {
        return 1
    }

    var consumed = File { fd: 5 }
    let consumed_code = if true && consume(move consumed) {
        9
    } else {
        2
    }
    return consumed_code
}
"#;
    assert_drop_program("cli-run-path-move-short-circuit", source, 9, 2);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_outer_value_once_across_zero_iteration_loop_and_match() {
    let source = r#"use std/log.write

struct File {
    fd: i32
}

destruct File(&+self) {
    write("drop\n")!
    return
}

enum Choice {
    yes
    no
}

func main(): i32! {
    var loop_file = File { fd: 6 }
    while false {
        drop loop_file
    }

    var match_file = File { fd: 7 }
    match Choice.yes {
        Choice.yes {
            drop match_file
        }
        _ {
        }
    }
    return 10
}
"#;
    assert_drop_program("cli-run-path-drop-loop-match", source, 10, 2);
}
