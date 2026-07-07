use super::pipeline::check_file_with_entry;
use crate::diagnostics::write_text_diagnostics;
use std::io;
use std::path::Path;
use std::process::ExitCode;

pub(super) fn run_check(file: &Path, entry_name: &str) -> ExitCode {
    let output = check_file_with_entry(file, entry_name);

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
