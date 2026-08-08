use super::*;

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
    let file = File { fd: 1 }
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
fn build_command_does_not_treat_relative_std_process_as_std_contract() {
    let project = TempProject::new("cli-build-relative-std-process-not-contract");
    let local_std = project.root().join("std/process");
    fs::create_dir_all(&local_std).unwrap();
    fs::write(
        local_std.join("index.nct"),
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
