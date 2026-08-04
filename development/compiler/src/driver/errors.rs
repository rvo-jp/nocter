use crate::diagnostics::{Diagnostic, write_text_diagnostics, write_text_diagnostics_with_sources};
use crate::source::SourceMap;
use std::io;
use std::path::Path;
use std::process::ExitCode;

const COMMAND_LINE_ERROR: &str = "E0700";
const TARGET_SELECTION_ERROR: &str = "E0701";
const FILESYSTEM_ERROR: &str = "E0702";
const NOCTER_HOME_ERROR: &str = "E0703";
const TEMPORARY_EXECUTABLE_ERROR: &str = "E0704";
const FORMAT_DIFFERENCE: &str = "E0602";

pub(super) fn command_line_diagnostic(message: impl Into<String>) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(COMMAND_LINE_ERROR, message);
    diagnostic.help = Some("run `nocter help` to see supported commands and options".to_string());
    diagnostic
}

pub(super) fn target_selection_diagnostic(message: impl Into<String>) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(TARGET_SELECTION_ERROR, message);
    diagnostic.help =
        Some("use the currently supported target `--target arm64-darwin`".to_string());
    diagnostic
}

pub(super) fn filesystem_diagnostic(message: impl Into<String>) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(FILESYSTEM_ERROR, message);
    diagnostic.help = Some("check the path and filesystem permissions".to_string());
    diagnostic
}

pub(super) fn nocter_home_diagnostic(message: impl Into<String>) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(NOCTER_HOME_ERROR, message);
    diagnostic.help = Some(
        "set `NOCTER_HOME` to the active Nocter home, or run `nocter` through a symlink to the installed `.nocter/nocter` binary"
            .to_string(),
    );
    diagnostic
}

pub(super) fn temporary_executable_diagnostic(message: impl Into<String>) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(TEMPORARY_EXECUTABLE_ERROR, message);
    diagnostic.help = Some("check the temporary directory and executable permissions".to_string());
    diagnostic
}

pub(super) fn format_difference_diagnostic(file: &Path) -> Diagnostic {
    let display = file.display();
    let mut diagnostic =
        Diagnostic::error(FORMAT_DIFFERENCE, format!("`{display}` is not formatted"));
    diagnostic.help = Some(format!("run `nocter fmt {display}`"));
    diagnostic
}

pub(super) fn write_human_diagnostics(
    diagnostics: &[Diagnostic],
    sources: Option<&SourceMap>,
    failure_exit: ExitCode,
) -> ExitCode {
    let mut stderr = io::stderr().lock();
    let result = match sources {
        Some(sources) => write_text_diagnostics_with_sources(&mut stderr, diagnostics, sources),
        None => write_text_diagnostics(&mut stderr, diagnostics),
    };

    if let Err(error) = result {
        eprintln!("internal compiler error: failed to write diagnostics: {error}");
        return internal_error_exit();
    }

    failure_exit
}

pub(super) fn exit_for_diagnostics(
    diagnostics: &[Diagnostic],
    default_failure_exit: ExitCode,
) -> ExitCode {
    if diagnostics
        .iter()
        .any(|diagnostic| is_cli_environment_error(&diagnostic.code))
    {
        ExitCode::from(2)
    } else {
        default_failure_exit
    }
}

pub(super) fn internal_error_exit() -> ExitCode {
    ExitCode::from(3)
}

fn is_cli_environment_error(code: &str) -> bool {
    matches!(
        code,
        COMMAND_LINE_ERROR
            | TARGET_SELECTION_ERROR
            | FILESYSTEM_ERROR
            | NOCTER_HOME_ERROR
            | TEMPORARY_EXECUTABLE_ERROR
    )
}
