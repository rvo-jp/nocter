use super::errors::{
    exit_for_diagnostics, filesystem_diagnostic, format_difference_diagnostic,
    write_human_diagnostics,
};
use crate::format::format_source;
use crate::source::SourceMap;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

pub(super) fn run_fmt(file: &Path, check: bool) -> ExitCode {
    let mut sources = SourceMap::new();
    let source = match sources.load_file(file) {
        Ok(source) => source,
        Err(diagnostic) => {
            return write_diagnostics_and_fail(&[diagnostic], None);
        }
    };

    let output = format_source(&sources, source);
    if !output.is_ok() {
        return write_diagnostics_and_fail(&output.diagnostics, Some(&sources));
    }

    let formatted = output
        .formatted
        .expect("successful format output must include formatted source");
    let original = sources
        .get(source)
        .expect("loaded source id must resolve in source map")
        .text();

    if original == formatted {
        return ExitCode::SUCCESS;
    }

    if check {
        let diagnostic = format_difference_diagnostic(file);
        return write_human_diagnostics(&[diagnostic], None, ExitCode::FAILURE);
    }

    match fs::write(file, formatted) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let diagnostic = filesystem_diagnostic(format!(
                "failed to write formatted source `{}`: {error}",
                file.display()
            ));
            write_human_diagnostics(&[diagnostic], None, ExitCode::from(2))
        }
    }
}

fn write_diagnostics_and_fail(
    diagnostics: &[crate::diagnostics::Diagnostic],
    sources: Option<&SourceMap>,
) -> ExitCode {
    let exit = exit_for_diagnostics(diagnostics, ExitCode::FAILURE);
    write_human_diagnostics(diagnostics, sources, exit)
}
