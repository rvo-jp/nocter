use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

#[path = "support/builtin_std.rs"]
mod builtin_std;

#[path = "cli_run/aggregates.rs"]
mod aggregates;
#[path = "cli_run/arrays.rs"]
mod arrays;
#[path = "cli_run/associated_types.rs"]
mod associated_types;
#[path = "cli_run/calls.rs"]
mod calls;
#[path = "cli_run/closures.rs"]
mod closures;
#[path = "cli_run/coercions.rs"]
mod coercions;
#[path = "cli_run/commands.rs"]
mod commands;
#[path = "cli_run/control_flow.rs"]
mod control_flow;
#[path = "cli_run/control_flow_drop_state.rs"]
mod control_flow_drop_state;
#[path = "cli_run/diagnostics.rs"]
mod diagnostics;
#[path = "cli_run/drops.rs"]
mod drops;
#[path = "cli_run/entry.rs"]
mod entry;
#[path = "cli_run/generics.rs"]
mod generics;
#[path = "cli_run/imports.rs"]
mod imports;
#[path = "cli_run/optional_fallible.rs"]
mod optional_fallible;
#[path = "cli_run/payload_enums.rs"]
mod payload_enums;
#[path = "cli_run/process_context.rs"]
mod process_context;
#[path = "cli_run/scalars.rs"]
mod scalars;
#[path = "cli_run/slices_strings_pointers.rs"]
mod slices_strings_pointers;

fn nocter<const N: usize>(project: &TempProject, args: [&str; N]) -> Output {
    let mut command = Command::new(NOCTER);
    command
        .args(args)
        .current_dir(project.root())
        .env("NOCTER_HOME", project.nocter_home());

    command.output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn write_process_exit_home(project: &TempProject) {
    project.write_nocter_home_file(
        "std/process/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive exit_raw(code: i32): never

pub func exit(code: i32): never {
    exit_raw(code)
}
"#,
    );
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
