use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn fallible_opaque_result_builds_and_runs() {
    let project = TempProject::new("cli-run-fallible-opaque-result");
    project.write_nocter_home_file(
        "std/error/index.nct",
        r#"pub(/) primitive new_error(code: &str, message: &str): error

construct error {
    pub default func new(code: &str, message: &str): Self from code | message {
        return new_error(code, message)
    }
}
"#,
    );
    let source = project.write_source(
        "index.nct",
        r#"interface Source {
    pub type Item
    pub method &self.get(): Self.Item
}

struct Number { value: i32 }

conform Source for Number {
    type Item = i32
    method &self.get(): i32 { return self.value }
}

func make(fail: bool): some Source<Item = i32>! {
    if fail { return error.new("app.make", "failed") }
    return Number { value: 42 }
}

func main(): i32! {
    let source = make(false)?
    return source.get()
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
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}
