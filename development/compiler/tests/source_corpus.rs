use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

#[path = "support/builtin_std.rs"]
mod builtin_std;

const VALID_FIXTURES: &[ValidFixture] = &[
    ValidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/valid/builtin-view-methods.nct",
    ),
    ValidFixture::new("development/compiler/tests/fixtures/source_corpus/valid/hello.nct"),
    ValidFixture::new("development/compiler/tests/fixtures/source_corpus/valid/doc-comments.nct"),
    ValidFixture::new("development/compiler/tests/fixtures/source_corpus/valid/fallible-catch.nct"),
    ValidFixture::new("development/compiler/tests/fixtures/source_corpus/valid/fallible-force.nct"),
    ValidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/valid/fallible-propagation.nct",
    ),
    ValidFixture::new("development/compiler/tests/fixtures/source_corpus/valid/if-is-enum.nct"),
    ValidFixture::new("development/compiler/tests/fixtures/source_corpus/valid/imports/app.nct"),
    ValidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/valid/optional-otherwise-default.nct",
    ),
    ValidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/valid/optional-otherwise.nct",
    ),
    ValidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/valid/optional-propagation.nct",
    ),
    ValidFixture::new("development/compiler/tests/fixtures/source_corpus/valid/range-for.nct"),
    ValidFixture::new("development/compiler/tests/fixtures/source_corpus/valid/match-enum.nct"),
    ValidFixture::new("development/compiler/tests/fixtures/source_corpus/valid/default-entry.nct"),
];

const INVALID_FIXTURES: &[InvalidFixture] = &[
    InvalidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/invalid/catch-optional.nct",
        "E0330",
    ),
    InvalidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/invalid/default-entry-missing.nct",
        "E0300",
    ),
    InvalidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/invalid/fallible-propagation.nct",
        "E0331",
    ),
    InvalidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/invalid/for-range-bound-type.nct",
        "E0360",
    ),
    InvalidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/invalid/force-plain-value.nct",
        "E0336",
    ),
    InvalidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/invalid/main-entry.nct",
        "E0303",
    ),
    InvalidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/invalid/module-declaration.nct",
        "E0200",
    ),
    InvalidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/invalid/optional-propagation.nct",
        "E0335",
    ),
    InvalidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/invalid/optional-otherwise-fallback-type.nct",
        "E0397",
    ),
    InvalidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/invalid/project-builtin-impl.nct",
        "E0416",
    ),
    InvalidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/invalid/return-type-mismatch.nct",
        "E0312",
    ),
    InvalidFixture::new(
        "development/compiler/tests/fixtures/source_corpus/invalid/match-non-enum.nct",
        "E0361",
    ),
];

struct ValidFixture {
    path: &'static str,
}

impl ValidFixture {
    const fn new(path: &'static str) -> Self {
        Self { path }
    }
}

struct InvalidFixture {
    path: &'static str,
    error_code: &'static str,
}

impl InvalidFixture {
    const fn new(path: &'static str, error_code: &'static str) -> Self {
        Self { path, error_code }
    }
}

#[test]
fn valid_source_corpus_passes_check() {
    let project = TempProject::new("source-corpus-valid");

    for fixture in VALID_FIXTURES {
        let source = repo_root().join(fixture.path);
        let output = check(&project, &source);

        assert_eq!(
            output.status.code(),
            Some(0),
            "valid fixture `{}` failed\nstdout:\n{}\nstderr:\n{}",
            fixture.path,
            text(&output.stdout),
            text(&output.stderr)
        );
        assert!(
            output.stdout.is_empty(),
            "valid fixture `{}` wrote stdout:\n{}",
            fixture.path,
            text(&output.stdout)
        );
        assert!(
            output.stderr.is_empty(),
            "valid fixture `{}` wrote stderr:\n{}",
            fixture.path,
            text(&output.stderr)
        );
    }
}

#[test]
fn invalid_source_corpus_fails_check() {
    let project = TempProject::new("source-corpus-invalid");

    for fixture in INVALID_FIXTURES {
        let source = repo_root().join(fixture.path);
        let output = check(&project, &source);

        assert_ne!(
            output.status.code(),
            Some(0),
            "invalid fixture `{}` unexpectedly passed\nstdout:\n{}\nstderr:\n{}",
            fixture.path,
            text(&output.stdout),
            text(&output.stderr)
        );
        assert!(
            output.stdout.is_empty(),
            "invalid fixture `{}` wrote stdout:\n{}",
            fixture.path,
            text(&output.stdout)
        );
        let stderr = text(&output.stderr);
        let expected_error = format!("error[{}]", fixture.error_code);
        assert!(
            stderr.contains(&expected_error),
            "invalid fixture `{}` did not report expected diagnostic `{}`\nstderr:\n{}",
            fixture.path,
            expected_error,
            stderr
        );
    }
}

#[test]
fn valid_source_corpus_passes_check_json() {
    let project = TempProject::new("source-corpus-valid-json");

    for fixture in VALID_FIXTURES {
        let source = repo_root().join(fixture.path);
        let output = check_json(&project, &source);

        assert_eq!(
            output.status.code(),
            Some(0),
            "valid fixture `{}` failed JSON check\nstdout:\n{}\nstderr:\n{}",
            fixture.path,
            text(&output.stdout),
            text(&output.stderr)
        );
        assert!(
            output.stderr.is_empty(),
            "valid fixture `{}` wrote JSON check stderr:\n{}",
            fixture.path,
            text(&output.stderr)
        );

        let json = parse_json_stdout(fixture.path, &output);
        assert_check_json_envelope(&json, fixture.path, true);
        assert_eq!(
            json["diagnostics"].as_array().map(Vec::len),
            Some(0),
            "valid fixture `{}` reported diagnostics:\n{}",
            fixture.path,
            json
        );
    }
}

#[test]
fn invalid_source_corpus_reports_check_json_diagnostics() {
    let project = TempProject::new("source-corpus-invalid-json");

    for fixture in INVALID_FIXTURES {
        let source = repo_root().join(fixture.path);
        let output = check_json(&project, &source);

        assert_ne!(
            output.status.code(),
            Some(0),
            "invalid fixture `{}` unexpectedly passed JSON check\nstdout:\n{}\nstderr:\n{}",
            fixture.path,
            text(&output.stdout),
            text(&output.stderr)
        );
        assert!(
            output.stderr.is_empty(),
            "invalid fixture `{}` wrote JSON check stderr:\n{}",
            fixture.path,
            text(&output.stderr)
        );

        let json = parse_json_stdout(fixture.path, &output);
        assert_check_json_envelope(&json, fixture.path, false);
        assert!(
            diagnostics_contain_code(&json, fixture.error_code),
            "invalid fixture `{}` did not report expected JSON diagnostic `{}`\nstdout:\n{}",
            fixture.path,
            fixture.error_code,
            text(&output.stdout)
        );
    }
}

#[test]
fn doc_comment_fixture_emits_ast_documentation() {
    let project = TempProject::new("source-corpus-docs");
    let source = repo_root()
        .join("development/compiler/tests/fixtures/source_corpus/valid/doc-comments.nct");
    let output = ast_json(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(0),
        "doc comment fixture failed AST JSON\nstdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "doc comment fixture wrote stderr:\n{}",
        text(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], Value::Bool(true));
    assert!(contains_documentation(
        &json,
        "File-level docs for source-corpus tooling coverage.\nMore file-level docs for AST JSON and future LSP hover."
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
fn ast_json_preserves_explicit_package_test_target_shape() {
    let project = TempProject::new("source-corpus-ast-test-target");
    let source = project.root().join("nocter.nct");
    fs::write(
        &source,
        "#test: { name: \"unit\", module: \"./tests/unit\" }\n",
    )
    .unwrap();

    let output = ast_json(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(0),
        "test target failed AST JSON\nstdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], Value::Bool(true));
    assert!(contains_ast_node(&json, "package_directive", Some("test")));
    assert!(contains_ast_node(&json, "directive_field", Some("name")));
    assert!(contains_ast_node(&json, "directive_field", Some("module")));
    assert!(contains_ast_node(
        &json,
        "directive_string",
        Some("./tests/unit")
    ));
}

#[test]
fn ast_json_emits_wildcard_and_discard_patterns() {
    let project = TempProject::new("source-corpus-ast-patterns");
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

fn parse_json_stdout(fixture: &str, output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "fixture `{}` did not emit valid JSON: {}\nstdout:\n{}\nstderr:\n{}",
            fixture,
            error,
            text(&output.stdout),
            text(&output.stderr)
        )
    })
}

fn assert_check_json_envelope(json: &Value, fixture: &str, ok: bool) {
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
        root.ends_with(fixture),
        "check JSON root `{root}` should end with `{fixture}`"
    );

    let absolute_root = json["root_absolute_path"]
        .as_str()
        .expect("check JSON envelope should include root_absolute_path");
    assert!(
        absolute_root.ends_with(fixture),
        "check JSON root_absolute_path `{absolute_root}` should end with `{fixture}`"
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
        for module in ["prelude", "error", "string", "io", "process"] {
            fs::create_dir_all(home.join("std").join(module)).unwrap();
        }
        builtin_std::write_builtin_type_surfaces(&home);

        fs::write(
            home.join("std/prelude/index.nct"),
            concat!(
                "pub use std/error.{Error, ErrorCode}\n",
                "pub use std/string.String\n",
            ),
        )
        .unwrap();
        fs::write(
            home.join("std/error/index.nct"),
            concat!(
                "pub type ErrorCode = &str\n",
                "pub type Error = error\n",
                "pub(nocter) primitive new_error(code: &str, message: &str): error\n",
                "\n",
                "pub func Error.new(code: ErrorCode, message: &str): Error from code | message {\n",
                "    return new_error(code, message)\n",
                "}\n",
            ),
        )
        .unwrap();
        fs::write(
            home.join("std/string/index.nct"),
            concat!("pub struct String {\n", "    bytes: &[u8]\n", "}\n",),
        )
        .unwrap();
        fs::write(
            home.join("std/io/index.nct"),
            concat!(
                "pub func print(text: &str): void! {\n",
                "    return\n",
                "}\n",
            ),
        )
        .unwrap();
        fs::write(
            home.join("std/process/index.nct"),
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
