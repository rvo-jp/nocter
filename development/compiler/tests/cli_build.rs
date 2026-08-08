use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

#[path = "support/builtin_std.rs"]
mod builtin_std;

#[path = "cli_build/aggregates.rs"]
mod aggregates;
#[path = "cli_build/arrays.rs"]
mod arrays;
#[path = "cli_build/calls.rs"]
mod calls;
#[path = "cli_build/commands.rs"]
mod commands;
#[path = "cli_build/control_flow.rs"]
mod control_flow;
#[path = "cli_build/diagnostics.rs"]
mod diagnostics;
#[path = "cli_build/drops.rs"]
mod drops;
#[path = "cli_build/entry.rs"]
mod entry;
#[path = "cli_build/generics.rs"]
mod generics;
#[path = "cli_build/imports.rs"]
mod imports;
#[path = "cli_build/optional_fallible.rs"]
mod optional_fallible;
#[path = "cli_build/payload_enums.rs"]
mod payload_enums;
#[path = "cli_build/scalars.rs"]
mod scalars;
#[path = "cli_build/slices_strings_pointers.rs"]
mod slices_strings_pointers;

fn nocter<const N: usize>(project: &TempProject, args: [&str; N]) -> Output {
    Command::new(NOCTER)
        .args(args)
        .current_dir(project.root())
        .env("NOCTER_HOME", project.nocter_home())
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

fn assert_macho_executable(path: &Path) {
    let bytes = fs::read(path).unwrap();
    assert_eq!(read_u32(&bytes, 0), 0xfeed_facf);
    assert_eq!(read_u32(&bytes, 4), 0x0100_000c);
    assert_eq!(read_u32(&bytes, 12), 0x2);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    let mut value = [0; 4];
    value.copy_from_slice(&bytes[offset..offset + 4]);
    u32::from_le_bytes(value)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

struct TempProject {
    root: PathBuf,
}

fn write_process_contract_std(project: &TempProject) {
    project.write_nocter_home_file(
        "std/error/index.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    project.write_nocter_home_file(
        "std/vec/index.nct",
        r#"pub struct Vec<T> {
    len: usize
}
"#,
    );
    project.write_nocter_home_file(
        "std/process/index.nct",
        r#"use std/error.Error
use std/vec.Vec

pub func args(): Vec<&str>! {
    return Error.new("std.process.unsupported", "process arguments are not implemented")
}

pub func env(name: &str): &str?! {
    return Error.new("std.process.unsupported", "process environment is not implemented")
}
"#,
    );
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

    fn write_source(&self, name: &str, text: &str) -> PathBuf {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, text).unwrap();
        path
    }

    fn write_nocter_home_file(&self, relative: &str, text: &str) {
        let path = self.nocter_home().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }

    fn write_nocter_home(&self) {
        let home = self.nocter_home();
        fs::create_dir_all(home.join("std/prelude")).unwrap();
        fs::write(home.join("std/prelude/index.nct"), "").unwrap();
        builtin_std::write_builtin_type_surfaces(&home);
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
