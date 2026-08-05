use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

#[test]
fn package_check_accepts_a_library_only_index() {
    let project = TempPackage::new("library");
    project.write(
        "index.nct",
        "#name: \"library\"\n\npub func value(): i32 { 7 }\n",
    );
    assert_success(&project.nocter(["check"]));
}

#[test]
fn package_build_uses_declared_module_and_artifact_name() {
    let project = TempPackage::new("build");
    project.write(
        "index.nct",
        r#"#name: "tool-package"
#executable: {
    name: "tool",
    module: "./src/app",
}
"#,
    );
    project.write("src/app.nct", "func main(): i32 { 0 }\n");
    assert_success(&project.nocter(["build"]));
    assert!(project.root.join("tool").is_file());
}

#[test]
fn package_build_never_discovers_undeclared_main_file() {
    let project = TempPackage::new("no-main-fallback");
    project.write("index.nct", "#name: \"library\"\n");
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
fn package_directives_are_rejected_outside_root_index() {
    let project = TempPackage::new("directive-location");
    project.write(
        "index.nct",
        r#"#executable: { name: "app", module: "./app" }
"#,
    );
    project.write("app.nct", "#name: \"nested\"\n\nfunc main(): i32 { 0 }\n");

    let output = project.nocter(["check", "--executable", "app"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).contains("package directives are only allowed"),
        "{}",
        text(&output.stderr)
    );
}

#[test]
fn package_run_requires_selection_for_multiple_executables() {
    let project = TempPackage::new("selection");
    project.write(
        "index.nct",
        r#"#executable: {
    name: "first",
    module: "./first",
}
#executable: {
    name: "second",
    module: "./second",
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
fn package_run_executes_selected_declared_module() {
    let project = TempPackage::new("run");
    project.write(
        "index.nct",
        r#"#executable: {
    name: "tool",
    module: "./app",
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
        "nocter-package-cli-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}
