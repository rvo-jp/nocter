use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_uses_package_declared_executable_when_source_is_omitted() {
    let project = TempProject::new("cli-run-default-source");
    project.write_source(
        "nocter.nct",
        r#"#executable: {
    name: "app",
    module: ".",
}
"#,
    );
    project.write_source(
        "index.nct",
        r#"func main(): i32 {
    return 13
}
"#,
    );

    let output = nocter(&project, ["run"]);

    assert_eq!(
        output.status.code(),
        Some(13),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_entry_exit_code() {
    let project = TempProject::new("cli-run-command");
    let source = project.write_source(
        "exit17.nct",
        r#"func main(): i32 {
    return 17
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(17),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}
