use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

#[test]
fn runs_all_test_targets_in_declaration_order_and_continues_after_failure() {
    let project = TempPackage::new("ordered");
    project.write(
        "nocter.nct",
        r#"#name: "ordered"
#test: { name: "first", entry: "./tests/first" }
#test: { name: "fails", entry: "./tests/fails" }
#test: { name: "last", entry: "./tests/last" }
"#,
    );
    project.write("tests/first.nct", "func main(): i32 { return 0 }\n");
    project.write("tests/fails.nct", "func main(): i32 { return 7 }\n");
    project.write("tests/last.nct", "func main(): i32 { return 0 }\n");

    let output = project.nocter(["test"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        text(&output.stdout),
        concat!(
            "test first ... ok\n",
            "test fails ... FAILED\n",
            "test last ... ok\n",
            "\n",
            "test result: FAILED. 2 passed; 1 failed\n",
        )
    );
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
}

#[test]
fn selects_one_named_test_target() {
    let project = TempPackage::new("selected");
    project.write(
        "nocter.nct",
        r#"#test: { name: "selected", entry: "./selected" }
#test: { name: "unselected", entry: "./unselected" }
"#,
    );
    project.write("selected.nct", "func main(): i32 { return 0 }\n");
    project.write("unselected.nct", "func main(): i32 { return 8 }\n");

    let output = project.nocter(["test", "--test", "selected"]);

    assert_success(&output);
    assert!(text(&output.stdout).contains("test selected ... ok"));
    assert!(!text(&output.stdout).contains("unselected"));
    assert!(!project.root.join("selected").exists());
}

#[test]
fn reports_missing_and_unknown_test_targets() {
    let library = TempPackage::new("no-targets");
    library.write("nocter.nct", "#name: \"library\"\n");
    let output = library.nocter(["test"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).contains("declares no test targets"),
        "{}",
        text(&output.stderr)
    );

    let package = TempPackage::new("unknown-target");
    package.write(
        "nocter.nct",
        "#test: { name: \"unit\", entry: \"./unit\" }\n",
    );
    package.write("unit.nct", "func main(): i32 { return 0 }\n");
    let output = package.nocter(["test", "--test", "missing"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).contains("has no test named `missing`"),
        "{}",
        text(&output.stderr)
    );
}

#[test]
fn compile_failure_does_not_prevent_later_test_targets() {
    let project = TempPackage::new("compile-failure");
    project.write(
        "nocter.nct",
        r#"#test: { name: "broken", entry: "./broken" }
#test: { name: "healthy", entry: "./healthy" }
"#,
    );
    project.write(
        "broken.nct",
        "func main(argument: i32): i32 { return argument }\n",
    );
    project.write("healthy.nct", "func main(): i32 { return 0 }\n");

    let output = project.nocter(["test"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        text(&output.stdout),
        concat!(
            "test broken ... FAILED\n",
            "test healthy ... ok\n",
            "\n",
            "test result: FAILED. 1 passed; 1 failed\n",
        )
    );
    assert!(text(&output.stderr).contains("error["));
}

#[test]
fn json_report_is_one_stable_machine_readable_envelope() {
    let project = TempPackage::new("json");
    project.write(
        "nocter.nct",
        "#name: \"json-tests\"\n#test: { name: \"unit\", entry: \"./unit\" }\n",
    );
    project.write("unit.nct", "func main(): i32 { return 9 }\n");

    let output = project.nocter(["test", "--format", "json"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema"], "nocter.tests");
    assert_eq!(report["version"], 1);
    assert_eq!(report["ok"], false);
    assert_eq!(report["package"], "json-tests");
    assert_eq!(report["diagnostics"], serde_json::json!([]));
    assert_eq!(report["tests"][0]["name"], "unit");
    assert_eq!(report["tests"][0]["outcome"], "failed");
    assert_eq!(report["tests"][0]["exit_code"], 9);
    assert_eq!(report["tests"][0]["signal"], Value::Null);
    assert_eq!(report["summary"]["passed"], 0);
    assert_eq!(report["summary"]["failed"], 1);
}

#[test]
fn locked_offline_test_uses_the_package_graph_without_mutating_the_manifest() {
    let project = TempPackage::new("locked-offline");
    let manifest = "#test: { name: \"unit\", entry: \"./unit\" }\n";
    project.write("nocter.nct", manifest);
    project.write("unit.nct", "func main(): i32 { return 0 }\n");

    let output = project.nocter(["test", "--locked", "--offline"]);

    assert_success(&output);
    assert_eq!(
        fs::read_to_string(project.root.join("nocter.nct")).unwrap(),
        manifest
    );
}

#[test]
fn fallible_entry_failure_is_a_failed_test_outcome() {
    let project = TempPackage::new("fallible-failure");
    project.write(
        ".nocter/std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    project.write(
        "nocter.nct",
        "#test: { name: \"fallible\", entry: \"./fallible\" }\n",
    );
    project.write(
        "fallible.nct",
        r#"use std/error.Error

func main(): i32! {
    return Error.new("test.failed", "expected failure")
}
"#,
    );

    let output = project.nocter(["test", "--format", "json"]);

    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["tests"][0]["outcome"], "failed");
    assert_eq!(report["tests"][0]["exit_code"], 1);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn process_trap_is_isolated_and_later_tests_still_run() {
    let project = TempPackage::new("trap");
    project.write(
        "nocter.nct",
        r#"#test: { name: "traps", entry: "./traps" }
#test: { name: "healthy", entry: "./healthy" }
"#,
    );
    project.write(
        "traps.nct",
        r#"func main(): i32 {
    let zero: i32 = 0
    return 1 / zero
}
"#,
    );
    project.write("healthy.nct", "func main(): i32 { return 0 }\n");

    let output = project.nocter(["test", "--format", "json"]);

    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["tests"][0]["name"], "traps");
    assert_eq!(report["tests"][0]["outcome"], "failed");
    assert!(report["tests"][0]["signal"].as_i64().is_some());
    assert_eq!(report["tests"][1]["name"], "healthy");
    assert_eq!(report["tests"][1]["outcome"], "passed");
}

struct TempPackage {
    root: PathBuf,
    home: PathBuf,
}

impl TempPackage {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(unique_name(name));
        let home = root.join(".nocter");
        fs::create_dir_all(home.join("std")).unwrap();
        fs::write(home.join("std/prelude.nct"), "").unwrap();
        Self { root, home }
    }

    fn write(&self, relative: &str, source: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, source).unwrap();
    }

    fn nocter<const N: usize>(&self, args: [&str; N]) -> Output {
        Command::new(NOCTER)
            .args(args)
            .current_dir(&self.root)
            .env("NOCTER_HOME", &self.home)
            .output()
            .unwrap()
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn assert_success(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn unique_name(name: &str) -> String {
    format!(
        "nocter-test-cli-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}
