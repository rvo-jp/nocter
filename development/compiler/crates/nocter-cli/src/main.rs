use std::process;

fn main() {
    match nocter_cli::execute_current_process() {
        Ok(outcome) => process::exit(outcome.exit_code()),
        Err(error) => {
            if let Some(code) = error.diagnostic_code() {
                eprintln!("error[{code}]: {error}");
            } else {
                eprintln!("error: {error}");
            }
            process::exit(1);
        }
    }
}
