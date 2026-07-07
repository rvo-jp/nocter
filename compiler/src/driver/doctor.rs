use crate::home::{resolve_nocter_home, validate_nocter_home};
use std::process::ExitCode;

pub(super) fn run_doctor() -> ExitCode {
    match resolve_nocter_home() {
        Ok(home) => {
            println!("Nocter home: {}", home.display());
            let errors = validate_nocter_home(&home);
            if errors.is_empty() {
                println!("ok");
                ExitCode::SUCCESS
            } else {
                for error in errors {
                    eprintln!("error: {error}");
                }
                install_error()
            }
        }
        Err(message) => {
            eprintln!("error: {message}");
            install_error()
        }
    }
}

fn install_error() -> ExitCode {
    ExitCode::from(2)
}
