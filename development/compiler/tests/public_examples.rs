use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");
const PACKAGES: &[(&str, &str)] = &[("hello", "hello"), ("file-summary", "file-summary")];

#[test]
fn public_example_packages_pass_check() {
    let environment = ExampleEnvironment::new("check");

    for &(directory, _) in PACKAGES {
        let package = repo_root().join("examples").join(directory);
        let source = package.join("nocter.nct");
        let format = nocter(&environment)
            .args(["fmt", "--check", source.to_str().unwrap()])
            .output()
            .unwrap();
        assert_success(directory, &format);

        let output = nocter(&environment)
            .args([
                "check",
                "--root",
                package.to_str().unwrap(),
                "--locked",
                "--offline",
            ])
            .output()
            .unwrap();

        assert_success(directory, &output);
        assert!(output.stdout.is_empty(), "{directory} wrote stdout");
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_example_packages_build_and_run() {
    let environment = ExampleEnvironment::new("run");

    for &(directory, executable) in PACKAGES {
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

        let run = if directory == "file-summary" {
            let input = environment.root.join("input.txt");
            fs::write(&input, "alpha\nbeta\n").unwrap();
            Command::new(&output_path).arg(input).output().unwrap()
        } else {
            Command::new(&output_path).output().unwrap()
        };

        assert_success(directory, &run);
        let expected = if directory == "file-summary" {
            "2\n"
        } else {
            "Hello from Nocter\n"
        };
        assert_eq!(
            text(&run.stdout),
            expected,
            "unexpected output from {directory}"
        );
    }
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
