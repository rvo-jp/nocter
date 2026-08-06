use super::errors::{
    exit_for_diagnostics, temporary_executable_diagnostic, write_human_diagnostics,
};
use super::pipeline::build_file_to_path_with_target;
use super::pipeline::build_package_executable_to_path_with_target;
use super::temporary_executable::TemporaryExecutable;
use std::path::Path;
use std::process::{Command, ExitCode, ExitStatus};

pub(super) fn run_file(file: &Path, target: &str) -> ExitCode {
    run_with_builder(|output| build_file_to_path_with_target(file, output, target))
}

pub(super) fn run_package_file(
    file: &Path,
    package_graph: &crate::package::PackageGraph,
    target: &str,
) -> ExitCode {
    run_with_builder(|output| {
        build_package_executable_to_path_with_target(file, package_graph, output, target)
    })
}

fn run_with_builder(build: impl FnOnce(&Path) -> super::pipeline::BuildOutput) -> ExitCode {
    let artifact = match TemporaryExecutable::new("run") {
        Ok(artifact) => artifact,
        Err(error) => {
            let diagnostic =
                temporary_executable_diagnostic(format!("failed to prepare run artifact: {error}"));
            return write_human_diagnostics(&[diagnostic], None, ExitCode::from(2));
        }
    };

    let output = build(artifact.path());
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
