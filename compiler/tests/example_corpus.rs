use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

const VALID_EXAMPLES: &[&str] = &[
    "spec/examples/valid/hello.nct",
    "spec/examples/valid/fallible-catch.nct",
    "spec/examples/valid/optional-default.nct",
];

const INVALID_EXAMPLES: &[InvalidExample] = &[
    InvalidExample {
        path: "spec/examples/invalid/main-entry.nct",
        error_code: "E0303",
    },
    InvalidExample {
        path: "spec/examples/invalid/module-declaration.nct",
        error_code: "E0200",
    },
    InvalidExample {
        path: "spec/examples/invalid/optional-propagation.nct",
        error_code: "E0335",
    },
    InvalidExample {
        path: "spec/examples/invalid/return-type-mismatch.nct",
        error_code: "E0312",
    },
];

struct InvalidExample {
    path: &'static str,
    error_code: &'static str,
}

#[test]
fn valid_example_corpus_passes_check() {
    let project = TempProject::new("example-corpus-valid");

    for example in VALID_EXAMPLES {
        let source = repo_root().join(example);
        let output = check(&project, &source);

        assert_eq!(
            output.status.code(),
            Some(0),
            "valid example `{}` failed\nstdout:\n{}\nstderr:\n{}",
            example,
            text(&output.stdout),
            text(&output.stderr)
        );
        assert!(
            output.stdout.is_empty(),
            "valid example `{}` wrote stdout:\n{}",
            example,
            text(&output.stdout)
        );
        assert!(
            output.stderr.is_empty(),
            "valid example `{}` wrote stderr:\n{}",
            example,
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

fn check(project: &TempProject, source: &Path) -> Output {
    Command::new(NOCTER)
        .args(["check", source.to_str().unwrap()])
        .current_dir(project.root())
        .env("NOCTER_HOME", project.nocter_home())
        .output()
        .unwrap()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("compiler crate should live below repository root")
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
        fs::create_dir_all(home.join("targets/arm64-darwin/std")).unwrap();

        fs::write(
            home.join("std/prelude.nct"),
            concat!(
                "pub type Int = i32\n",
                "pub from std/error import Error, ErrorCode\n",
                "pub from std/string import String\n",
            ),
        )
        .unwrap();
        fs::write(
            home.join("std/error.nct"),
            concat!(
                "pub type ErrorCode = str\n",
                "pub type Error = error\n",
                "pub(nocter) primitive make_error(code: ErrorCode, message: str): error\n",
            ),
        )
        .unwrap();
        fs::write(
            home.join("std/string.nct"),
            concat!("pub struct String {\n", "    bytes: [u8]\n", "}\n",),
        )
        .unwrap();
        fs::write(
            home.join("std/io.nct"),
            concat!(
                "pub func print(text: str): void! {\n",
                "    return\n",
                "}\n",
            ),
        )
        .unwrap();
        fs::write(
            home.join("targets/arm64-darwin/std/process.nct"),
            concat!(
                "pub(nocter) primitive env_impl(name: str): (str?)!\n",
                "\n",
                "pub func env(name: str): (str?)! {\n",
                "    return env_impl(name)?\n",
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
