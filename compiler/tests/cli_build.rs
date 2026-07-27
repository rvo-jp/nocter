use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

#[test]
fn command_line_errors_use_diagnostic_display() {
    let project = TempProject::new("cli-command-line-diagnostic");

    let output = nocter(&project, ["wat"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0700]"), "stderr:\n{stderr}");
    assert!(
        stderr.contains("unknown command `wat`"),
        "stderr:\n{stderr}"
    );
    assert!(stderr.contains("run `nocter help`"), "stderr:\n{stderr}");
}

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
fn build_command_uses_main_nct_when_source_is_omitted() {
    let project = TempProject::new("cli-build-default-source");
    let source = project.write_source(
        "main.nct",
        r#"func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter(&project, ["build"]);
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
    assert!(stderr.contains("error[E0701]"), "stderr:\n{stderr}");
    assert!(
        stderr.contains("target `x64-linux` is recognized but not implemented"),
        "expected unimplemented target error, got:\n{stderr}"
    );
    assert!(
        stderr.contains("use `--target arm64-darwin`"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn check_json_reports_target_parse_error_as_diagnostic_envelope() {
    let project = TempProject::new("cli-check-json-target-diagnostic");
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
            "check",
            source.to_str().unwrap(),
            "--target",
            "x64-linux",
            "--format",
            "json",
        ],
    );

    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["schema"], "nocter.diagnostics");
    assert_eq!(json["ok"], false);
    assert_eq!(json["command"], "check");
    assert_eq!(json["target"], "x64-linux");
    assert_eq!(
        json["root"],
        source.to_str().expect("source path should be UTF-8")
    );
    assert_eq!(json["diagnostics"][0]["code"], "E0701");
    assert_eq!(
        json["diagnostics"][0]["message"],
        "target `x64-linux` is recognized but not implemented"
    );
    assert_eq!(
        json["diagnostics"][0]["help"],
        "use `--target arm64-darwin` with Nocter v0"
    );
}

#[test]
fn check_json_reports_command_line_parse_error_as_diagnostic_envelope() {
    let project = TempProject::new("cli-check-json-command-line-diagnostic");
    let source = project.write_source(
        "app.nct",
        r#"func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter(
        &project,
        [
            "check",
            source.to_str().unwrap(),
            "--format",
            "json",
            "extra",
        ],
    );

    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["schema"], "nocter.diagnostics");
    assert_eq!(json["ok"], false);
    assert_eq!(json["command"], "check");
    assert_eq!(
        json["root"],
        source.to_str().expect("source path should be UTF-8")
    );
    assert_eq!(json["diagnostics"][0]["code"], "E0700");
    assert_eq!(
        json["diagnostics"][0]["message"],
        "unexpected argument `extra`"
    );
    assert_eq!(
        json["diagnostics"][0]["help"],
        "run `nocter help` to see supported commands and options"
    );
}

#[test]
fn build_command_reports_missing_source_as_filesystem_diagnostic() {
    let project = TempProject::new("cli-build-missing-source-diagnostic");
    let missing = project.root().join("missing.nct");

    let output = nocter(&project, ["build", missing.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0702]"), "stderr:\n{stderr}");
    assert!(
        stderr.contains("failed to resolve source file"),
        "stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("check the path passed to the command"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn check_json_reports_missing_source_as_diagnostic_envelope() {
    let project = TempProject::new("cli-check-json-missing-source-diagnostic");
    let missing = project.root().join("missing.nct");

    let output = nocter(
        &project,
        ["check", missing.to_str().unwrap(), "--format", "json"],
    );

    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["schema"], "nocter.diagnostics");
    assert_eq!(json["ok"], false);
    assert_eq!(json["command"], "check");
    assert_eq!(json["target"], "arm64-darwin");
    assert_eq!(
        json["root"],
        missing.to_str().expect("missing path should be UTF-8")
    );
    assert_eq!(json["diagnostics"][0]["code"], "E0702");
    assert!(
        json["diagnostics"][0]["message"]
            .as_str()
            .expect("message should be a string")
            .contains("failed to resolve source file")
    );
}

#[test]
fn doctor_reports_nocter_home_errors_as_diagnostics() {
    let project = TempProject::new("cli-doctor-home-diagnostic");
    let missing_home = project.root().join("missing-home");

    let output = Command::new(NOCTER)
        .arg("doctor")
        .current_dir(project.root())
        .env("NOCTER_HOME", &missing_home)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0703]"), "stderr:\n{stderr}");
    assert!(
        stderr.contains("Nocter home is not a directory"),
        "stderr:\n{stderr}"
    );
    assert!(stderr.contains("NOCTER_HOME"), "stderr:\n{stderr}");
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
fn build_command_lowers_void_implicit_return_after_statements() {
    let project = TempProject::new("cli-build-void-implicit-return-after-statements");
    let source = project.write_source(
        "void_implicit_return_after_statements.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): void {
    let file = File{ fd: 1 }
    run()
}

func run(): void {
    let value = 1
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
    drop &+self {
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
    drop &+self {
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
    drop &+self {
        return
    }
}

struct Wide {
    left: i32
    right: i32
}

impl Wide {
    drop &+self {
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
fn build_command_lowers_nonterminal_slice_index_assignments() {
    let project = TempProject::new("cli-build-nonterminal-slice-index-assignments");
    let source = project.write_source(
        "nonterminal_slice_index_assignments.nct",
        r#"func main(): i32 {
    let bytes = buffer()
    if true {
        bytes[0] = 1
    }
    while false {
        bytes[1] = 2
    }
    return 0
}

func buffer(): &+[u8] {
    return buffer()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_slice_index_compound_assignments() {
    let project = TempProject::new("cli-build-slice-index-compound-assignments");
    let source = project.write_source(
        "slice_index_compound_assignments.nct",
        r#"func main(): i32 {
    let numbers = i32_buffer()
    numbers[0] += 1
    let words = usize_buffer()
    words[1] %= 5
    return 0
}

func i32_buffer(): &+[i32] {
    return i32_buffer()
}

func usize_buffer(): &+[usize] {
    return usize_buffer()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_nonterminal_slice_index_compound_assignments() {
    let project = TempProject::new("cli-build-nonterminal-slice-index-compound-assignments");
    let source = project.write_source(
        "nonterminal_slice_index_compound_assignments.nct",
        r#"func main(): i32 {
    let numbers = i32_buffer()
    if true {
        numbers[0] += 1
    }
    let words = usize_buffer()
    while false {
        words[1] %= 5
    }
    return 0
}

func i32_buffer(): &+[i32] {
    return i32_buffer()
}

func usize_buffer(): &+[usize] {
    return usize_buffer()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_rejects_u8_slice_index_compound_assignment_before_ir_lowering() {
    let project = TempProject::new("cli-build-u8-slice-index-compound-boundary");
    let source = project.write_source(
        "u8_slice_index_compound_boundary.nct",
        r#"func main(): i32 {
    let bytes = buffer()
    bytes[0] += 1
    return 0
}

func buffer(): &+[u8] {
    return buffer()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("compound assignment statements"),
        "expected compound assignment diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("3 |     bytes[0] += 1"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
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
    drop &+self {
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
    drop &+self {
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
    drop &+self {
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
    drop &+self {
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
    drop &+self {
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
    drop &+self {
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
    drop &+self {
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
    drop &+self {
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
    drop &+self {
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
    drop &+self {
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
    drop &+self {
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
    drop &+self {
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
    drop &+self {
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
    drop &+self {
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
    drop &+self {
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
    drop &+self {
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
fn build_command_lowers_terminal_if_aggregate_scalar_field_short_circuit() {
    let project = TempProject::new("cli-build-terminal-if-aggregate-scalar-field-short-circuit");
    let source = project.write_source(
        "terminal_if_aggregate_scalar_field_short_circuit.nct",
        r#"struct Header {
    tag: u8
    ok: bool
}

func main(): i32 {
    let header = Header{ tag: 7, ok: true }
    if header.ok && header.tag == 7 {
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
fn build_command_reports_bool_compound_assignment_compile_diagnostic() {
    let project = TempProject::new("cli-build-bool-compound-assignment-diagnostic");
    let source = project.write_source(
        "bool_compound_assignment_diagnostic.nct",
        r#"func main(): i32 {
    var ready = true
    ready += false
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0437]"),
        "expected compound assignment diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("compound assignment requires matching integer operands"),
        "expected compound assignment diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("3 |     ready += false"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E0435]"),
        "typecheck should reject before buildability preflight, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_lowers_field_compound_assignment() {
    let project = TempProject::new("cli-build-field-compound-assignment");
    let source = project.write_source(
        "field_compound_assignment.nct",
        r#"struct Counter {
    value: i32
}

func main(): i32 {
    var counter = Counter{ value: 1 }
    counter.value += 1
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
fn build_command_lowers_nonterminal_field_compound_assignments() {
    let project = TempProject::new("cli-build-nonterminal-field-compound-assignment");
    let source = project.write_source(
        "nonterminal_field_compound_assignment.nct",
        r#"struct Counter {
    count: i32
    size: usize
}

func main(): i32 {
    var counter = Counter{ count: 40, size: 47 }
    if true {
        counter.count += 1
    }
    while false {
        counter.size %= 5
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
fn build_command_reports_bool_field_compound_assignment_compile_diagnostic() {
    let project = TempProject::new("cli-build-bool-field-compound-assignment-diagnostic");
    let source = project.write_source(
        "bool_field_compound_assignment_diagnostic.nct",
        r#"struct Flag {
    ready: bool
}

func main(): i32 {
    var flag = Flag{ ready: true }
    flag.ready += false
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0437]"),
        "expected compound assignment diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("compound assignment requires matching integer operands"),
        "expected compound assignment diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("7 |     flag.ready += false"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E0435]"),
        "typecheck should reject before buildability preflight, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_lowers_compound_bool_equality_nested_call_operand() {
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

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        executable.exists(),
        "build should leave an executable for nested call bool equality"
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
fn build_command_lowers_aggregate_field_borrow_argument() {
    let project = TempProject::new("cli-build-aggregate-field-borrow-argument");
    let source = project.write_source(
        "aggregate_field_borrow_argument.nct",
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

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_readonly_temporary_scalar_borrow_argument() {
    let project = TempProject::new("cli-build-readonly-temporary-scalar-borrow-argument");
    let source = project.write_source(
        "readonly_temporary_scalar_borrow_argument.nct",
        r#"func main(): i32 {
    return choose(&answer(), 0)
}

func answer(): i32 {
    return 1
}

func choose(value: &i32, fallback: i32): i32 {
    return fallback
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_non_binding_root_borrow_argument() {
    let project = TempProject::new("cli-build-non-binding-root-borrow-argument");
    let source = project.write_source(
        "non_binding_root_borrow_argument.nct",
        r#"copy struct Pair {
    value: i32
}

func main(): i32 {
    return choose(&make().value, 0)
}

func make(): Pair {
    return Pair{ value: 1 }
}

func choose(value: &i32, fallback: i32): i32 {
    return fallback
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
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
    drop &+self {
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
    drop &+self {
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
    drop &+self {
        return
    }
}

struct Holder {
    file: File
}

impl Holder {
    drop &+self {
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
fn build_command_lowers_ignored_scalar_call_expression_statement() {
    let project = TempProject::new("cli-build-ignored-scalar-call-statement");
    let source = project.write_source(
        "ignored_scalar_call_statement.nct",
        r#"func value(): i32 {
    return 1
}

func main(): void {
    value()
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
fn build_command_lowers_ignored_view_call_expression_statement() {
    let project = TempProject::new("cli-build-ignored-view-call-statement");
    project.write_nocter_home_file(
        "std/string.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "ignored_view_call_statement.nct",
        r#"use std/string.bytes

func text(): &str {
    return "ignored"
}

func data(): &[u8] {
    return bytes("ignored")
}

func main(): void {
    text()
    data()
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
fn build_command_lowers_ignored_fallible_scalar_and_view_call_expression_statement() {
    let project = TempProject::new("cli-build-ignored-fallible-scalar-view-call-statement");
    project.write_nocter_home_file(
        "std/string.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "ignored_fallible_scalar_view_call_statement.nct",
        r#"use std/string.bytes

func value(): i32! {
    return 1
}

func text(): &str! {
    return "ignored"
}

func data(): &[u8]! {
    return bytes("ignored")
}

func main(): void! {
    value()?
    text()?
    data()?
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
fn build_command_lowers_ignored_aggregate_call_expression_statement() {
    let project = TempProject::new("cli-build-ignored-aggregate-call-statement");
    let source = project.write_source(
        "ignored_aggregate_call_statement.nct",
        r#"struct Value {
    code: i32
}

func value(): Value {
    return Value{ code: 1 }
}

func main(): void {
    value()
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
fn build_command_lowers_ignored_aggregate_literal_expression_statement() {
    let project = TempProject::new("cli-build-ignored-aggregate-literal-statement");
    let source = project.write_source(
        "ignored_aggregate_literal_statement.nct",
        r#"struct Value {
    code: i32
}

func main(): void {
    Value{ code: 1 }
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
fn build_command_lowers_ignored_fallible_aggregate_call_expression_statement() {
    let project = TempProject::new("cli-build-ignored-fallible-aggregate-call-statement");
    let source = project.write_source(
        "ignored_fallible_aggregate_call_statement.nct",
        r#"struct Value {
    code: i32
}

func value(): Value! {
    return Value{ code: 1 }
}

func main(): void! {
    value()?
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
fn build_command_rejects_ignored_unsupported_method_call_expression_statement_before_ir_lowering() {
    let project = TempProject::new("cli-build-ignored-unsupported-method-call-statement");
    let source = project.write_source(
        "ignored_unsupported_method_call_statement.nct",
        r#"struct Box {
    value: i32
}

impl Box {
    method &+self.borrow_self(): &+Self {
        return self
    }
}

func main(): void {
    var box = Box{ value: 1 }
    box.borrow_self()
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("13 |     box.borrow_self()"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "method expression statement should be rejected before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after compile diagnostics"
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
fn build_command_lowers_explicit_move_in_terminal_if_condition() {
    let project = TempProject::new("cli-build-move-in-terminal-if-condition");
    let source = project.write_source(
        "move_in_terminal_if_condition.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
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

    assert_success(&output);
    assert_macho_executable(&executable);
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
fn build_command_reports_payload_match_before_ir_lowering() {
    let project = TempProject::new("cli-build-payload-match-boundary");
    let source = project.write_source(
        "payload_match_boundary.nct",
        r#"enum AppError {
    missing_path
    open_failed(path: &str)
}

func main(): i32 {
    return describe(AppError.missing_path)
}

func describe(error: AppError): i32 {
    match error {
        AppError.missing_path {
            return 0
        }

        AppError.open_failed(path) {
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
        stderr.contains("11 |     match error {"),
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
fn build_command_lowers_generic_function_with_concrete_arguments() {
    let project = TempProject::new("cli-build-generic-function");
    let source = project.write_source(
        "generic_function.nct",
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

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_generic_associated_function_with_concrete_arguments() {
    let project = TempProject::new("cli-build-generic-associated-function");
    let source = project.write_source(
        "generic_associated_function.nct",
        r#"struct Box<T> {
    value: T
}

func Box.unwrap<T>(box: Box<T>): T {
    return box.value
}

func main(): i32 {
    let box = Box<i32>{ value: 42 }
    return Box.unwrap(move box)
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
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
fn build_command_lowers_concrete_generic_struct_literal() {
    let project = TempProject::new("cli-build-concrete-generic-struct-literal");
    let source = project.write_source(
        "concrete_generic_struct_literal.nct",
        r#"struct Box<T> {
    value: T
}

func main(): i32 {
    let box = Box<i32>{
        value: 42,
    }
    return box.value
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
fn build_command_reports_reachable_nested_fallible_method_return_before_ir_lowering() {
    let project = TempProject::new("cli-build-nested-fallible-method-return-boundary");
    let source = project.write_source(
        "nested_fallible_method_return_boundary.nct",
        r#"copy struct Holder {
    pub value: i32
}

impl Holder {
    pub method &self.make_value(): (i32?)! {
        return none
    }
}

func main(): i32 {
    let holder = Holder { value: 0 }
    return consume(holder.make_value()!)
}

func consume(item: i32?): i32 {
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
        stderr.contains("nested fallible or optional return types"),
        "expected nested fallible return diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("6 |     pub method &self.make_value(): (i32?)! {"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E8007]"),
        "buildability preflight should reject before IR method lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_reachable_nested_fallible_associated_return_before_ir_lowering() {
    let project = TempProject::new("cli-build-nested-fallible-associated-return-boundary");
    let source = project.write_source(
        "nested_fallible_associated_return_boundary.nct",
        r#"copy struct Holder {
    pub value: i32
}

func Holder.make_value(): (i32?)! {
    return none
}

func main(): i32 {
    return consume(Holder.make_value()!)
}

func consume(item: i32?): i32 {
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
        stderr.contains("nested fallible or optional return types"),
        "expected nested fallible return diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("5 | func Holder.make_value(): (i32?)! {"),
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
fn build_command_does_not_reject_unreachable_nested_fallible_method_return() {
    let project = TempProject::new("cli-build-unreachable-nested-fallible-method-return");
    let source = project.write_source(
        "unreachable_nested_fallible_method_return.nct",
        r#"copy struct Holder {
    pub value: i32
}

impl Holder {
    pub method &self.make_value(): (i32?)! {
        return none
    }
}

func main(): i32 {
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
fn build_command_lowers_std_process_args_failure_boundary() {
    let project = TempProject::new("cli-build-process-args-failure-boundary");
    write_process_contract_std(&project);
    let source = project.write_source(
        "process_args_failure_boundary.nct",
        r#"use std/process.args

func main(): i32! {
    let values = args()?
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
fn build_command_reports_std_process_env_check_only_before_ir_lowering() {
    let project = TempProject::new("cli-build-process-env-check-only-boundary");
    write_process_contract_std(&project);
    let source = project.write_source(
        "process_env_check_only_boundary.nct",
        r#"use std/process.env as lookup

func main(): i32! {
    let value = lookup("HOME")?
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
        stderr.contains("check-only `std/process.env` calls"),
        "expected env check-only diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("4 |     let value = lookup(\"HOME\")?"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("nested fallible or optional return types"),
        "std internal return-shape diagnostic should not leak for check-only calls, got:\n{stderr}"
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
fn build_command_reports_std_process_env_bare_use_check_only_before_ir_lowering() {
    let project = TempProject::new("cli-build-process-env-bare-use-check-only-boundary");
    write_process_contract_std(&project);
    let source = project.write_source(
        "process_env_bare_use_check_only_boundary.nct",
        r#"use std/process

func main(): i32! {
    let value = env("HOME")?
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
        stderr.contains("check-only `std/process.env` calls"),
        "expected env check-only diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("4 |     let value = env(\"HOME\")?"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("nested fallible or optional return types"),
        "std internal return-shape diagnostic should not leak for check-only calls, got:\n{stderr}"
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
fn build_command_does_not_treat_relative_std_process_as_std_contract() {
    let project = TempProject::new("cli-build-relative-std-process-not-contract");
    let local_std = project.root().join("std");
    fs::create_dir_all(&local_std).unwrap();
    fs::write(
        local_std.join("process.nct"),
        r#"pub func args(): i32! {
    return 9
}
"#,
    )
    .unwrap();
    let source = project.write_source(
        "relative_process_args.nct",
        r#"use ./std/process.args

func main(): i32! {
    return args()?
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_generic_impl_method_with_concrete_receiver() {
    let project = TempProject::new("cli-build-generic-impl-method");
    let source = project.write_source(
        "generic_impl_method.nct",
        r#"struct Box<T> {
    value: T
}

impl<U> Box<U> {
    method self.into_value(): U {
        return self.value
    }
}

func main(): i32 {
    let box = Box<i32>{ value: 42 }
    return (move box).into_value()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_concrete_generic_scope_end_drop() {
    let project = TempProject::new("cli-build-concrete-generic-scope-end-drop");
    let source = project.write_source(
        "concrete_generic_scope_end_drop.nct",
        r#"struct Box<T> {
    value: T
}

impl Box<i32> {
    drop &+self {
        return
    }
}

func main(): i32 {
    var box = Box<i32>{ value: 42 }
    return box.value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_generic_function_body_method_call_with_concrete_arguments() {
    let project = TempProject::new("cli-build-generic-function-body-method");
    let source = project.write_source(
        "generic_function_body_method.nct",
        r#"struct Box<T> {
    value: T
}

impl<U> Box<U> {
    method self.into_value(): U {
        return self.value
    }
}

func forward<T>(box: Box<T>): T {
    return (move box).into_value()
}

func main(): i32 {
    let box = Box<i32>{ value: 42 }
    return forward(move box)
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_concrete_generic_impl_method() {
    let project = TempProject::new("cli-build-concrete-generic-impl-method");
    let source = project.write_source(
        "concrete_generic_impl_method.nct",
        r#"struct Box<T> {
    value: T
}

impl Box<i32> {
    method &self.read(): i32 {
        return self.value
    }
}

func main(): i32 {
    let box = Box<i32>{ value: 42 }
    return box.read()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_temporary_method_borrow_receiver() {
    let project = TempProject::new("cli-build-temporary-method-borrow-receiver");
    let source = project.write_source(
        "temporary_method_borrow_receiver.nct",
        r#"copy struct File {
    fd: i32
}

impl File {
    method &self.value(): i32 {
        return self.fd
    }
}

func main(): i32 {
    return make_file().value()
}

func make_file(): File {
    return File{ fd: 42 }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
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
fn build_command_reports_scope_drop_body_before_ir_lowering() {
    let project = TempProject::new("cli-build-scope-drop-body-boundary");
    let source = project.write_source(
        "scope_drop_body_boundary.nct",
        r#"struct Resource {
    value: i32
}

impl Resource {
    drop &+self {
        let bytes: [u8; 2] = [1, 2]
        return
    }
}

func main(): i32 {
    let resource = Resource{ value: 1 }
    return resource.value
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
        "expected array literal diagnostic from drop body, got:\n{stderr}"
    );
    assert!(
        stderr.contains("7 |         let bytes: [u8; 2] = [1, 2]"),
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
fn build_command_reports_field_replacement_drop_body_before_ir_lowering() {
    let project = TempProject::new("cli-build-field-replacement-drop-body-boundary");
    let source = project.write_source(
        "field_replacement_drop_body_boundary.nct",
        r#"struct Resource {
    value: i32
}

impl Resource {
    drop &+self {
        let bytes: [u8; 2] = [1, 2]
        return
    }
}

struct Holder {
    inner: Resource
}

func main(): i32 {
    var holder = Holder{ inner: Resource{ value: 1 } }
    holder.inner = Resource{ value: 2 }
    return holder.inner.value
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
        "expected array literal diagnostic from field drop body, got:\n{stderr}"
    );
    assert!(
        stderr.contains("7 |         let bytes: [u8; 2] = [1, 2]"),
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
fn build_command_lowers_member_rooted_slice_index_assignment() {
    let project = TempProject::new("cli-build-member-rooted-slice-index-assignment");
    project.write_nocter_home_file(
        "std/ptr.nct",
        r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive slice_from_raw_parts_mut(pointer: *u8, len: usize): &+[u8]
"#,
    );
    project.write_nocter_home_file(
        "std/buffer.nct",
        r#"use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_mut

pub func buffer(): &+[u8] {
    return slice_from_raw_parts_mut(from_addr(0), 0)
}
"#,
    );
    let source = project.write_source(
        "member_rooted_slice_index_assignment.nct",
        r#"use std/buffer.buffer

struct Buffer {
    pub bytes: &+[u8]
}

func main(): i32 {
    let holder = Buffer{ bytes: buffer() }
    holder.bytes[0] = 1
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
fn build_command_accepts_str_equality() {
    let project = TempProject::new("cli-build-str-equality");
    let source = project.write_source(
        "str_equality.nct",
        r#"func main(): i32 {
    if "a" == "b" {
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
fn build_command_accepts_payloadless_enum_equality() {
    let project = TempProject::new("cli-build-payloadless-enum-equality");
    let source = project.write_source(
        "payloadless_enum_equality.nct",
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    if Choice.yes == Choice.no {
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
fn build_command_accepts_payloadless_if_is() {
    let project = TempProject::new("cli-build-payloadless-if-is");
    let source = project.write_source(
        "payloadless_if_is.nct",
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    if choice is Choice.yes {
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
}

#[test]
fn build_command_accepts_payloadless_match() {
    let project = TempProject::new("cli-build-payloadless-match");
    let source = project.write_source(
        "payloadless_match.nct",
        r#"enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    match choose() {
        Choice.yes {
            return 1
        }

        Choice.no {
            return 2
        }

        Choice.maybe {
            return 3
        }
    }
}

func choose(): Choice {
    return Choice.yes
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payloadless_match_expression_body_result() {
    let project = TempProject::new("cli-build-payloadless-match-expression-body-result");
    let source = project.write_source(
        "payloadless_match_expression_body_result.nct",
        r#"enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    let choice = Choice.no
    match choice {
        Choice.yes { 1 }
        Choice.no { 2 }
        else { 3 }
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
fn build_command_accepts_terminal_control_return_expressions() {
    let project = TempProject::new("cli-build-terminal-control-return-expressions");
    let source = project.write_source(
        "terminal_control_return_expressions.nct",
        r#"enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    return from_if(1) + from_if_is(Choice.no) + from_match(Choice.maybe)
}

func from_if(value: i32): i32 {
    return if value == 1 {
        10
    } else {
        1
    }
}

func from_if_is(choice: Choice): i32 {
    return if choice is Choice.no {
        20
    } else {
        2
    }
}

func from_match(choice: Choice): i32 {
    return match choice {
        Choice.yes { 3 }
        Choice.no { 4 }
        else { 12 }
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
fn build_command_accepts_value_if_bindings_and_assignments() {
    let project = TempProject::new("cli-build-value-if-bindings-and-assignments");
    let source = project.write_source(
        "value_if_bindings_and_assignments.nct",
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let byte: u8 = if true { 5 } else { 1 }
    let size: usize = if byte == 5 { 7 } else { 1 }
    let text: &str = if size == 7 { "Nocter" } else { "Other" }
    let ok: bool = if text == "Nocter" { true } else { false }
    var code = if ok { 10 } else { 1 }
    let choice = Choice.no
    code = if choice is Choice.no { code + 32 } else { 0 }
    return code
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_value_if_aggregate_scalar_field_assignments() {
    let project = TempProject::new("cli-build-value-if-aggregate-scalar-field-assignments");
    let source = project.write_source(
        "value_if_aggregate_scalar_field_assignments.nct",
        r#"copy struct Packet {
    count: i32
    byte: u8
    size: usize
    ok: bool
}

enum Choice {
    yes
    no
}

func main(): i32 {
    var packet = Packet{ count: 0, byte: 0, size: 0, ok: false }
    let choice = Choice.no
    packet.count = if choice is Choice.no { 10 } else { 1 }
    packet.byte = if packet.count == 10 { 5 } else { 1 }
    packet.size = if packet.count == 10 { 7 } else { 1 }
    packet.ok = if packet.count == 10 { true } else { false }
    return if packet.ok { packet.count + 32 } else { 1 }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_value_match_bindings_and_assignments() {
    let project = TempProject::new("cli-build-value-match-bindings-and-assignments");
    project.write_nocter_home_file(
        "std/string.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "value_match_bindings_and_assignments.nct",
        r#"use std/string.bytes

copy struct Packet {
    count: i32
    byte: u8
    size: usize
    ok: bool
}

enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    let choice = Choice.no
    let code = match choice {
        Choice.yes { 1 }
        Choice.no { 10 }
        else { 0 }
    }
    let byte: u8 = match choice { Choice.no { 5 } else { 1 } }
    let size: usize = match choice { Choice.no { 7 } else { 1 } }
    let text: &str = match choice { Choice.no { "Nocter" } else { "Other" } }
    let data: &[u8] = match choice { Choice.no { bytes(text) } else { bytes("x") } }
    let ok: bool = match choice { Choice.no { data.len() == 6 } else { false } }
    var total = 0
    total = match choice { Choice.no { code } else { 1 } }
    var packet = Packet{ count: 0, byte: 0, size: 0, ok: false }
    packet.count = match choice { Choice.no { total } else { 1 } }
    packet.byte = match choice { Choice.no { byte } else { 1 } }
    packet.size = match choice { Choice.no { size } else { 1 } }
    packet.ok = match choice { Choice.no { ok } else { false } }
    return if packet.ok { packet.count + 32 } else { 1 }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_value_control_call_arguments() {
    let project = TempProject::new("cli-build-value-control-call-arguments");
    project.write_nocter_home_file(
        "std/string.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "value_control_call_arguments.nct",
        r#"use std/string.bytes

enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    let choice = Choice.no
    return score(
        if choice is Choice.no { 5 } else { 1 },
        match choice { Choice.no { 7 } else { 1 } },
        if choice is Choice.no { true } else { false },
        match choice { Choice.no { "Nocter" } else { "Other" } },
        match choice { Choice.no { bytes("abc") } else { bytes("x") } }
    )
}

func score(byte: u8, size: usize, ok: bool, text: &str, data: &[u8]): i32 {
    if byte == 5 && size == 7 && ok && text == "Nocter" && data.len() == 3 {
        42
    } else {
        1
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
fn build_command_accepts_value_control_method_call_arguments() {
    let project = TempProject::new("cli-build-value-control-method-call-arguments");
    project.write_nocter_home_file(
        "std/string.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "value_control_method_call_arguments.nct",
        r#"use std/string.bytes

copy struct Checker {
    seed: i32
}

impl Checker {
    method &self.score(byte: u8, size: usize, ok: bool, text: &str, data: &[u8]): i32 {
        if self.seed == 40 && byte == 5 && size == 7 && ok && text == "Nocter" && data.len() == 3 {
            42
        } else {
            1
        }
    }
}

enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    let choice = Choice.no
    let checker = Checker{ seed: 40 }
    return checker.score(
        if choice is Choice.no { 5 } else { 1 },
        match choice { Choice.no { 7 } else { 1 } },
        if choice is Choice.no { true } else { false },
        match choice { Choice.no { "Nocter" } else { "Other" } },
        match choice { Choice.no { bytes("abc") } else { bytes("x") } }
    )
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_value_control_struct_literal_scalar_fields() {
    let project = TempProject::new("cli-build-value-control-struct-literal-scalar-fields");
    let source = project.write_source(
        "value_control_struct_literal_scalar_fields.nct",
        r#"copy struct Header {
    code: i32
    tag: u8
    size: usize
    ok: bool
}

enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    let choice = Choice.no
    let header = Header{
        code: if choice is Choice.no { 10 } else { 1 },
        tag: match choice { Choice.no { 5 } else { 1 } },
        size: match choice { Choice.no { 7 } else { 1 } },
        ok: if choice is Choice.no { true } else { false }
    }
    return if header.ok && header.tag == 5 && header.size == 7 {
        header.code + 32
    } else {
        1
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
fn build_command_lowers_str_view_aggregate_fields() {
    let project = TempProject::new("cli-build-str-view-aggregate-fields");
    let source = project.write_source(
        "str_view_aggregate_fields.nct",
        r#"copy struct Label {
    text: &str
}

enum Choice {
    yes
    no
}

func make_label(text: &str): Label {
    return Label{ text: text }
}

func main(): i32 {
    let choice = Choice.yes
    var label = Label{ text: if choice is Choice.yes { "old" } else { "bad" } }
    if label.text != "old" {
        return 1
    }

    label.text = match choice { Choice.yes { "Nocter" } else { "Other" } }
    if label.text != "Nocter" {
        return 2
    }

    let returned = make_label("Done")
    if returned.text == "Done" {
        return 0
    }
    return 3
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_slice_view_aggregate_fields() {
    let project = TempProject::new("cli-build-slice-view-aggregate-fields");
    project.write_nocter_home_file(
        "std/string.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "slice_view_aggregate_fields.nct",
        r#"use std/string.bytes

copy struct Packet {
    data: &[u8]
}

enum Choice {
    yes
    no
}

func make_packet(data: &[u8]): Packet {
    return Packet{ data: data }
}

func packet_data(packet: Packet): &[u8] {
    return packet.data
}

func main(): i32 {
    let choice = Choice.yes
    var packet = Packet{ data: if choice is Choice.yes { bytes("Nocter") } else { bytes("x") } }
    if packet.data.len() != 6 {
        return 1
    }
    if packet.data[0] != 78 {
        return 2
    }

    let data: &[u8] = packet.data
    if data[5] != 114 {
        return 3
    }

    packet.data = match choice { Choice.yes { bytes("Done") } else { bytes("bad") } }
    if packet.data.len() != 4 {
        return 4
    }

    let returned = make_packet(bytes("OK"))
    let returned_data = packet_data(returned)
    if returned_data[1] == 75 {
        return 0
    }
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
fn build_command_reports_payload_enum_construction_before_ir_lowering() {
    let project = TempProject::new("cli-build-payload-enum-construction-boundary");
    let source = project.write_source(
        "payload_enum_construction_boundary.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.ok(10)
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
        stderr.contains("payload enum values"),
        "expected payload enum diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("7 |     let result = Result.ok(10)"),
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
fn build_command_reports_payload_enum_member_value_before_ir_lowering() {
    let project = TempProject::new("cli-build-payload-enum-member-value-boundary");
    let source = project.write_source(
        "payload_enum_member_value_boundary.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.failed
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
        stderr.contains("payload enum values"),
        "expected payload enum diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("7 |     let result = Result.failed"),
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
fn build_command_reports_payload_match_expression_before_ir_lowering() {
    let project = TempProject::new("cli-build-payload-match-expression-boundary");
    let source = project.write_source(
        "payload_match_expression_boundary.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.ok(10)
    return match result {
        Result.ok(value) { value }
        else { 0 }
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
        stderr.contains("`match` expressions"),
        "expected match expression diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("8 |     return match result {"),
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
fn build_command_lowers_dynamic_failure_payload() {
    let project = TempProject::new("cli-build-dynamic-failure-payload");
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
        "dynamic_failure_payload.nct",
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

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_dynamic_failure_payload_code_and_message() {
    let project = TempProject::new("cli-build-dynamic-failure-payload-code-message");
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
        "dynamic_failure_payload_code_message.nct",
        r#"use std/error.Error

func main(): i32! {
    return Error.new(dynamic_code(), dynamic_message())
}

func dynamic_code(): &str {
    return "app.failed"
}

func dynamic_message(): &str {
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
fn built_executable_runs_str_is_empty_call_results() {
    let project = TempProject::new("cli-build-run-str-is-empty-call-results");
    let source = project.write_source(
        "str_is_empty_call_results.nct",
        r#"func main(): i32 {
    let empty = "".is_empty()
    let nonempty = identity("Nocter").is_empty()
    if empty && !nonempty {
        return 42
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
    assert_eq!(status.code(), Some(42));
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

fn write_process_contract_std(project: &TempProject) {
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
    project.write_nocter_home_file(
        "std/vec.nct",
        r#"pub struct Vec<T> {
    len: usize
}
"#,
    );
    project.write_nocter_home_file(
        "std/process.nct",
        r#"use std/error.Error
use std/vec.Vec

pub func args(): Vec<&str>! {
    return Error.new("std.process.unsupported", "process arguments are not implemented")
}

pub func env(name: &str): &str?! {
    return Error.new("std.process.unsupported", "process environment is not implemented")
}
"#,
    );
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
        fs::write(home.join("std/prelude.nct"), "").unwrap();
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
