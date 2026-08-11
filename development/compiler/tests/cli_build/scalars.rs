use super::*;

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
