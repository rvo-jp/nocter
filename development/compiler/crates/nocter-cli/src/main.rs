use std::process;

fn main() {
    match nocter_cli::execute_current_process() {
        Ok(outcome) => {
            match outcome.render_json_diagnostics() {
                Ok(Some(rendered)) => print!("{rendered}"),
                Ok(None) => {}
                Err(error) => {
                    eprintln!("error: cannot render JSON diagnostic: {error}");
                    process::exit(3);
                }
            }
            process::exit(outcome.exit_code());
        }
        Err(error) => {
            match error.render_json_diagnostics() {
                Ok(Some(rendered)) => {
                    print!("{rendered}");
                    process::exit(error.exit_code());
                }
                Ok(None) => {}
                Err(render_error) => {
                    eprintln!("error: cannot render JSON diagnostic: {render_error}; {error}");
                    process::exit(3);
                }
            }
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
            process::exit(error.exit_code());
        }
    }
}
