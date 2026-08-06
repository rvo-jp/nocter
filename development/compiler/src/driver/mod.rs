mod buildability;
mod command;
mod compile_options;
mod doctor;
mod errors;
mod fmt;
mod fmt_options;
mod json;
mod json_tool_options;
mod lsp;
mod package_commands;
mod package_plan;
mod pipeline;
mod run;
mod temporary_executable;
mod test_command;
mod test_options;
mod test_report;

use crate::target::{DEFAULT_TARGET, HOST};
use command::{Command, CommandErrorKind, parse_command};
use doctor::run_doctor;
use errors::{command_line_diagnostic, target_selection_diagnostic, write_human_diagnostics};
use fmt::run_fmt;
use json::{run_ast_json, run_tokens_json, write_diagnostics_json};
use lsp::run_lsp;
use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run_from_env() -> ExitCode {
    run(env::args_os())
}

pub fn run<I, S>(args: I) -> ExitCode
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into);
    let _program_name = args.next();
    let rest: Vec<OsString> = args.collect();

    match parse_command(&rest) {
        Ok(Command::Help) => match write_usage(io::stdout().lock()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("internal compiler error: failed to write usage: {error}");
                ExitCode::from(3)
            }
        },
        Ok(Command::Version) => {
            println!("Nocter {VERSION}");
            println!("host: {HOST}");
            println!("default target: {DEFAULT_TARGET}");
            ExitCode::SUCCESS
        }
        Ok(Command::Doctor) => run_doctor(),
        Ok(Command::Fetch(command)) => package_commands::run_fetch_command(&command),
        Ok(Command::Build(command)) => package_commands::run_build_command(&command),
        Ok(Command::Run(command)) => package_commands::run_run_command(&command),
        Ok(Command::Check(command)) => package_commands::run_check_command(&command),
        Ok(Command::CheckJson(command)) => package_commands::run_check_json_command(&command),
        Ok(Command::Test(command)) => test_command::run_test_command(&command),
        Ok(Command::Fmt { check, file }) => run_fmt(&file, check),
        Ok(Command::Tokens(file)) => run_tokens_json(&file),
        Ok(Command::Ast(file)) => run_ast_json(&file),
        Ok(Command::Lsp) => run_lsp(),
        Err(error) => run_command_error(error),
    }
}

fn run_command_error(error: command::CommandError) -> ExitCode {
    let diagnostic = match error.kind() {
        CommandErrorKind::CommandLine => command_line_diagnostic(error.message()),
        CommandErrorKind::TargetSelection => target_selection_diagnostic(error.message()),
    };

    if error.wants_json() {
        return write_diagnostics_json(
            error.command().unwrap_or("check"),
            error.target(),
            error.root(),
            None,
            vec![diagnostic],
            ExitCode::from(2),
        );
    }

    write_human_diagnostics(&[diagnostic], None, ExitCode::from(2))
}

fn write_usage(mut writer: impl Write) -> io::Result<()> {
    writeln!(writer, "usage: nocter <command> [args]")?;
    writeln!(writer)?;
    writeln!(writer, "commands:")?;
    writeln!(writer, "  fetch [--root <dir>] [--locked] [--offline]")?;
    writeln!(
        writer,
        "  test [--root <dir>] [--test <name>] [--locked] [--offline] [--target <target>] [--format json]"
    )?;
    writeln!(
        writer,
        "  build [--root <dir>] [--executable <name>] [-o <path>] [--target <target>]"
    )?;
    writeln!(
        writer,
        "  run [--root <dir>] [--executable <name>] [--target <target>]"
    )?;
    writeln!(
        writer,
        "  check [--root <dir>] [--executable <name>] [--target <target>]"
    )?;
    writeln!(
        writer,
        "  check [--root <dir>] [--executable <name>] [--target <target>] --format json"
    )?;
    writeln!(
        writer,
        "  build|run|check <file.nct> [options]  (or --file <file.nct>)"
    )?;
    writeln!(writer, "  fmt [--check] <file.nct>")?;
    writeln!(writer, "  tokens <file.nct> --format json")?;
    writeln!(writer, "  ast <file.nct> --format json")?;
    writeln!(writer, "  doctor")?;
    writeln!(writer, "  --version")?;
    writeln!(writer, "  lsp")
}
