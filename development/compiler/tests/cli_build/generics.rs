use super::*;

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
