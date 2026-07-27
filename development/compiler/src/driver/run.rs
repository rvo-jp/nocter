use super::errors::{
    exit_for_diagnostics, temporary_executable_diagnostic, write_human_diagnostics,
};
use super::pipeline::build_file_to_path_with_target;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn run_file(file: &Path, target: &str) -> ExitCode {
    let artifact = match RunArtifact::new() {
        Ok(artifact) => artifact,
        Err(error) => {
            let diagnostic =
                temporary_executable_diagnostic(format!("failed to prepare run artifact: {error}"));
            return write_human_diagnostics(&[diagnostic], None, ExitCode::from(2));
        }
    };

    let output = build_file_to_path_with_target(file, artifact.executable_path(), target);
    if !output.is_ok() {
        let exit = exit_for_diagnostics(&output.diagnostics, ExitCode::FAILURE);
        return write_human_diagnostics(&output.diagnostics, Some(&output.sources), exit);
    }

    let status = match Command::new(&output.output_path).status() {
        Ok(status) => status,
        Err(error) => {
            let diagnostic = temporary_executable_diagnostic(format!(
                "failed to run `{}`: {error}",
                output.output_path.display()
            ));
            return write_human_diagnostics(&[diagnostic], None, ExitCode::from(2));
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
