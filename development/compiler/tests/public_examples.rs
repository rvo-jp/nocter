use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

#[test]
fn public_examples_are_formatted() {
    let environment = ExampleEnvironment::new("format");
    let examples = repo_root().join("examples");

    for source in example_sources(&examples) {
        let name = source.strip_prefix(&examples).unwrap().to_string_lossy();
        let format = nocter(&environment)
            .args(["fmt", "--check", source.to_str().unwrap()])
            .output()
            .unwrap();
        assert_success(&name, &format);
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_hello_builds_and_runs() {
    assert_single_file_example("hello.nct", "Hello from Nocter\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_custom_format_builds_and_runs() {
    assert_single_file_example("custom-format.nct", "point = (3, 4)\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_equality_builds_and_runs() {
    assert_single_file_example("equality.nct", "equality found the point\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_indexing_builds_and_runs() {
    assert_single_file_example("indexing.nct", "");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_recovery_builds_and_runs() {
    assert_single_file_example("recovery.nct", "");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_mutable_iteration_builds_and_runs() {
    assert_single_file_example(
        "mutable-iteration.nct",
        "mutable iteration updated every element\n",
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_ordering_builds_and_runs() {
    assert_single_file_example(
        "ordering.nct",
        "strict ordering selected source declarations\n",
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_file_summary_builds_and_runs() {
    let directory = "file-summary";
    let executable = "file-summary";
    let environment = ExampleEnvironment::new(directory);
    let package = repo_root().join("examples").join(directory);
    let output_path = environment.root.join(executable);
    let output = nocter(&environment)
        .args([
            "build",
            "--root",
            package.to_str().unwrap(),
            "--executable",
            executable,
            "-o",
            output_path.to_str().unwrap(),
            "--locked",
            "--offline",
        ])
        .output()
        .unwrap();
    assert_success(directory, &output);

    let input = environment.root.join("input.txt");
    fs::write(&input, "alpha\nbeta\n").unwrap();
    let run = Command::new(&output_path).arg(input).output().unwrap();

    assert_success(directory, &run);
    assert_eq!(
        text(&run.stdout),
        "2\n",
        "unexpected output from {directory}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn assert_single_file_example(file: &str, expected_stdout: &str) {
    let environment = ExampleEnvironment::new(file.trim_end_matches(".nct"));
    let source = repo_root().join("examples").join(file);
    let output_path = environment.root.join(file.trim_end_matches(".nct"));
    let output = nocter(&environment)
        .args([
            "build",
            source.to_str().unwrap(),
            "-o",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_success(file, &output);

    let run = Command::new(&output_path).output().unwrap();
    assert_success(file, &run);
    assert_eq!(
        text(&run.stdout),
        expected_stdout,
        "unexpected output from {file}"
    );
}

fn nocter(environment: &ExampleEnvironment) -> Command {
    let mut command = Command::new(NOCTER);
    command
        .current_dir(&environment.root)
        .env("NOCTER_HOME", &environment.home);
    command
}

fn assert_success(name: &str, output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "example `{name}` failed\nstdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "example `{name}` wrote stderr:\n{}",
        text(&output.stderr)
    );
}

struct ExampleEnvironment {
    root: PathBuf,
    home: PathBuf,
}

impl ExampleEnvironment {
    fn new(suffix: &str) -> Self {
        let root = std::env::temp_dir().join(unique_name(&format!("public-examples-{suffix}")));
        let home = root.join(".nocter");
        fs::create_dir_all(&home).unwrap();

        let repository = repo_root();
        copy_tree(&repository.join("development/std"), &home.join("std"));
        for file in ["VERSION", "MANIFEST.json"] {
            fs::copy(
                repository.join("development/packaging").join(file),
                home.join(file),
            )
            .unwrap();
        }
        for file in ["LICENSE", "NOTICE"] {
            fs::copy(repository.join(file), home.join(file)).unwrap();
        }

        Self { root, home }
    }
}

impl Drop for ExampleEnvironment {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn example_sources(root: &Path) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    collect_example_sources(root, &mut sources);
    sources.sort();
    sources
}

fn collect_example_sources(directory: &Path, sources: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            collect_example_sources(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "nct") {
            sources.push(path);
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("compiler crate should live below development root")
        .to_path_buf()
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

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
