use super::errors::{exit_for_diagnostics, write_human_diagnostics};
use super::pipeline::check_file_with_entry_and_target;
use std::path::Path;
use std::process::ExitCode;

pub(super) fn run_check(file: &Path, entry_name: &str, target: &str) -> ExitCode {
    let output = check_file_with_entry_and_target(file, entry_name, target);

    if output.is_ok() {
        return ExitCode::SUCCESS;
    }

    let exit = exit_for_diagnostics(&output.diagnostics, ExitCode::FAILURE);
    write_human_diagnostics(&output.diagnostics, Some(&output.sources), exit)
}
