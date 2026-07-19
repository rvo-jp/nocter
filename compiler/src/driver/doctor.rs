use super::errors::{nocter_home_diagnostic, write_human_diagnostics};
use crate::home::{resolve_nocter_home, validate_nocter_home};
use std::process::ExitCode;

pub(super) fn run_doctor() -> ExitCode {
    match resolve_nocter_home() {
        Ok(home) => {
            let errors = validate_nocter_home(&home);
            if errors.is_empty() {
                println!("Nocter home: {}", home.display());
                println!("ok");
                ExitCode::SUCCESS
            } else {
                let diagnostics = errors
                    .into_iter()
                    .map(nocter_home_diagnostic)
                    .collect::<Vec<_>>();
                write_human_diagnostics(&diagnostics, None, install_error())
            }
        }
        Err(message) => {
            let diagnostic = nocter_home_diagnostic(message);
            write_human_diagnostics(&[diagnostic], None, install_error())
        }
    }
}

fn install_error() -> ExitCode {
    ExitCode::from(2)
}
