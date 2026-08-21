use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new(label: &str) -> Self {
        loop {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "nocter-cli-{label}-{}-{serial}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("cannot create test directory: {error}"),
            }
        }
    }

    fn installation(&self, host: &str, complete_standard: bool) -> PathBuf {
        let root = self.0.join("home");
        fs::create_dir(&root).unwrap();
        let standard = root.join("std");
        if complete_standard {
            let compiler = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
            copy_directory(&compiler.join("../std"), &standard);
        } else {
            fs::create_dir(&standard).unwrap();
            fs::write(standard.join("nocter.nct"), "#name: \"std\"\n").unwrap();
        }
        fs::write(root.join("VERSION"), "0.14.0\n").unwrap();
        fs::write(root.join("MANIFEST.json"), manifest(host)).unwrap();
        fs::write(root.join("nocter"), "compiler").unwrap();
        fs::write(root.join("LICENSE"), "license").unwrap();
        fs::write(root.join("NOTICE"), "notice").unwrap();
        root
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn manifest(host: &str) -> String {
    format!(
        r#"{{
            "schema": "nocter.manifest",
            "schema_version": 1,
            "release": "0.14.0",
            "host": "{host}",
            "default_target": "arm64-darwin",
            "compiler": {{ "path": "nocter" }},
            "std": {{ "path": "std" }},
            "license": {{
                "id": "Apache-2.0",
                "path": "LICENSE",
                "notice": "NOTICE"
            }},
            "implemented_targets": [{{
                "name": "arm64-darwin",
                "backend": "arm64",
                "executable": "macho",
                "os": "darwin"
            }}],
            "archive": {{
                "name": "nocter-v0.14.0-{host}.tar.gz",
                "root": ".nocter"
            }}
        }}"#
    )
}

fn invocation(
    arguments: impl IntoIterator<Item = impl Into<OsString>>,
    directory: &Path,
    home: &Path,
    host: &str,
) -> Invocation {
    Invocation::new(
        arguments.into_iter().map(Into::into),
        directory,
        Some(home.as_os_str().to_owned()),
        home.join("nocter"),
        host,
    )
}

#[test]
fn argument_structure_precedes_installation_filesystem_access() {
    let tree = TempTree::new("argument-first");
    let missing = tree.0.join("missing-home");
    let error = execute_invocation(invocation(
        Vec::<&str>::new(),
        &tree.0,
        &missing,
        "arm64-darwin",
    ))
    .unwrap_err();

    assert!(matches!(error, InvocationError::Arguments(_)));
    assert_eq!(error.diagnostic_code(), Some("E0700"));
}

#[test]
fn compiler_host_is_checked_before_user_source_preparation() {
    let tree = TempTree::new("host");
    let home = tree.installation("arm64-darwin", false);
    let error = execute_invocation(invocation(
        ["build", "missing.nct"],
        &tree.0,
        &home,
        "x64-linux",
    ))
    .unwrap_err();

    assert!(matches!(error, InvocationError::HostMismatch { .. }));
    assert_eq!(error.diagnostic_code(), Some("E0703"));
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
#[test]
fn public_invocation_builds_through_the_installed_standard_library() {
    let tree = TempTree::new("build");
    let home = tree.installation("arm64-darwin", true);
    fs::write(tree.0.join("app.nct"), "func main(): i32 { return 0 }\n").unwrap();
    let output = tree.0.join("app-bin");
    let outcome = execute_invocation(invocation(
        [
            OsString::from("build"),
            OsString::from("app.nct"),
            OsString::from("-o"),
            output.as_os_str().to_owned(),
        ],
        &tree.0,
        &home,
        "arm64-darwin",
    ))
    .unwrap();

    assert!(matches!(&outcome, InvocationOutcome::Build(_)));
    assert_eq!(outcome.exit_code(), 0);
    assert_eq!(&fs::read(output).unwrap()[..4], &[0xcf, 0xfa, 0xed, 0xfe]);
}

fn copy_directory(source: &Path, destination: &Path) {
    fs::create_dir(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source = entry.path();
        let destination = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_directory(&source, &destination);
        } else {
            fs::copy(source, destination).unwrap();
        }
    }
}
