use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

#[path = "support/builtin_std.rs"]
mod builtin_std;

#[test]
fn package_check_accepts_code_in_the_package_file() {
    let project = TempPackage::new("library");
    project.write(
        "nocter.nct",
        "#name: \"library\"\n\npub func value(): i32 { 7 }\n",
    );
    assert_success(&project.nocter(["check"]));
}

#[test]
fn init_creates_a_checkable_library_without_overwriting() {
    let project = TempPackage::new("init-library");
    let output = project.nocter(["init", "created", "--name", "sample", "--library"]);
    assert_success(&output);
    let package = project.root.join("created/nocter.nct");
    let source = fs::read_to_string(&package).unwrap();
    assert!(source.contains("#name: \"sample\"") && source.contains("#test:"));

    let checked = Command::new(NOCTER)
        .args(["check"])
        .current_dir(package.parent().unwrap())
        .env("NOCTER_HOME", &project.home)
        .output()
        .unwrap();
    assert_success(&checked);

    let repeated = project.nocter(["init", "created", "--library"]);
    assert_eq!(repeated.status.code(), Some(2));
    assert!(text(&repeated.stderr).contains("already exists"));
}

#[test]
fn graph_json_is_deterministic_and_exposes_dependency_identity() {
    let project = TempPackage::new("graph-json");
    project.write("dep/nocter.nct", "#name: \"dep\"\n#version: \"1.0.0\"\n");
    project.write(
        "nocter.nct",
        "#name: \"root\"\n#dependencies: { dep: { path: \"./dep\" } }\n",
    );
    let manifest_before = fs::read(project.root.join("nocter.nct")).unwrap();
    let first = project.nocter(["graph", "--format", "json"]);
    let second = project.nocter(["graph", "--format", "json"]);
    assert_success(&first);
    assert_eq!(first.stdout, second.stdout);
    let value: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(value["format"], 1);
    assert_eq!(value["packages"].as_array().unwrap().len(), 2);
    assert!(text(&first.stdout).contains("\"source\":\"path\""));
    assert_eq!(
        fs::read(project.root.join("nocter.nct")).unwrap(),
        manifest_before
    );
}

#[test]
fn package_build_uses_declared_entry_and_artifact_name() {
    let project = TempPackage::new("build");
    project.write(
        "nocter.nct",
        r#"#name: "tool-package"
#executable: {
    name: "tool",
    entry: "./src/app",
}
"#,
    );
    project.write("src/app.nct", "func main(): i32 { 0 }\n");
    assert_success(&project.nocter(["build"]));
    assert!(project.root.join("tool").is_file());
}

#[test]
fn package_build_uses_the_root_module_when_entry_is_omitted() {
    let project = TempPackage::new("root-entry");
    project.write(
        "nocter.nct",
        r#"#executable: {
    name: "app",
}

func main(): i32 { 0 }
"#,
    );

    assert_success(&project.nocter(["build"]));
    assert!(project.root.join("app").is_file());
}

#[test]
fn package_check_analyzes_a_root_executable_once() {
    let project = TempPackage::new("root-entry-check-plan");
    project.write(
        "nocter.nct",
        r#"#executable: { name: "app" }

func main(value: i32): i32 { value }
"#,
    );

    let output = project.nocter(["check"]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert_eq!(stderr.matches("error[").count(), 1, "{stderr}");
}

#[test]
fn dot_entry_selects_index_nct() {
    let project = TempPackage::new("index-entry");
    project.write(
        "nocter.nct",
        "#executable: { name: \"app\", entry: \".\" }\n",
    );
    project.write("index.nct", "func main(): i32 { 0 }\n");

    assert_success(&project.nocter(["check", "--executable", "app"]));
}

#[test]
fn package_build_never_discovers_undeclared_main_file() {
    let project = TempPackage::new("no-main-fallback");
    project.write("nocter.nct", "#name: \"library\"\n");
    project.write("main.nct", "func main(): i32 { 0 }\n");
    let output = project.nocter(["build"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).contains("declares no executable target"),
        "{}",
        text(&output.stderr)
    );
    assert!(!project.root.join("main").exists());
}

#[test]
fn package_check_resolves_a_declared_path_dependency_namespace() {
    let project = TempPackage::new("path-dependency");
    project.write(
        "nocter.nct",
        r#"#dependencies: {
    math: { path: "./packages/math" },
}

use math.answer

pub func value(): i32 {
    answer()
}
"#,
    );
    project.write(
        "packages/math/nocter.nct",
        "pub func answer(): i32 { 42 }\n",
    );
    assert_success(&project.nocter(["check"]));
}

#[test]
fn fetch_generates_a_git_lock_and_offline_check_reuses_the_exact_package() {
    let project = TempPackage::new("git-dependency");
    let repository = project.root.join("dependency-repository");
    fs::create_dir_all(&repository).unwrap();
    fs::write(
        repository.join("nocter.nct"),
        "pub func answer(): i32 { 42 }\n",
    )
    .unwrap();
    git(&repository, ["init", "--quiet"]);
    git(&repository, ["config", "user.email", "test@nocter.dev"]);
    git(&repository, ["config", "user.name", "Nocter Test"]);
    git(&repository, ["add", "nocter.nct"]);
    git(&repository, ["commit", "--quiet", "-m", "initial"]);

    project.write(
        "nocter.nct",
        &format!(
            r#"#dependencies: {{
    math: {{
        git: "{}",
        revision: "HEAD",
    }},
}}

use math.answer

pub func value(): i32 {{
    answer()
}}
"#,
            repository.display()
        ),
    );

    assert_success(&project.nocter(["fetch"]));
    let manifest = fs::read_to_string(project.root.join("nocter.nct")).unwrap();
    assert!(manifest.contains("#lock: {"), "{manifest}");
    assert!(manifest.contains("math: \"git:"), "{manifest}");
    assert_success(&project.nocter(["check", "--locked", "--offline"]));
}

#[test]
fn fetch_generates_and_verifies_an_archive_content_lock() {
    let project = TempPackage::new("archive-dependency");
    let archive_source = project.root.join("archive-source");
    fs::create_dir_all(&archive_source).unwrap();
    fs::write(
        archive_source.join("nocter.nct"),
        "pub func answer(): i32 { 42 }\n",
    )
    .unwrap();
    let archive = project.root.join("math.tar.gz");
    let output = Command::new("tar")
        .args(["-czf"])
        .arg(&archive)
        .arg("-C")
        .arg(&archive_source)
        .arg(".")
        .output()
        .unwrap();
    assert!(output.status.success());
    project.write(
        "nocter.nct",
        &format!(
            r#"#dependencies: {{
    math: {{ archive: "file://{}" }},
}}

use math.answer

pub func value(): i32 {{ answer() }}
"#,
            archive.canonicalize().unwrap().display()
        ),
    );

    assert_success(&project.nocter(["fetch"]));
    let manifest = fs::read_to_string(project.root.join("nocter.nct")).unwrap();
    assert!(manifest.contains("math: \"sha256:"), "{manifest}");
    assert_success(&project.nocter(["check", "--locked", "--offline"]));
}

#[test]
fn failed_graph_does_not_write_a_partial_generated_lock() {
    let project = TempPackage::new("failed-graph-lock-transaction");
    let repository = project.root.join("dependency-repository");
    fs::create_dir_all(&repository).unwrap();
    fs::write(
        repository.join("nocter.nct"),
        "pub func value(): i32 { 1 }\n",
    )
    .unwrap();
    git(&repository, ["init", "--quiet"]);
    git(&repository, ["config", "user.email", "test@nocter.dev"]);
    git(&repository, ["config", "user.name", "Nocter Test"]);
    git(&repository, ["add", "nocter.nct"]);
    git(&repository, ["commit", "--quiet", "-m", "initial"]);
    project.write(
        "nocter.nct",
        &format!(
            r#"#dependencies: {{
    available: {{ git: "{}", revision: "HEAD" }},
    missing: {{ path: "./missing" }},
}}
"#,
            repository.display()
        ),
    );

    let output = project.nocter(["fetch"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output.stderr).contains("cannot be resolved"));
    let manifest = fs::read_to_string(project.root.join("nocter.nct")).unwrap();
    assert!(!manifest.contains("#lock:"), "{manifest}");
}

#[test]
fn offline_graph_requires_the_exact_locked_package_in_a_store() {
    let project = TempPackage::new("offline-cache-miss");
    project.write(
        "nocter.nct",
        &format!(
            r#"#dependencies: {{
    missing: {{ git: "https://example.invalid/missing.git" }},
}}
#lock: {{
    format: 1,
    dependencies: {{
        missing: "git:{}",
    }},
}}
"#,
            "0".repeat(40)
        ),
    );

    let output = project.nocter(["fetch", "--offline"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).contains("is not cached for offline use"),
        "{}",
        text(&output.stderr)
    );
}

#[test]
fn dependency_source_and_lock_kinds_must_match() {
    let project = TempPackage::new("source-lock-mismatch");
    project.write(
        "nocter.nct",
        &format!(
            r#"#dependencies: {{
    data: {{ archive: "https://example.invalid/data.tar.gz" }},
}}
#lock: {{
    format: 1,
    dependencies: {{
        data: "git:{}",
    }},
}}
"#,
            "0".repeat(40)
        ),
    );

    let output = project.nocter(["fetch"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).contains("source and lock kinds do not match"),
        "{}",
        text(&output.stderr)
    );
}

#[test]
fn path_dependency_cycles_are_rejected() {
    let project = TempPackage::new("path-cycle");
    project.write(
        "nocter.nct",
        "#dependencies: { child: { path: \"./packages/child\" } }\n",
    );
    project.write(
        "packages/child/nocter.nct",
        "#dependencies: { parent: { path: \"../..\" } }\n",
    );

    let output = project.nocter(["fetch"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).contains("dependency cycle reaches package"),
        "{}",
        text(&output.stderr)
    );
}

#[test]
fn package_directives_are_rejected_outside_nocter_file() {
    let project = TempPackage::new("directive-location");
    project.write(
        "nocter.nct",
        r#"#executable: { name: "app", entry: "./app" }
"#,
    );
    project.write("app.nct", "#name: \"nested\"\n\nfunc main(): i32 { 0 }\n");

    let output = project.nocter(["check", "--executable", "app"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).contains("valid only in a package-root `nocter.nct`"),
        "{}",
        text(&output.stderr)
    );
}

#[test]
fn package_run_requires_selection_for_multiple_executables() {
    let project = TempPackage::new("selection");
    project.write(
        "nocter.nct",
        r#"#executable: {
    name: "first",
    entry: "./first",
}
#executable: {
    name: "second",
    entry: "./second",
}
"#,
    );
    project.write("first.nct", "func main(): i32 { 0 }\n");
    project.write("second.nct", "func main(): i32 { 0 }\n");
    let output = project.nocter(["run"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output.stderr).contains("use `--executable <name>`"));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn package_run_executes_selected_declared_entry() {
    let project = TempPackage::new("run");
    project.write(
        "nocter.nct",
        r#"#executable: {
    name: "tool",
    entry: "./app",
}
"#,
    );
    project.write("app.nct", "func main(): i32 { 7 }\n");
    let output = project.nocter(["run"]);
    assert_eq!(output.status.code(), Some(7), "{}", text(&output.stderr));
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
        builtin_std::write_builtin_type_surfaces(&home);
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

fn git<const N: usize>(repository: &PathBuf, arguments: [&str; N]) {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repository)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn unique_name(name: &str) -> String {
    format!(
        "nocter-package-cli-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}
