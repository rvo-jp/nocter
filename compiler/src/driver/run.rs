use super::pipeline::build_file_to_path_with_entry;
use crate::diagnostics::write_text_diagnostics;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn run_file(file: &Path, entry_name: &str) -> ExitCode {
    let artifact = match RunArtifact::new() {
        Ok(artifact) => artifact,
        Err(error) => {
            eprintln!("error: failed to prepare run artifact: {error}");
            return ExitCode::FAILURE;
        }
    };

    let output = build_file_to_path_with_entry(file, artifact.executable_path(), entry_name);
    if !output.is_ok() {
        let mut stderr = io::stderr().lock();
        if let Err(error) = write_text_diagnostics(&mut stderr, &output.diagnostics) {
            eprintln!("internal compiler error: failed to write diagnostics: {error}");
            return ExitCode::from(3);
        }
        return ExitCode::FAILURE;
    }

    let status = match Command::new(&output.output_path).status() {
        Ok(status) => status,
        Err(error) => {
            eprintln!(
                "error: failed to run `{}`: {error}",
                output.output_path.display()
            );
            return ExitCode::FAILURE;
        }
    };

    exit_code_from_status(status)
}

fn exit_code_from_status(status: ExitStatus) -> ExitCode {
    match status.code().and_then(|code| u8::try_from(code).ok()) {
        Some(code) => ExitCode::from(code),
        None => ExitCode::FAILURE,
    }
}

#[derive(Debug)]
struct RunArtifact {
    root: PathBuf,
    executable: PathBuf,
}

impl RunArtifact {
    fn new() -> io::Result<Self> {
        let root = std::env::temp_dir().join(unique_run_dir_name());
        fs::create_dir_all(&root)?;
        let executable = root.join("app");

        Ok(Self { root, executable })
    }

    fn executable_path(&self) -> &Path {
        &self.executable
    }
}

impl Drop for RunArtifact {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.executable);
        let _ = fs::remove_dir(&self.root);
    }
}

fn unique_run_dir_name() -> String {
    format!(
        "nocter-run-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_artifact_uses_unique_temp_executable_path() {
        let artifact = RunArtifact::new().unwrap();

        assert!(artifact.executable_path().ends_with("app"));
        assert!(artifact.root.is_dir());
    }
}
