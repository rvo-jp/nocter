mod build;
mod check;
mod command;
mod doctor;
mod json;
mod pipeline;
mod run;

use crate::target::{DEFAULT_TARGET, HOST};
use build::run_build;
use check::run_check;
use command::{Command, parse_command};
use doctor::run_doctor;
use json::{run_ast_json, run_check_json, run_tokens_json};
use run::run_file;
use std::env;
use std::ffi::OsString;
use std::path::Path;
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
        Ok(Command::Help) => {
            print_usage();
            ExitCode::SUCCESS
        }
        Ok(Command::Version) => {
            println!("nocter {VERSION}");
            println!("host: {HOST}");
            println!("default target: {DEFAULT_TARGET}");
            ExitCode::SUCCESS
        }
        Ok(Command::Doctor) => run_doctor(),
        Ok(Command::Build(command)) => run_build(&command.file, &command.entry),
        Ok(Command::Run(command)) => run_file(&command.file, &command.entry),
        Ok(Command::Check(command)) => run_check(&command.file, &command.entry),
        Ok(Command::CheckJson(command)) => run_check_json(&command.file, &command.entry),
        Ok(Command::Fmt { check, file }) => {
            let mode = if check { "fmt --check" } else { "fmt" };
            not_implemented(mode, &file)
        }
        Ok(Command::Tokens(file)) => run_tokens_json(&file),
        Ok(Command::Ast(file)) => run_ast_json(&file),
        Ok(Command::Lsp) => {
            eprintln!("error: nocter lsp is not implemented yet");
            ExitCode::FAILURE
        }
        Err(message) => {
            eprintln!("error: {message}");
            eprintln!();
            print_usage();
            ExitCode::FAILURE
        }
    }
}

fn not_implemented(command: &str, file: &Path) -> ExitCode {
    eprintln!(
        "error: nocter {command} is not implemented yet for `{}`",
        file.display()
    );
    ExitCode::FAILURE
}

fn print_usage() {
    println!("usage: nocter <command> [args]");
    println!();
    println!("commands:");
    println!("  build <file.nct> [--entry <name>]");
    println!("  run <file.nct> [--entry <name>]");
    println!("  <file.nct> [--entry <name>]");
    println!("  check <file.nct> [--entry <name>]");
    println!("  check <file.nct> [--entry <name>] --format json");
    println!("  fmt [--check] <file.nct>");
    println!("  tokens <file.nct> --format json");
    println!("  ast <file.nct> --format json");
    println!("  doctor");
    println!("  --version");
    println!("  lsp");
}
