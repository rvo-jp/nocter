use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn bare_source_command_is_rejected() {
    let project = TempProject::new("cli-run-bare-source");
    let source = project.write_source(
        "exit23.nct",
        r#"func main(): i32 {
    return 23
}
"#,
    );

    let output = nocter(&project, [source.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output.stderr).contains("unknown command"));
}

#[test]
fn check_command_uses_nocter_nct_when_source_is_omitted() {
    let project = TempProject::new("cli-check-default-source");
    project.write_source(
        "nocter.nct",
        r#"pub func answer(): i32 {
    return 42
}
"#,
    );

    let output = nocter(&project, ["check"]);

    assert_eq!(output.status.code(), Some(0));
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

#[test]
fn check_command_rejects_entry_option() {
    let project = TempProject::new("cli-check-reject-entry");
    let source = project.write_source(
        "app.nct",
        r#"func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter(
        &project,
        ["check", source.to_str().unwrap(), "--entry", "start"],
    );

    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("unexpected argument `--entry`"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn check_command_accepts_target_option() {
    let project = TempProject::new("cli-check-target");
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
            "arm64-darwin",
        ],
    );

    assert_eq!(output.status.code(), Some(0));
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
