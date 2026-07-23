use super::errors::{exit_for_diagnostics, write_human_diagnostics};
use super::pipeline::check_file_with_target;
use std::path::Path;
use std::process::ExitCode;

pub(super) fn run_check(file: &Path, target: &str) -> ExitCode {
    let output = check_file_with_target(file, target);

    if output.is_ok() {
        return ExitCode::SUCCESS;
    }

    let exit = exit_for_diagnostics(&output.diagnostics, ExitCode::FAILURE);
    write_human_diagnostics(&output.diagnostics, Some(&output.sources), exit)
}
