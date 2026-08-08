use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

#[test]
fn fmt_command_rewrites_source_file() {
    let project = TempProject::new("cli-fmt-rewrite");
    let source = project.write_source(
        "app.nct",
        r#"func main(  ):i32{return 0}
"#,
    );

    let output = nocter(&project, ["fmt", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "stdout:\n{}",
        text(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "stderr:\n{}",
        text(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(source).unwrap(),
        concat!("func main(): i32 {\n", "    return 0\n", "}\n",)
    );
}

#[test]
fn fmt_command_formats_package_directives_without_source_code() {
    let project = TempProject::new("cli-fmt-package");
    let source = project.write_source(
        "nocter.nct",
        "#name:\"tool\"\n#executable:{name:\"tool\"}\n#test:{name:\"unit\",module:\"./tests/unit\"}\n",
    );

    let output = nocter(&project, ["fmt", source.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        fs::read_to_string(source).unwrap(),
        concat!(
            "#name: \"tool\"\n",
            "#executable: {\n",
            "    name: \"tool\",\n",
            "}\n",
            "#test: {\n",
            "    name: \"unit\",\n",
            "    module: \"./tests/unit\",\n",
            "}\n",
        )
    );
}

#[test]
fn fmt_check_accepts_formatted_source() {
    let project = TempProject::new("cli-fmt-check-ok");
    let source = project.write_source(
        "app.nct",
        concat!("func main(): i32 {\n", "    return 0\n", "}\n",),
    );

    let output = nocter(&project, ["fmt", "--check", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "stdout:\n{}",
        text(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "stderr:\n{}",
        text(&output.stderr)
    );
}

#[test]
fn fmt_command_formats_block_use_declarations() {
    let project = TempProject::new("cli-fmt-block-use");
    let source = project.write_source(
        "app.nct",
        r#"func greet(  ):void{use std/io.{print,write as output}
print("hello")
output("done")
return}
"#,
    );

    let output = nocter(&project, ["fmt", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "stdout:\n{}",
        text(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "stderr:\n{}",
        text(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(source).unwrap(),
        concat!(
            "func greet(): void {\n",
            "    use std/io.{print, write as output}\n",
            "    print(\"hello\")\n",
            "    output(\"done\")\n",
            "    return\n",
            "}\n",
        )
    );
}

#[test]
fn fmt_command_formats_wildcard_match_and_payload_discard_patterns() {
    let project = TempProject::new("cli-fmt-wildcard-patterns");
    let source = project.write_source(
        "app.nct",
        r#"enum AppError {missing_path open_failed(path:&str)}
func main(error:AppError):i32{match error{AppError.open_failed(_){return 1}_ {return 0}}}
func code(error:AppError):i32{if error is AppError.open_failed(_){return 2}else{return 3}}
"#,
    );

    let output = nocter(&project, ["fmt", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "stdout:\n{}",
        text(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "stderr:\n{}",
        text(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(source).unwrap(),
        concat!(
            "enum AppError {\n",
            "    missing_path,\n",
            "    open_failed(path: &str),\n",
            "}\n",
            "\n",
            "func main(error: AppError): i32 {\n",
            "    match error {\n",
            "        AppError.open_failed(_) {\n",
            "            return 1\n",
            "        }\n",
            "        _ {\n",
            "            return 0\n",
            "        }\n",
            "    }\n",
            "}\n",
            "\n",
            "func code(error: AppError): i32 {\n",
            "    if error is AppError.open_failed(_) {\n",
            "        return 2\n",
            "    } else {\n",
            "        return 3\n",
            "    }\n",
            "}\n",
        )
    );
}

#[test]
fn fmt_check_reports_unformatted_source_without_rewriting() {
    let project = TempProject::new("cli-fmt-check-fail");
    let original = "func main(  ):i32{return 0}\n";
    let source = project.write_source("app.nct", original);

    let output = nocter(&project, ["fmt", "--check", source.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "stdout:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0602]"), "stderr:\n{stderr}");
    assert!(stderr.contains("is not formatted"), "stderr:\n{stderr}");
    assert!(stderr.contains("run `nocter fmt"), "stderr:\n{stderr}");
    assert_eq!(fs::read_to_string(source).unwrap(), original);
}

#[test]
fn fmt_rejects_comments_without_rewriting() {
    let project = TempProject::new("cli-fmt-comments");
    let original = concat!(
        "func main(): i32 { // keep this comment\n",
        "    return 0\n",
        "}\n",
    );
    let source = project.write_source("app.nct", original);

    let output = nocter(&project, ["fmt", source.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "stdout:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0601]"), "stderr:\n{stderr}");
    assert!(
        stderr.contains("the formatter cannot safely preserve comments yet"),
        "stderr:\n{stderr}"
    );
    assert_eq!(fs::read_to_string(source).unwrap(), original);
}

fn nocter<const N: usize>(project: &TempProject, args: [&str; N]) -> Output {
    Command::new(NOCTER)
        .args(args)
        .current_dir(project.root())
        .output()
        .unwrap()
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
        Self { root }
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn write_source(&self, name: &str, text: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, text).unwrap();
        path
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
