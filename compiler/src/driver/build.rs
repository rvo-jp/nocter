use super::pipeline::{build_file_to_path_with_entry_and_target, build_file_with_entry_and_target};
use crate::diagnostics::write_text_diagnostics;
use std::io;
use std::path::Path;
use std::process::ExitCode;

pub(super) fn run_build(
    file: &Path,
    entry_name: &str,
    target: &str,
    output_path: Option<&Path>,
) -> ExitCode {
    let output = match output_path {
        Some(output_path) => {
            build_file_to_path_with_entry_and_target(file, output_path, entry_name, target)
        }
        None => build_file_with_entry_and_target(file, entry_name, target),
    };

    if output.is_ok() {
        return ExitCode::SUCCESS;
    }

    let mut stderr = io::stderr().lock();
    if let Err(error) = write_text_diagnostics(&mut stderr, &output.diagnostics) {
        eprintln!("internal compiler error: failed to write diagnostics: {error}");
        return ExitCode::from(3);
    }

    ExitCode::FAILURE
}
