use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

#[test]
fn build_command_writes_default_macho_executable() {
    let project = TempProject::new("cli-build-default-output");
    let source = project.write_source(
        "app.nct",
        r#"func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_entry_option() {
    let project = TempProject::new("cli-build-entry");
    let source = project.write_source(
        "custom.nct",
        r#"func start(): i32 {
    return 0
}
"#,
    );

    let output = nocter(
        &project,
        ["build", source.to_str().unwrap(), "--entry", "start"],
    );
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_writes_configured_output_path() {
    let project = TempProject::new("cli-build-custom-output");
    let source = project.write_source(
        "custom_output.nct",
        r#"func main(): i32 {
    return 0
}
"#,
    );
    let executable = project.root().join("bin/app");
    fs::create_dir_all(executable.parent().unwrap()).unwrap();

    let output = nocter(
        &project,
        [
            "build",
            source.to_str().unwrap(),
            "-o",
            executable.to_str().unwrap(),
        ],
    );

    assert_success(&output);
    assert_macho_executable(&executable);
    assert!(
        !source.with_extension("").exists(),
        "default output path should not be written when -o is used"
    );
}

#[test]
fn build_command_accepts_target_option() {
    let project = TempProject::new("cli-build-target");
    let source = project.write_source(
        "target.nct",
        r#"func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter(
        &project,
        [
            "build",
            source.to_str().unwrap(),
            "--target",
            "arm64-darwin",
        ],
    );
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_rejects_unimplemented_reserved_target() {
    let project = TempProject::new("cli-build-unimplemented-target");
    let source = project.write_source(
        "target.nct",
        r#"func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter(
        &project,
        ["build", source.to_str().unwrap(), "--target", "x64-linux"],
    );

    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("target `x64-linux` is recognized but not implemented"),
        "expected unimplemented target error, got:\n{stderr}"
    );
}

#[test]
fn build_command_lowers_i32_let_binding_return() {
    let project = TempProject::new("cli-build-let-return");
    let source = project.write_source(
        "local.nct",
        r#"func main(): i32 {
    let value = 42
    return value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_i32_local_addition() {
    let project = TempProject::new("cli-build-local-add");
    let source = project.write_source(
        "local_add.nct",
        r#"func main(): i32 {
    let base = 40
    let result = base + 2
    return result
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_i32_call_multiplication() {
    let project = TempProject::new("cli-build-i32-call-multiply");
    let source = project.write_source(
        "i32_call_multiply.nct",
        r#"func main(): i32 {
    return answer() * 2
}

func answer(): i32 {
    return 21
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_i32_call_division_and_remainder() {
    let project = TempProject::new("cli-build-i32-call-div-rem");
    let source = project.write_source(
        "i32_call_div_rem.nct",
        r#"func main(): i32 {
    return total() / divisor() + dividend() % modulus()
}

func total(): i32 {
    return 60
}

func divisor(): i32 {
    return 2
}

func dividend(): i32 {
    return 25
}

func modulus(): i32 {
    return 13
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_i32_call_shifts() {
    let project = TempProject::new("cli-build-i32-call-shifts");
    let source = project.write_source(
        "i32_call_shifts.nct",
        r#"func main(): i32 {
    return (value() << left_count()) + (shifted() >> right_count())
}

func value(): i32 {
    return 5
}

func left_count(): i32 {
    return 3
}

func shifted(): i32 {
    return 8
}

func right_count(): i32 {
    return 1
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_i32_normal_call_let_initializer() {
    let project = TempProject::new("cli-build-normal-call-let");
    let source = project.write_source(
        "normal_call_let.nct",
        r#"func main(): i32 {
    let value = answer()
    return value
}

func answer(): i32 {
    return 42
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_usize_let_and_condition() {
    let project = TempProject::new("cli-build-usize-let-condition");
    let source = project.write_source(
        "usize_condition.nct",
        r#"func main(): i32 {
    let value: usize = size()
    if value >= 42 {
        return 0
    } else {
        return 1
    }
}

func size(): usize {
    return 42
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_usize_entry_return() {
    let project = TempProject::new("cli-build-usize-entry-return");
    let source = project.write_source(
        "usize_entry_return.nct",
        r#"func main(): usize {
    let value: usize = 23
    return value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_usize_terminal_if_function() {
    let project = TempProject::new("cli-build-usize-terminal-if-function");
    let source = project.write_source(
        "usize_terminal_if_function.nct",
        r#"func main(): i32 {
    let value: usize = choose(true)
    if value == 7 {
        return 0
    } else {
        return 1
    }
}

func choose(flag: bool): usize {
    if flag {
        return 7
    } else {
        return 9
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_u8_normal_and_tail_calls() {
    let project = TempProject::new("cli-build-u8-normal-tail-calls");
    let source = project.write_source(
        "u8_normal_tail_calls.nct",
        r#"func main(): i32 {
    let byte: u8 = forward(42)
    return byte as i32
}

func forward(byte: u8): u8 {
    return identity(byte)
}

func identity(byte: u8): u8 {
    return byte
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_void_terminal_if_function() {
    let project = TempProject::new("cli-build-void-terminal-if-function");
    let source = project.write_source(
        "void_terminal_if_function.nct",
        r#"func main(): i32 {
    run(true)
    return 0
}

func run(flag: bool): void {
    if flag {
        return
    } else {
        return
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_usize_arithmetic_and_shifts() {
    let project = TempProject::new("cli-build-usize-arithmetic-shifts");
    let source = project.write_source(
        "usize_arithmetic_shifts.nct",
        r#"func main(): i32 {
    if combined(20, size()) == 23 {
        return 42
    } else {
        return 1
    }
}

func combined(left: usize, right: usize): usize {
    return arithmetic(left, right) + shifted_left() + shifted_right()
}

func arithmetic(left: usize, right: usize): usize {
    let doubled: usize = right * 2
    let adjusted: usize = left + doubled - 4
    let quotient: usize = adjusted / 2
    let remainder: usize = quotient % 9
    return remainder
}

func shifted_left(): usize {
    return one() << left_count()
}

func shifted_right(): usize {
    return sixty_four() >> right_count()
}

func size(): usize {
    return 6
}

func one(): usize {
    return 1
}

func sixty_four(): usize {
    return 64
}

func left_count(): usize {
    return 4
}

func right_count(): usize {
    return 5
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_imported_usize_call_condition() {
    let project = TempProject::new("cli-build-imported-usize-condition");
    project.write_nocter_home_file(
        "std/sizes.nct",
        r#"pub func size(): usize {
    return 42
}
"#,
    );
    let source = project.write_source(
        "imported_usize_condition.nct",
        r#"use std/sizes.size

func main(): i32 {
    let value: usize = size()
    if value == 42 {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_if() {
    let project = TempProject::new("cli-build-terminal-if");
    let source = project.write_source(
        "terminal_if.nct",
        r#"func main(): i32 {
    if false {
        return 1
    } else {
        return 2
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_nested_terminal_if() {
    let project = TempProject::new("cli-build-nested-terminal-if");
    let source = project.write_source(
        "nested_terminal_if.nct",
        r#"func main(): i32 {
    if true {
        if false {
            return 1
        } else {
            return 0
        }
    } else {
        return 2
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_if_branch_drop() {
    let project = TempProject::new("cli-build-terminal-if-branch-drop");
    let source = project.write_source(
        "terminal_if_branch_drop.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    var file = File{ fd: 3 }
    if true {
        drop file
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_nonterminal_if_branch_scope_drop() {
    let project = TempProject::new("cli-build-nonterminal-if-branch-scope-drop");
    let source = project.write_source(
        "nonterminal_if_branch_scope_drop.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    if true {
        var file = File{ fd: 1 }
    } else {
        var file = File{ fd: 2 }
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_nonterminal_if_distinct_branch_aggregate_layouts() {
    let project = TempProject::new("cli-build-nonterminal-if-distinct-branch-layouts");
    let source = project.write_source(
        "nonterminal_if_distinct_branch_layouts.nct",
        r#"struct Small {
    value: i32
}

impl Small {
    drop small: &+Self {
        return
    }
}

struct Wide {
    left: i32
    right: i32
}

impl Wide {
    drop wide: &+Self {
        return
    }
}

func main(): i32 {
    if true {
        var small = Small{ value: 1 }
    } else {
        var wide = Wide{ left: 2, right: 3 }
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_nonterminal_outer_scalar_assignments() {
    let project = TempProject::new("cli-build-nonterminal-outer-scalar-assignments");
    let source = project.write_source(
        "nonterminal_outer_scalar_assignments.nct",
        r#"func main(): i32 {
    var value = 1
    if true {
        value = 2
    }
    while false {
        value = 3
    }
    return value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_nonterminal_while_body_scope_drop() {
    let project = TempProject::new("cli-build-nonterminal-while-body-scope-drop");
    let source = project.write_source(
        "nonterminal_while_body_scope_drop.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    while false {
        var file = File{ fd: 1 }
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_nonterminal_while_body_explicit_drop() {
    let project = TempProject::new("cli-build-nonterminal-while-body-explicit-drop");
    let source = project.write_source(
        "nonterminal_while_body_explicit_drop.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    while false {
        var file = File{ fd: 1 }
        drop file
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_nonterminal_while_body_local_aggregate_replacement() {
    let project = TempProject::new("cli-build-nonterminal-while-body-local-aggregate-replacement");
    let source = project.write_source(
        "nonterminal_while_body_local_aggregate_replacement.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    while false {
        var file = File{ fd: 1 }
        file = File{ fd: 2 }
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_nonterminal_while_break_cleanup() {
    let project = TempProject::new("cli-build-nonterminal-while-break-cleanup");
    let source = project.write_source(
        "nonterminal_while_break_cleanup.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    while false {
        var file = File{ fd: 1 }
        break
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_nonterminal_while_continue_cleanup() {
    let project = TempProject::new("cli-build-nonterminal-while-continue-cleanup");
    let source = project.write_source(
        "nonterminal_while_continue_cleanup.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    while false {
        var file = File{ fd: 1 }
        continue
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_nested_if_in_nonterminal_while_body() {
    let project = TempProject::new("cli-build-terminal-nested-if-in-nonterminal-while-body");
    let source = project.write_source(
        "terminal_nested_if_in_nonterminal_while_body.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    while false {
        var file = File{ fd: 1 }
        if true {
            break
        } else {
            continue
        }
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_nonterminal_loop_break_cleanup() {
    let project = TempProject::new("cli-build-nonterminal-loop-break-cleanup");
    let source = project.write_source(
        "nonterminal_loop_break_cleanup.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    loop {
        var file = File{ fd: 1 }
        break
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_nested_if_in_nonterminal_loop_body() {
    let project = TempProject::new("cli-build-terminal-nested-if-in-nonterminal-loop-body");
    let source = project.write_source(
        "terminal_nested_if_in_nonterminal_loop_body.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    loop {
        var file = File{ fd: 1 }
        if true {
            break
        } else {
            continue
        }
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_loop_body_return_cleanup() {
    let project = TempProject::new("cli-build-terminal-loop-body-return-cleanup");
    let source = project.write_source(
        "terminal_loop_body_return_cleanup.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    loop {
        var file = File{ fd: 1 }
        return 7
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_return_in_nonterminal_while_body() {
    let project = TempProject::new("cli-build-return-in-nonterminal-while-body");
    let source = project.write_source(
        "return_in_nonterminal_while_body.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    while false {
        var file = File{ fd: 1 }
        return 7
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_if_branch_void_call() {
    let project = TempProject::new("cli-build-terminal-if-branch-void-call");
    let source = project.write_source(
        "terminal_if_branch_void_call.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    var file = File{ fd: 3 }
    if true {
        touch(&+file)
        return 0
    } else {
        return 1
    }
}

func touch(file: &+File): void {
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_direct_aggregate_terminal_if_return() {
    let project = TempProject::new("cli-build-direct-aggregate-terminal-if-return");
    let source = project.write_source(
        "direct_aggregate_terminal_if_return.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

struct Pair {
    first: i32
    second: i32
}

func main(): i32 {
    let pair = choose(true)
    return pair.first
}

func choose(flag: bool): Pair {
    var file = File{ fd: 3 }
    if flag {
        return Pair{ first: 42, second: 1 }
    } else {
        return Pair{ first: 7, second: 2 }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_direct_aggregate_terminal_if_call_return() {
    let project = TempProject::new("cli-build-direct-aggregate-terminal-if-call-return");
    let source = project.write_source(
        "direct_aggregate_terminal_if_call_return.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

struct Pair {
    first: i32
    second: i32
}

func main(): i32 {
    let pair = choose(true)
    return pair.first
}

func make_pair(first: i32, second: i32): Pair {
    return Pair{ first: first, second: second }
}

func choose(flag: bool): Pair {
    var file = File{ fd: 3 }
    if flag {
        return make_pair(42, 1)
    } else {
        return make_pair(7, 2)
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_direct_aggregate_terminal_if_branch_leading_statements() {
    let project =
        TempProject::new("cli-build-direct-aggregate-terminal-if-branch-leading-statements");
    let source = project.write_source(
        "direct_aggregate_terminal_if_branch_leading_statements.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

struct Pair {
    first: i32
    second: i32
}

func main(): i32 {
    let pair = choose(true)
    return pair.first
}

func choose(flag: bool): Pair {
    var file = File{ fd: 3 }
    if flag {
        drop file
        return Pair{ first: 42, second: 1 }
    } else {
        touch(&+file)
        return Pair{ first: 7, second: 2 }
    }
}

func touch(file: &+File): void {
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_direct_aggregate_terminal_if_branch_local_binding() {
    let project = TempProject::new("cli-build-direct-aggregate-terminal-if-branch-local-binding");
    let source = project.write_source(
        "direct_aggregate_terminal_if_branch_local_binding.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

struct Pair {
    first: i32
    second: i32
}

func main(): i32 {
    let pair = choose(true)
    return pair.first
}

func choose(flag: bool): Pair {
    if flag {
        var file = File{ fd: 1 }
        return Pair{ first: 42, second: 1 }
    } else {
        var file = File{ fd: 2 }
        return Pair{ first: 7, second: 2 }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_direct_aggregate_terminal_if_branch_assignment() {
    let project = TempProject::new("cli-build-direct-aggregate-terminal-if-branch-assignment");
    let source = project.write_source(
        "direct_aggregate_terminal_if_branch_assignment.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    var file = choose(true)
    drop file
    return 0
}

func choose(flag: bool): File {
    var file = File{ fd: 1 }
    if flag {
        file = File{ fd: 2 }
        return move file
    } else {
        file = File{ fd: 3 }
        return move file
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_if_bool_local() {
    let project = TempProject::new("cli-build-terminal-if-bool-local");
    let source = project.write_source(
        "terminal_if_bool_local.nct",
        r#"func main(): i32 {
    let enabled = true
    if enabled {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_if_bool_logical() {
    let project = TempProject::new("cli-build-terminal-if-bool-logical");
    let source = project.write_source(
        "terminal_if_bool_logical.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    if ready && !blocked {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_bool_equality() {
    let project = TempProject::new("cli-build-bool-equality");
    let source = project.write_source(
        "bool_equality.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    let same = ready == blocked
    if same {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_compound_bool_equality() {
    let project = TempProject::new("cli-build-compound-bool-equality");
    let source = project.write_source(
        "compound_bool_equality.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    let same = !ready == blocked
    if same {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_function_leading_compound_bool_equality() {
    let project = TempProject::new("cli-build-function-leading-binding-span");
    let source = project.write_source(
        "function_leading_binding_span.nct",
        r#"func main(): i32 {
    return helper()
}

func helper(): i32 {
    let ready = true
    let blocked = false
    let same = !ready == blocked
    if same {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_compound_bool_equality_in_nonterminal_if_binding() {
    let project = TempProject::new("cli-build-nonterminal-if-binding-span");
    let source = project.write_source(
        "nonterminal_if_binding_span.nct",
        r#"func main(): i32 {
    let ready = true
    if ready {
        let ok = true
        let same = !ok == ready
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_compound_bool_equality_in_nonterminal_while_binding() {
    let project = TempProject::new("cli-build-nonterminal-while-binding-span");
    let source = project.write_source(
        "nonterminal_while_binding_span.nct",
        r#"func main(): i32 {
    let ready = true
    while ready {
        let ok = true
        let same = !ok == ready
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_compound_bool_equality_in_terminal_if_branch_binding() {
    let project = TempProject::new("cli-build-terminal-if-branch-binding-span");
    let source = project.write_source(
        "terminal_if_branch_binding_span.nct",
        r#"func main(): i32 {
    let ready = true
    if ready {
        let ok = true
        let same = !ok == ready
        return 1
    } else {
        return 0
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_compound_bool_equality_in_terminal_aggregate_branch_binding() {
    let project = TempProject::new("cli-build-terminal-aggregate-branch-binding-span");
    let source = project.write_source(
        "terminal_aggregate_branch_binding_span.nct",
        r#"func main(): i32 {
    return make(true).len
}

struct Text {
    start: i32
    len: i32
    capacity: i32
}

func make(flag: bool): Text {
    if flag {
        let ok = true
        let same = !ok == flag
        return Text{ start: 1, len: 42, capacity: 99 }
    } else {
        return Text{ start: 2, len: 7, capacity: 11 }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_compound_bool_equality_condition() {
    let project = TempProject::new("cli-build-compound-bool-equality-condition");
    let source = project.write_source(
        "compound_bool_equality_condition.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    if !ready == blocked {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_reports_compound_bool_equality_nested_call_before_ir_lowering() {
    let project = TempProject::new("cli-build-compound-bool-equality-call-boundary");
    let source = project.write_source(
        "compound_bool_equality_call_boundary.nct",
        r#"func main(): i32 {
    if (ready() && other()) == true {
        return 0
    } else {
        return 1
    }
}

func ready(): bool {
    return true
}

func other(): bool {
    return true
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("compound bool equality operands with nested calls"),
        "expected compound bool equality diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("2 |     if (ready() && other()) == true {"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E8006]"),
        "buildability preflight should reject before IR bool lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_lowers_compound_bool_equality_in_nonterminal_while_condition() {
    let project = TempProject::new("cli-build-nonterminal-while-condition-span");
    let source = project.write_source(
        "nonterminal_while_condition_span.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    while !ready == blocked {
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_imported_i32_call() {
    let project = TempProject::new("cli-build-imported-call");
    project.write_nocter_home_file(
        "std/math.nct",
        r#"pub func answer(): i32 {
    return 42
}
"#,
    );
    let source = project.write_source(
        "imported_call.nct",
        r#"use std/math.answer

func main(): i32 {
    let value = answer()
    return value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[test]
fn build_command_lowers_imported_alias_i32_call() {
    let project = TempProject::new("cli-build-imported-alias-call");
    project.write_nocter_home_file(
        "std/math.nct",
        r#"pub func answer(): i32 {
    return 42
}
"#,
    );
    let source = project.write_source(
        "imported_alias_call.nct",
        r#"use std/math.answer as imported_answer

func main(): i32 {
    return imported_answer()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[test]
fn build_command_lowers_alias_parameter_and_return_abi() {
    let project = TempProject::new("cli-build-alias-parameter-return-abi");
    let source = project.write_source(
        "alias_parameter_return_abi.nct",
        r#"type Exit = i32
type Text = str
type Bytes = [u8]

func main(): i32 {
    return 0
}

func answer(name: &Text, code: Exit): Exit {
    return code
}

func echo(bytes: &+Bytes): &+Bytes {
    return bytes
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(0));
}

#[test]
fn build_command_lowers_imported_bool_condition() {
    let project = TempProject::new("cli-build-imported-bool-condition");
    project.write_nocter_home_file(
        "std/flags.nct",
        r#"pub func ready(): bool {
    return true
}
"#,
    );
    let source = project.write_source(
        "imported_bool_condition.nct",
        r#"use std/flags.ready

func main(): i32 {
    if ready() {
        return 42
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[test]
fn build_command_lowers_imported_nested_argument() {
    let project = TempProject::new("cli-build-imported-nested-argument");
    project.write_nocter_home_file(
        "std/math.nct",
        r#"pub func base(): i32 {
    return 41
}

pub func add_one(value: i32): i32 {
    return value + 1
}
"#,
    );
    let source = project.write_source(
        "imported_nested_argument.nct",
        r#"use std/math.add_one
use std/math.base

func main(): i32 {
    return add_one(base())
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[test]
fn build_command_lowers_fallible_aggregate_binding_and_borrow_argument() {
    let project = TempProject::new("cli-build-fallible-aggregate-binding-borrow");
    let source = project.write_source(
        "aggregate_binding_borrow.nct",
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32! {
    var value = make()?
    touch(&+value)?
    return 0
}

func make(): Text! {
    return Text{ start: 1, len: 2, capacity: 3 }
}

func touch(value: &+Text): void! {
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_reports_non_binding_borrow_argument_before_ir_lowering() {
    let project = TempProject::new("cli-build-non-binding-borrow-argument-boundary");
    let source = project.write_source(
        "non_binding_borrow_argument_boundary.nct",
        r#"type IntRef = &i32

copy struct Pair {
    value: i32
}

func main(): i32 {
    let pair = Pair{ value: 1 }
    return choose(&pair.value, 0)
}

func choose(value: IntRef, fallback: i32): i32 {
    return fallback
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("borrow call arguments from non-binding expressions"),
        "expected borrow argument diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("9 |     return choose(&pair.value, 0)"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E8006]"),
        "buildability preflight should reject before IR call argument lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_lowers_u16_u32_aggregate_scalar_fields() {
    let project = TempProject::new("cli-build-u16-u32-aggregate-scalar-fields");
    let source = project.write_source(
        "u16_u32_aggregate_scalar_fields.nct",
        r#"struct Header {
    tag: u8
    code: u16
    wide: u32
}

func main(): i32 {
    let header = make()
    return 0
}

func make(): Header {
    return Header{ tag: 7, code: 42, wide: 100 }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_aggregate_struct_literal_binding_and_borrow_argument() {
    let project = TempProject::new("cli-build-aggregate-literal-binding-borrow");
    let source = project.write_source(
        "aggregate_literal_binding_borrow.nct",
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32! {
    var value = Text{ start: 1, len: 2, capacity: 3 }
    touch(&+value)?
    return 0
}

func touch(value: &+Text): void! {
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_aggregate_struct_literal_assignment_and_borrow_argument() {
    let project = TempProject::new("cli-build-aggregate-literal-assignment-borrow");
    let source = project.write_source(
        "aggregate_literal_assignment_borrow.nct",
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32! {
    var value = Text{ start: 1, len: 2, capacity: 3 }
    value = Text{ start: 4, len: 5, capacity: 6 }
    touch(&+value)?
    return 0
}

func touch(value: &+Text): void! {
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_moved_aggregate_slot_assignment() {
    let project = TempProject::new("cli-build-moved-aggregate-slot-assignment");
    let source = project.write_source(
        "moved_aggregate_slot_assignment.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    var source = File{ fd: 7 }
    var target = File{ fd: 1 }
    target = move source
    return target.fd
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_moved_aggregate_binding() {
    let project = TempProject::new("cli-build-moved-aggregate-binding");
    let source = project.write_source(
        "moved_aggregate_binding.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    var source = File{ fd: 7 }
    var target = move source
    return target.fd
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_copy_aggregate_binding() {
    let project = TempProject::new("cli-build-copy-aggregate-binding");
    let source = project.write_source(
        "copy_aggregate_binding.nct",
        r#"copy struct Pair {
    left: i32
    right: i32
}

func main(): i32 {
    let source = Pair{ left: 40, right: 2 }
    let target = source
    return target.left + target.right
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_copy_aggregate_field_from_non_copy_owner() {
    let project = TempProject::new("cli-build-copy-aggregate-field-non-copy-owner");
    let source = project.write_source(
        "copy_aggregate_field_non_copy_owner.nct",
        r#"copy struct Header {
    code: i32
    len: i32
}

struct Packet {
    prefix: i32
    header: Header
    tail: i32
}

func main(): i32 {
    let packet = Packet{ prefix: 1, header: Header{ code: 40, len: 2 }, tail: 3 }
    let header = packet.header
    return header.code + header.len
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_copy_aggregate_field_from_non_copy_call_result() {
    let project = TempProject::new("cli-build-copy-aggregate-field-non-copy-call-result");
    let source = project.write_source(
        "copy_aggregate_field_non_copy_call_result.nct",
        r#"copy struct Header {
    code: i32
    len: i32
}

struct Packet {
    prefix: i32
    header: Header
    tail: i32
}

func make_packet(): Packet {
    return Packet{ prefix: 1, header: Header{ code: 40, len: 2 }, tail: 3 }
}

func main(): i32 {
    let header = make_packet().header
    let again = header
    return again.code + again.len
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_moved_aggregate_struct_literal_field() {
    let project = TempProject::new("cli-build-moved-aggregate-struct-literal-field");
    let source = project.write_source(
        "moved_aggregate_struct_literal_field.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

struct Holder {
    file: File
}

impl Holder {
    drop holder: &+Self {
        return
    }
}

func main(): i32 {
    var file = File{ fd: 7 }
    var holder = Holder{ file: move file }
    return holder.file.fd
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_copy_aggregate_slot_assignment_and_borrow_argument() {
    let project = TempProject::new("cli-build-copy-aggregate-slot-assignment-borrow");
    let source = project.write_source(
        "copy_aggregate_slot_assignment_borrow.nct",
        r#"copy struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32! {
    var source = Text{ start: 1, len: 2, capacity: 3 }
    var target = Text{ start: 4, len: 5, capacity: 6 }
    target = source
    touch(&+target)?
    return 0
}

func touch(value: &+Text): void! {
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_imported_copy_aggregate_slot_assignment_and_borrow_argument() {
    let project = TempProject::new("cli-build-imported-copy-aggregate-slot-assignment-borrow");
    project.write_nocter_home_file(
        "std/text.nct",
        r#"pub copy struct Text {
    pub start: usize
    pub len: usize
    pub capacity: usize
}
"#,
    );
    let source = project.write_source(
        "imported_copy_aggregate_slot_assignment_borrow.nct",
        r#"use std/text.Text

func main(): i32! {
    var source = Text{ start: 1, len: 2, capacity: 3 }
    var target = Text{ start: 4, len: 5, capacity: 6 }
    target = source
    touch(&+target)?
    return 0
}

func touch(value: &+Text): void! {
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_direct_aggregate_call_assignment_and_borrow_argument() {
    let project = TempProject::new("cli-build-direct-aggregate-call-assignment-borrow");
    let source = project.write_source(
        "direct_aggregate_call_assignment_borrow.nct",
        r#"struct Allocator {
    state: usize
    kind: u64
}

func main(): i32 {
    var allocator = page_allocator()
    allocator = reset_allocator()
    touch(&+allocator)
    return 0
}

func page_allocator(): Allocator {
    return Allocator{ state: 0, kind: 0 }
}

func reset_allocator(): Allocator {
    return Allocator{ state: 1, kind: 2 }
}

func touch(allocator: &+Allocator): void {
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fallible_direct_aggregate_call_binding_and_borrow_argument() {
    let project = TempProject::new("cli-build-fallible-direct-aggregate-binding-borrow");
    let source = project.write_source(
        "fallible_direct_aggregate_binding_borrow.nct",
        r#"struct Allocator {
    state: usize
    kind: u64
}

func main(): i32! {
    var allocator = page_allocator()?
    touch(&+allocator)?
    return 0
}

func page_allocator(): Allocator! {
    return Allocator{ state: 0, kind: 0 }
}

func touch(allocator: &+Allocator): void! {
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fallible_direct_aggregate_call_assignment_and_borrow_argument() {
    let project = TempProject::new("cli-build-fallible-direct-aggregate-assignment-borrow");
    let source = project.write_source(
        "fallible_direct_aggregate_assignment_borrow.nct",
        r#"struct Allocator {
    state: usize
    kind: u64
}

func main(): i32! {
    var allocator = page_allocator()
    allocator = reset_allocator()?
    touch(&+allocator)?
    return 0
}

func page_allocator(): Allocator {
    return Allocator{ state: 0, kind: 0 }
}

func reset_allocator(): Allocator! {
    return Allocator{ state: 1, kind: 2 }
}

func touch(allocator: &+Allocator): void! {
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_indirect_aggregate_call_assignment_and_borrow_argument() {
    let project = TempProject::new("cli-build-indirect-aggregate-call-assignment-borrow");
    let source = project.write_source(
        "indirect_aggregate_call_assignment_borrow.nct",
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    var value = Text{ start: 1, len: 2, capacity: 3 }
    value = make()
    touch(&+value)
    return 0
}

func make(): Text {
    return Text{ start: 4, len: 5, capacity: 6 }
}

func touch(value: &+Text): void {
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_propagated_indirect_aggregate_call_assignment_and_borrow_argument() {
    let project = TempProject::new("cli-build-propagated-aggregate-call-assignment-borrow");
    let source = project.write_source(
        "propagated_aggregate_call_assignment_borrow.nct",
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32! {
    var value = Text{ start: 1, len: 2, capacity: 3 }
    value = make()?
    touch(&+value)?
    return 0
}

func make(): Text! {
    return Text{ start: 4, len: 5, capacity: 6 }
}

func touch(value: &+Text): void! {
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_std_page_allocator_direct_aggregate_binding_and_borrow_argument() {
    let project = TempProject::new("cli-build-page-allocator-borrow");
    project.write_nocter_home_file(
        "std/mem.nct",
        r#"pub struct Allocator {
    state: usize
    kind: u64
}

pub func page_allocator(): Allocator {
    return Allocator{ state: 0, kind: 0 }
}
"#,
    );
    let source = project.write_source(
        "page_allocator_borrow.nct",
        r#"use std/mem.Allocator
use std/mem.page_allocator

func main(): i32 {
    var allocator = page_allocator()
    touch(&+allocator)
    return 0
}

func touch(allocator: &+Allocator): void {
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_if_i32_equality() {
    let project = TempProject::new("cli-build-terminal-if-equality");
    let source = project.write_source(
        "terminal_if_equality.nct",
        r#"func main(): i32 {
    let value = 42
    if value == 42 {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_if_i32_inequality() {
    let project = TempProject::new("cli-build-terminal-if-inequality");
    let source = project.write_source(
        "terminal_if_inequality.nct",
        r#"func main(): i32 {
    let value = 42
    if value != 41 {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_if_i32_less_equal() {
    let project = TempProject::new("cli-build-terminal-if-less-equal");
    let source = project.write_source(
        "terminal_if_less_equal.nct",
        r#"func main(): i32 {
    let value = 42
    if value <= 42 {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_reports_compile_diagnostics_without_output() {
    let project = TempProject::new("cli-build-diagnostics");
    let source = project.write_source(
        "bad.nct",
        r#"func main(): i32 {
    return "bad"
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0312]"),
        "expected return type diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("2 |     return \"bad\""),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        stderr.contains("  |            ^^^^^"),
        "expected source underline, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after compile diagnostics"
    );
}

#[test]
fn build_command_reports_value_expression_statement_before_ir_lowering() {
    let project = TempProject::new("cli-build-value-expression-statement-boundary");
    let source = project.write_source(
        "value_expression_statement_boundary.nct",
        r#"func value(): i32 {
    return 1
}

func main(): void {
    value()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("value-producing expression statements"),
        "expected value expression statement diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("6 |     value()"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E8002]"),
        "buildability preflight should reject before IR entry body lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_self_move_assignment_before_ir_lowering() {
    let project = TempProject::new("cli-build-self-move-assignment");
    let source = project.write_source(
        "self_move_assignment.nct",
        r#"struct File {
    fd: i32
}

func main(): i32 {
    var file = File{ fd: 1 }
    file = move file
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0395]"),
        "expected self-move assignment diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("7 |     file = move file"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E8008]"),
        "self-move assignment should be rejected before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after compile diagnostics"
    );
}

#[test]
fn build_command_reports_explicit_move_in_condition_before_ir_lowering() {
    let project = TempProject::new("cli-build-move-in-condition-boundary");
    let source = project.write_source(
        "move_in_condition_boundary.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func consume(file: File): bool {
    return true
}

func main(): i32 {
    var file = File{ fd: 1 }
    if consume(move file) {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("explicit aggregate moves in control-flow conditions"),
        "expected move condition diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("17 |     if consume(move file) {"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E8002]"),
        "buildability preflight should reject before IR control-flow lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_lowers_reachable_i32_range_for() {
    let project = TempProject::new("cli-build-range-for");
    let source = project.write_source(
        "range_for.nct",
        r#"func main(): i32 {
    return helper()
}

func helper(): i32 {
    var total = 0
    for value in 0..<4 {
        total = total + value
    }

    return total
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(6));
}

#[test]
fn build_command_does_not_reject_unreachable_range_for_body() {
    let project = TempProject::new("cli-build-unreachable-range-for");
    let source = project.write_source(
        "unreachable_range_for.nct",
        r#"func main(): i32 {
    return 0
}

func unused(): i32 {
    for value in 0..<4 {
        return value
    }

    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_loaded_imported_i32_range_for() {
    let project = TempProject::new("cli-build-imported-range-for");
    project.write_nocter_home_file(
        "std/loops.nct",
        r#"pub func helper(): i32 {
    var total = 0
    for value in 0..<4 {
        total = total + value
    }

    return total
}
"#,
    );
    let source = project.write_source(
        "imported_range_for.nct",
        r#"use std/loops.helper

func main(): i32 {
    return helper()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(6));
}

#[test]
fn build_command_reports_unsupported_u64_range_for_before_ir_lowering() {
    let project = TempProject::new("cli-build-u64-range-for-boundary");
    let source = project.write_source(
        "u64_range_for_boundary.nct",
        r#"func main(): i32 {
    return helper(4)
}

func helper(limit: u64): i32 {
    for value in 0..<limit {
        return 1
    }

    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("range `for` loops outside i32/usize bounds"),
        "expected range for diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("6 |     for value in 0..<limit {"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_match_before_ir_lowering() {
    let project = TempProject::new("cli-build-match-boundary");
    let source = project.write_source(
        "match_boundary.nct",
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    return describe(Choice.yes)
}

func describe(choice: Choice): i32 {
    match choice {
        Choice.yes {
            return 0
        }

        else {
            return 1
        }
    }

    return 2
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("`match` statements"),
        "expected match diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("11 |     match choice {"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_reachable_generic_function_before_ir_lowering() {
    let project = TempProject::new("cli-build-generic-function-boundary");
    let source = project.write_source(
        "generic_function_boundary.nct",
        r#"func main(): i32 {
    return identity(42)
}

func identity<T>(value: T): T {
    return value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("generic functions"),
        "expected generic function diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("5 | func identity<T>(value: T): T {"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_does_not_reject_unreachable_generic_function() {
    let project = TempProject::new("cli-build-unreachable-generic-function");
    let source = project.write_source(
        "unreachable_generic_function.nct",
        r#"func main(): i32 {
    return 0
}

func identity<T>(value: T): T {
    return value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_reports_reachable_nested_fallible_return_before_ir_lowering() {
    let project = TempProject::new("cli-build-nested-fallible-return-boundary");
    let source = project.write_source(
        "nested_fallible_return_boundary.nct",
        r#"func main(): i32 {
    return consume(make_value()!)
}

func consume(item: i32?): i32 {
    return 0
}

func make_value(): (i32?)! {
    return none
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("nested fallible or optional return types"),
        "expected nested fallible return diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("9 | func make_value(): (i32?)! {"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E8007]"),
        "buildability preflight should reject before IR function lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_does_not_reject_unreachable_nested_fallible_return() {
    let project = TempProject::new("cli-build-unreachable-nested-fallible-return");
    let source = project.write_source(
        "unreachable_nested_fallible_return.nct",
        r#"func main(): i32 {
    return 0
}

func value(): (i32?)! {
    return none
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_reports_reachable_generic_impl_method_before_ir_lowering() {
    let project = TempProject::new("cli-build-generic-impl-method-boundary");
    let source = project.write_source(
        "generic_impl_method_boundary.nct",
        r#"struct Box<T> {
    value: T
}

impl<U> Box<U> {
    method (box: Self).value(): U {
        return box.value
    }
}

func main(): i32 {
    let box = Box<i32>{
        value: 42,
    }
    return box.value()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("generic impl members"),
        "expected generic impl diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("5 | impl<U> Box<U> {"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_reachable_array_literal_before_ir_lowering() {
    let project = TempProject::new("cli-build-array-literal-boundary");
    let source = project.write_source(
        "array_literal_boundary.nct",
        r#"func main(): i32 {
    let header: [u8; 2] = [1, 2]
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("array literals"),
        "expected array literal diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("2 |     let header: [u8; 2] = [1, 2]"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_does_not_reject_unreachable_array_literal() {
    let project = TempProject::new("cli-build-unreachable-array-literal");
    let source = project.write_source(
        "unreachable_array_literal.nct",
        r#"func main(): i32 {
    return 0
}

func header(): [u8; 2] {
    return [1, 2]
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_reports_dynamic_failure_payload_before_ir_lowering() {
    let project = TempProject::new("cli-build-dynamic-failure-payload-boundary");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "dynamic_failure_payload_boundary.nct",
        r#"use std/error.Error

func main(): i32! {
    return Error.new("app.failed", dynamic())
}

func dynamic(): &str {
    return "failed"
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("dynamic failure payload arguments"),
        "expected dynamic failure payload diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("4 |     return Error.new(\"app.failed\", dynamic())"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_does_not_reject_unreachable_dynamic_failure_payload() {
    let project = TempProject::new("cli-build-unreachable-dynamic-failure-payload");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "unreachable_dynamic_failure_payload.nct",
        r#"use std/error.Error

func main(): i32 {
    return 0
}

func unused(): i32! {
    return Error.new("app.failed", dynamic())
}

func dynamic(): &str {
    return "failed"
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn check_command_reports_source_snippet_for_compile_diagnostic() {
    let project = TempProject::new("cli-check-source-diagnostic");
    let source = project.write_source(
        "bad.nct",
        r#"func main(): i32 {
    return "bad"
}
"#,
    );

    let output = nocter(&project, ["check", source.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0312]"),
        "expected return type diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("2 |     return \"bad\""),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        stderr.contains("  |            ^^^^^"),
        "expected source underline, got:\n{stderr}"
    );
}

#[test]
fn check_command_reports_source_snippet_for_diagnostic_notes() {
    let project = TempProject::new("cli-check-source-note-diagnostic");
    let source = project.write_source(
        "bad_argument.nct",
        r#"func callee(value: i32): i32 {
    return value
}

func main(): i32 {
    return callee("bad")
}
"#,
    );

    let output = nocter(&project, ["check", source.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0321]"),
        "expected argument type diagnostic, got:\n{stderr}"
    );
    let lines: Vec<&str> = stderr.lines().collect();
    let primary_line = lines
        .iter()
        .position(|line| line.contains("6 |     return callee(\"bad\")"))
        .unwrap_or_else(|| panic!("expected primary source line, got:\n{stderr}"));
    assert!(
        lines
            .get(primary_line + 1)
            .is_some_and(|line| line.contains("^^^^^")),
        "expected primary source underline, got:\n{stderr}"
    );
    assert!(
        stderr.contains("note:") && stderr.contains("parameter `value` is declared here"),
        "expected parameter note, got:\n{stderr}"
    );
    let note_line = lines
        .iter()
        .position(|line| line.contains("1 | func callee(value: i32): i32 {"))
        .unwrap_or_else(|| panic!("expected note source line, got:\n{stderr}"));
    assert!(
        lines
            .get(note_line + 1)
            .is_some_and(|line| line.contains("^^^")),
        "expected note source underline, got:\n{stderr}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_entry_exit_code() {
    let project = TempProject::new("cli-build-run-exit");
    let source = project.write_source(
        "exit37.nct",
        r#"func main(): i32 {
    return 37
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(37));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_runs_same_file_i32_call_with_arguments() {
    let project = TempProject::new("cli-build-run-call-args");
    let source = project.write_source(
        "add.nct",
        r#"func main(): i32 {
    return add(20, 22)
}

func add(a: i32, b: i32): i32 {
    return a + b
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_i32_let_binding_value() {
    let project = TempProject::new("cli-build-run-let-return");
    let source = project.write_source(
        "local_exit.nct",
        r#"func main(): i32 {
    let value = 42
    return value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_i32_local_addition_value() {
    let project = TempProject::new("cli-build-run-local-add");
    let source = project.write_source(
        "local_add_exit.nct",
        r#"func main(): i32 {
    let base = 40
    let result = base + 2
    return result
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_else_value() {
    let project = TempProject::new("cli-build-run-terminal-if");
    let source = project.write_source(
        "terminal_if_exit.nct",
        r#"func main(): i32 {
    if false {
        return 1
    } else {
        return 42
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_equality_value() {
    let project = TempProject::new("cli-build-run-terminal-if-equality");
    let source = project.write_source(
        "terminal_if_equality_exit.nct",
        r#"func main(): i32 {
    let value = 42
    if value == 42 {
        return value
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_inequality_value() {
    let project = TempProject::new("cli-build-run-terminal-if-inequality");
    let source = project.write_source(
        "terminal_if_inequality_exit.nct",
        r#"func main(): i32 {
    let value = 42
    if value != 41 {
        return value
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_greater_value() {
    let project = TempProject::new("cli-build-run-terminal-if-greater");
    let source = project.write_source(
        "terminal_if_greater_exit.nct",
        r#"func main(): i32 {
    let value = 42
    if value > 41 {
        return value
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_bool_local_value() {
    let project = TempProject::new("cli-build-run-terminal-if-bool-local");
    let source = project.write_source(
        "terminal_if_bool_local_exit.nct",
        r#"func main(): i32 {
    let value = 42
    let enabled = true
    if enabled {
        return value
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_bool_or_binding_value() {
    let project = TempProject::new("cli-build-run-terminal-if-bool-or");
    let source = project.write_source(
        "terminal_if_bool_or_exit.nct",
        r#"func main(): i32 {
    let value = 42
    let ready = false
    let fallback = true
    let enabled = ready || fallback
    if enabled {
        return value
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_runs_str_len_and_index_call_results() {
    let project = TempProject::new("cli-build-run-str-call-result-ops");
    let source = project.write_source(
        "str_call_result_ops.nct",
        r#"func main(): i32 {
    let size: usize = identity("Nocter").len()
    let byte: u8 = identity("Nocter")[3]
    if size == 6 && byte == 116 {
        return 0
    } else {
        return 1
    }
}

func identity(text: &str): &str {
    return text
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(0));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_passes_direct_aggregate_argument_words() {
    let project = TempProject::new("cli-build-run-direct-aggregate-argument-words");
    let source = project.write_source(
        "direct_aggregate_argument_words.nct",
        r#"copy struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32 {
    var pair = Pair{ a: 10, b: 20, c: 7, d: 5 }
    return check(pair)
}

func check(pair: Pair): i32 {
    return pair.a + pair.b + pair.c + pair.d
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_passes_partial_direct_aggregate_argument_bytes() {
    let project = TempProject::new("cli-build-run-partial-direct-aggregate-argument-bytes");
    let source = project.write_source(
        "partial_direct_aggregate_argument_bytes.nct",
        r#"copy struct Bytes {
    first: u8
    second: u8
    third: u8
}

func main(): i32 {
    var bytes = Bytes{ first: 1, second: 2, third: 42 }
    return read(bytes) as i32
}

func read(bytes: Bytes): u8 {
    return bytes.third
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_passes_moved_non_copy_direct_aggregate_argument() {
    let project = TempProject::new("cli-build-run-moved-non-copy-direct-aggregate-argument");
    let source = project.write_source(
        "moved_non_copy_direct_aggregate_argument.nct",
        r#"struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32 {
    let pair = Pair{ a: 10, b: 20, c: 7, d: 5 }
    return check(move pair)
}

func check(pair: Pair): i32 {
    return pair.a + pair.b + pair.c + pair.d
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[test]
fn build_command_rejects_use_after_moved_non_copy_aggregate() {
    let project = TempProject::new("cli-build-reject-use-after-moved-non-copy-aggregate");
    let source = project.write_source(
        "use_after_moved_non_copy_aggregate.nct",
        r#"struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32 {
    let pair = Pair{ a: 10, b: 20, c: 7, d: 5 }
    let total = check(move pair)
    return total + pair.a
}

func check(pair: Pair): i32 {
    return pair.a + pair.b + pair.c + pair.d
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0385]"),
        "expected ownership diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("because it was moved"),
        "expected moved-state diagnostic, got:\n{stderr}"
    );
}

#[test]
fn build_command_rejects_implicit_non_copy_aggregate_argument() {
    let project = TempProject::new("cli-build-reject-implicit-non-copy-aggregate-argument");
    let source = project.write_source(
        "implicit_non_copy_aggregate_argument.nct",
        r#"struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32 {
    let pair = Pair{ a: 10, b: 20, c: 7, d: 5 }
    return check(pair)
}

func check(pair: Pair): i32 {
    return pair.a + pair.b + pair.c + pair.d
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0392]"),
        "expected aggregate argument typecheck diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("cannot implicitly copy non-copy struct `Pair` from `pair`"),
        "expected non-copy aggregate argument diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("|     return check(pair)"),
        "expected source line for aggregate argument diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("|                  ^^^^"),
        "expected source underline for aggregate argument diagnostic, got:\n{stderr}"
    );
}

#[test]
fn build_command_rejects_implicit_non_copy_aggregate_return() {
    let project = TempProject::new("cli-build-reject-implicit-non-copy-aggregate-return");
    let source = project.write_source(
        "implicit_non_copy_aggregate_return.nct",
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    return make().len
}

func make(): Text {
    let text = Text{ start: 1, len: 42, capacity: 99 }
    return text
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0393]"),
        "expected aggregate return typecheck diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("cannot implicitly copy non-copy struct `Text` from `text`"),
        "expected non-copy aggregate return diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("|     return text"),
        "expected source line for aggregate return diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("|            ^^^^"),
        "expected source underline for aggregate return diagnostic, got:\n{stderr}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_direct_aggregate_with_scalar_fields() {
    let project = TempProject::new("cli-build-run-direct-aggregate-scalar-return");
    let source = project.write_source(
        "direct_aggregate_scalar_return.nct",
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
}

func main(): i32 {
    var header = make()
    if header.ok {
        return header.code
    } else {
        return 1
    }
}

func make(): Header {
    return Header{ tag: 7, ok: true, code: 42 }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_fallible_entry_failure_reports_stderr() {
    let project = TempProject::new("cli-build-run-fallible-failure");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "fail.nct",
        r#"use std/error.Error

func main(): i32! {
    return Error.new("app.failed", "failed")
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let output = Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.failed: failed\n");
}

fn nocter<const N: usize>(project: &TempProject, args: [&str; N]) -> Output {
    Command::new(NOCTER)
        .args(args)
        .current_dir(project.root())
        .env("NOCTER_HOME", project.nocter_home())
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

fn assert_macho_executable(path: &Path) {
    let bytes = fs::read(path).unwrap();
    assert_eq!(read_u32(&bytes, 0), 0xfeed_facf);
    assert_eq!(read_u32(&bytes, 4), 0x0100_000c);
    assert_eq!(read_u32(&bytes, 12), 0x2);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    let mut value = [0; 4];
    value.copy_from_slice(&bytes[offset..offset + 4]);
    u32::from_le_bytes(value)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(unique_name(name));
        fs::create_dir_all(&root).unwrap();

        let project = Self { root };
        project.write_nocter_home();
        project
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn nocter_home(&self) -> PathBuf {
        self.root.join(".nocter")
    }

    fn write_source(&self, name: &str, text: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, text).unwrap();
        path
    }

    fn write_nocter_home_file(&self, relative: &str, text: &str) {
        let path = self.nocter_home().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }

    fn write_nocter_home(&self) {
        let home = self.nocter_home();
        fs::create_dir_all(home.join("std")).unwrap();
        fs::write(home.join("std/prelude.nct"), "pub type Int = i32\n").unwrap();
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn unique_name(name: &str) -> String {
    format!(
        "nocter-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}
