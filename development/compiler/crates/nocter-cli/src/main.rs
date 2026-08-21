use std::process;

fn main() {
    match nocter_cli::execute_current_process() {
        Ok(outcome) => process::exit(outcome.exit_code()),
        Err(error) => {
            match error.render_source_diagnostics() {
                Ok(Some(rendered)) => eprint!("{rendered}"),
                Ok(None) => {
                    if let Some(code) = error.diagnostic_code() {
                        eprintln!("error[{code}]: {error}");
                    } else {
                        eprintln!("error: {error}");
                    }
                }
                Err(render_error) => {
                    eprintln!("error: cannot render source diagnostic: {render_error}; {error}");
                }
            }
            process::exit(1);
        }
    }
}
