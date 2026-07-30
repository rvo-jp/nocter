use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

const VALID_EXAMPLES: &[ValidExample] = &[
    ValidExample::new("spec/examples/valid/hello.nct"),
    ValidExample::new("spec/examples/valid/doc-comments.nct"),
    ValidExample::new("spec/examples/valid/fallible-catch.nct"),
    ValidExample::new("spec/examples/valid/fallible-force.nct"),
    ValidExample::new("spec/examples/valid/fallible-propagation.nct"),
    ValidExample::new("spec/examples/valid/if-is-enum.nct"),
    ValidExample::new("spec/examples/valid/imports/app.nct"),
    ValidExample::new("spec/examples/valid/optional-otherwise-default.nct"),
    ValidExample::new("spec/examples/valid/optional-otherwise.nct"),
    ValidExample::new("spec/examples/valid/optional-propagation.nct"),
    ValidExample::new("spec/examples/valid/range-for.nct"),
    ValidExample::new("spec/examples/valid/match-enum.nct"),
    ValidExample::new("spec/examples/valid/default-entry.nct"),
];

const INVALID_EXAMPLES: &[InvalidExample] = &[
    InvalidExample::new("spec/examples/invalid/catch-optional.nct", "E0330"),
    InvalidExample::new("spec/examples/invalid/default-entry-missing.nct", "E0300"),
    InvalidExample::new("spec/examples/invalid/fallible-propagation.nct", "E0331"),
    InvalidExample::new("spec/examples/invalid/for-range-bound-type.nct", "E0360"),
    InvalidExample::new("spec/examples/invalid/force-plain-value.nct", "E0336"),
    InvalidExample::new("spec/examples/invalid/main-entry.nct", "E0303"),
    InvalidExample::new("spec/examples/invalid/module-declaration.nct", "E0200"),
    InvalidExample::new("spec/examples/invalid/optional-propagation.nct", "E0335"),
    InvalidExample::new(
        "spec/examples/invalid/optional-otherwise-fallback-type.nct",
        "E0397",
    ),
    InvalidExample::new("spec/examples/invalid/return-type-mismatch.nct", "E0312"),
    InvalidExample::new("spec/examples/invalid/match-non-enum.nct", "E0361"),
];

struct ValidExample {
    path: &'static str,
}

impl ValidExample {
    const fn new(path: &'static str) -> Self {
        Self { path }
    }
}

struct InvalidExample {
    path: &'static str,
    error_code: &'static str,
}

impl InvalidExample {
    const fn new(path: &'static str, error_code: &'static str) -> Self {
        Self { path, error_code }
    }
}

#[test]
fn valid_example_corpus_passes_check() {
    let project = TempProject::new("example-corpus-valid");

    for example in VALID_EXAMPLES {
        let source = repo_root().join(example.path);
        let output = check(&project, &source);

        assert_eq!(
            output.status.code(),
            Some(0),
            "valid example `{}` failed\nstdout:\n{}\nstderr:\n{}",
            example.path,
            text(&output.stdout),
            text(&output.stderr)
        );
        assert!(
            output.stdout.is_empty(),
            "valid example `{}` wrote stdout:\n{}",
            example.path,
            text(&output.stdout)
        );
        assert!(
            output.stderr.is_empty(),
            "valid example `{}` wrote stderr:\n{}",
            example.path,
            text(&output.stderr)
        );
    }
}

#[test]
fn invalid_example_corpus_fails_check() {
    let project = TempProject::new("example-corpus-invalid");

    for example in INVALID_EXAMPLES {
        let source = repo_root().join(example.path);
        let output = check(&project, &source);

        assert_ne!(
            output.status.code(),
            Some(0),
            "invalid example `{}` unexpectedly passed\nstdout:\n{}\nstderr:\n{}",
            example.path,
            text(&output.stdout),
            text(&output.stderr)
        );
        assert!(
            output.stdout.is_empty(),
            "invalid example `{}` wrote stdout:\n{}",
            example.path,
            text(&output.stdout)
        );
        let stderr = text(&output.stderr);
        let expected_error = format!("error[{}]", example.error_code);
        assert!(
            stderr.contains(&expected_error),
            "invalid example `{}` did not report expected diagnostic `{}`\nstderr:\n{}",
            example.path,
            expected_error,
            stderr
        );
    }
}

#[test]
fn valid_example_corpus_passes_check_json() {
    let project = TempProject::new("example-corpus-valid-json");

    for example in VALID_EXAMPLES {
        let source = repo_root().join(example.path);
        let output = check_json(&project, &source);

        assert_eq!(
            output.status.code(),
            Some(0),
            "valid example `{}` failed JSON check\nstdout:\n{}\nstderr:\n{}",
            example.path,
            text(&output.stdout),
            text(&output.stderr)
        );
        assert!(
            output.stderr.is_empty(),
            "valid example `{}` wrote JSON check stderr:\n{}",
            example.path,
            text(&output.stderr)
        );

        let json = parse_json_stdout(example.path, &output);
        assert_check_json_envelope(&json, example.path, true);
        assert_eq!(
            json["diagnostics"].as_array().map(Vec::len),
            Some(0),
            "valid example `{}` reported diagnostics:\n{}",
            example.path,
            json
        );
    }
}

#[test]
fn invalid_example_corpus_reports_check_json_diagnostics() {
    let project = TempProject::new("example-corpus-invalid-json");

    for example in INVALID_EXAMPLES {
        let source = repo_root().join(example.path);
        let output = check_json(&project, &source);

        assert_ne!(
            output.status.code(),
            Some(0),
            "invalid example `{}` unexpectedly passed JSON check\nstdout:\n{}\nstderr:\n{}",
            example.path,
            text(&output.stdout),
            text(&output.stderr)
        );
        assert!(
            output.stderr.is_empty(),
            "invalid example `{}` wrote JSON check stderr:\n{}",
            example.path,
            text(&output.stderr)
        );

        let json = parse_json_stdout(example.path, &output);
        assert_check_json_envelope(&json, example.path, false);
        assert!(
            diagnostics_contain_code(&json, example.error_code),
            "invalid example `{}` did not report expected JSON diagnostic `{}`\nstdout:\n{}",
            example.path,
            example.error_code,
            text(&output.stdout)
        );
    }
}

#[test]
fn doc_comment_example_emits_ast_documentation() {
    let project = TempProject::new("example-corpus-docs");
    let source = repo_root().join("spec/examples/valid/doc-comments.nct");
    let output = ast_json(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(0),
        "doc comment example failed AST JSON\nstdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "doc comment example wrote stderr:\n{}",
        text(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], Value::Bool(true));
    assert!(contains_documentation(
        &json,
        "File-level docs for tooling.\nMore file-level docs for AST JSON and future LSP hover."
    ));
    assert!(contains_documentation(
        &json,
        "Returns the process exit code."
    ));
    assert!(contains_documentation(
        &json,
        "Local binding docs are available to tooling."
    ));
}

#[test]
fn ast_json_emits_wildcard_and_discard_patterns() {
    let project = TempProject::new("example-corpus-ast-patterns");
    let source = project.root().join("patterns.nct");
    fs::write(
        &source,
        r#"enum AppError {
    missing_path
    open_failed(path: &str)
}

func main(error: AppError): i32 {
    match error {
        AppError.open_failed(_) {
            return 1
        }
        _ {
            return 0
        }
    }
}

func code(error: AppError): i32 {
    if error is AppError.open_failed(_) {
        return 2
    } else {
        return 3
    }
}
"#,
    )
    .unwrap();
    let output = ast_json(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(0),
        "pattern source failed AST JSON\nstdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "pattern source wrote stderr:\n{}",
        text(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], Value::Bool(true));
    assert!(contains_ast_node(&json, "match_payload_discard", Some("_")));
    assert!(contains_ast_node(&json, "match_wildcard_arm", None));
    assert!(contains_ast_node(&json, "if_is_payload_discard", Some("_")));
}

fn check(project: &TempProject, source: &Path) -> Output {
    Command::new(NOCTER)
        .args(["check", source.to_str().unwrap()])
        .current_dir(project.root())
        .env("NOCTER_HOME", project.nocter_home())
        .output()
        .unwrap()
}

fn check_json(project: &TempProject, source: &Path) -> Output {
    Command::new(NOCTER)
        .args(["check", source.to_str().unwrap(), "--format", "json"])
        .current_dir(project.root())
        .env("NOCTER_HOME", project.nocter_home())
        .output()
        .unwrap()
}

fn ast_json(project: &TempProject, source: &Path) -> Output {
    Command::new(NOCTER)
        .args(["ast", source.to_str().unwrap(), "--format", "json"])
        .current_dir(project.root())
        .env("NOCTER_HOME", project.nocter_home())
        .output()
        .unwrap()
}

fn parse_json_stdout(example: &str, output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "example `{}` did not emit valid JSON: {}\nstdout:\n{}\nstderr:\n{}",
            example,
            error,
            text(&output.stdout),
            text(&output.stderr)
        )
    })
}

fn assert_check_json_envelope(json: &Value, example: &str, ok: bool) {
    assert_eq!(
        json["schema"],
        Value::String("nocter.diagnostics".to_string())
    );
    assert_eq!(json["version"], Value::from(1));
    assert_eq!(json["ok"], Value::Bool(ok));
    assert_eq!(json["command"], Value::String("check".to_string()));
    assert_eq!(json["target"], Value::String("arm64-darwin".to_string()));

    let root = json["root"]
        .as_str()
        .expect("check JSON envelope should include root");
    assert!(
        root.ends_with(example),
        "check JSON root `{root}` should end with `{example}`"
    );

    let absolute_root = json["root_absolute_path"]
        .as_str()
        .expect("check JSON envelope should include root_absolute_path");
    assert!(
        absolute_root.ends_with(example),
        "check JSON root_absolute_path `{absolute_root}` should end with `{example}`"
    );
}

fn diagnostics_contain_code(json: &Value, expected_code: &str) -> bool {
    json["diagnostics"].as_array().is_some_and(|diagnostics| {
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"].as_str() == Some(expected_code))
    })
}

fn contains_documentation(value: &Value, expected: &str) -> bool {
    match value {
        Value::Object(object) => {
            object
                .get("documentation")
                .and_then(Value::as_str)
                .is_some_and(|documentation| documentation == expected)
                || object
                    .values()
                    .any(|value| contains_documentation(value, expected))
        }
        Value::Array(values) => values
            .iter()
            .any(|value| contains_documentation(value, expected)),
        _ => false,
    }
}

fn contains_ast_node(value: &Value, expected_kind: &str, expected_value: Option<&str>) -> bool {
    match value {
        Value::Object(object) => {
            let kind_matches = object
                .get("kind")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind == expected_kind);
            let value_matches = match expected_value {
                Some(expected) => object
                    .get("value")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value == expected),
                None => true,
            };

            (kind_matches && value_matches)
                || object
                    .values()
                    .any(|value| contains_ast_node(value, expected_kind, expected_value))
        }
        Value::Array(values) => values
            .iter()
            .any(|value| contains_ast_node(value, expected_kind, expected_value)),
        _ => false,
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("compiler crate should live below development root")
        .to_path_buf()
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

    fn write_nocter_home(&self) {
        let home = self.nocter_home();
        fs::create_dir_all(home.join("std")).unwrap();

        fs::write(
            home.join("std/prelude.nct"),
            concat!(
                "pub use std/error.{Error, ErrorCode}\n",
                "pub use std/string.String\n",
            ),
        )
        .unwrap();
        fs::write(
            home.join("std/error.nct"),
            concat!(
                "pub type ErrorCode = &str\n",
                "pub type Error = error\n",
                "pub(nocter) primitive new_error(code: &str, message: &str): error\n",
                "\n",
                "pub func Error.new(code: ErrorCode, message: &str): Error {\n",
                "    return new_error(code, message)\n",
                "}\n",
            ),
        )
        .unwrap();
        fs::write(
            home.join("std/string.nct"),
            concat!("pub struct String {\n", "    bytes: &[u8]\n", "}\n",),
        )
        .unwrap();
        fs::write(
            home.join("std/io.nct"),
            concat!(
                "pub func print(text: &str): void! {\n",
                "    return\n",
                "}\n",
            ),
        )
        .unwrap();
        fs::write(
            home.join("std/process.nct"),
            concat!(
                "use std/error.Error\n",
                "\n",
                "pub func env(name: &str): &str?! {\n",
                "    return Error.new(\"std.process.unsupported\", \"process environment is not implemented\")\n",
                "}\n",
            ),
        )
        .unwrap();
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
