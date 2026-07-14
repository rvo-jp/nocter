use super::pipeline::check_file_with_entry_and_target;
use crate::diagnostics::write_text_diagnostics_with_sources;
use std::io;
use std::path::Path;
use std::process::ExitCode;

pub(super) fn run_check(file: &Path, entry_name: &str, target: &str) -> ExitCode {
    let output = check_file_with_entry_and_target(file, entry_name, target);

    if output.is_ok() {
        return ExitCode::SUCCESS;
    }

    let mut stderr = io::stderr().lock();
    if let Err(error) =
        write_text_diagnostics_with_sources(&mut stderr, &output.diagnostics, &output.sources)
    {
        eprintln!("internal compiler error: failed to write diagnostics: {error}");
        return ExitCode::from(3);
    }

    ExitCode::FAILURE
}
