use super::*;

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
fn build_command_uses_index_declared_executable_when_source_is_omitted() {
    let project = TempProject::new("cli-build-default-source");
    project.write_source(
        "index.nct",
        r#"#executable: {
    name: "app",
    module: "./main",
}
"#,
    );
    project.write_source(
        "main.nct",
        r#"func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter(&project, ["build"]);
    let executable = project.root().join("app");

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
        stderr.contains("use the currently supported target `--target arm64-darwin`"),
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
        "use the currently supported target `--target arm64-darwin`"
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
        "unexpected additional source `extra`"
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
