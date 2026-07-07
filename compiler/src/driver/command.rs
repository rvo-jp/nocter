use super::compile_options::{
    CompileCommandKind, CompileCommandOptions, SourceCommand, parse_bare_run_command,
    parse_compile_command,
};
use super::fmt_options::{FmtCommandOptions, parse_fmt_command};
use super::json_tool_options::parse_json_tool_command;
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Command {
    Help,
    Version,
    Doctor,
    Build(SourceCommand),
    Run(SourceCommand),
    Check(SourceCommand),
    CheckJson(SourceCommand),
    Fmt { check: bool, file: PathBuf },
    Tokens(PathBuf),
    Ast(PathBuf),
    Lsp,
}

pub(super) fn parse_command(args: &[OsString]) -> Result<Command, String> {
    if args.is_empty() {
        return Ok(Command::Help);
    }

    let command = args[0].to_string_lossy();
    match command.as_ref() {
        "-h" | "--help" | "help" => Ok(Command::Help),
        "--version" | "version" => expect_no_extra(args, Command::Version),
        "doctor" => expect_no_extra(args, Command::Doctor),
        "build" => parse_compile_command(args, CompileCommandKind::Build)
            .map(|options| Command::Build(options.source)),
        "run" => parse_compile_command(args, CompileCommandKind::Run)
            .map(|options| Command::Run(options.source)),
        "check" => {
            parse_compile_command(args, CompileCommandKind::Check).map(check_command_from_options)
        }
        "fmt" => parse_fmt_command(args).map(fmt_command_from_options),
        "tokens" => parse_json_tool_command(args).map(Command::Tokens),
        "ast" => parse_json_tool_command(args).map(Command::Ast),
        "lsp" => expect_no_extra(args, Command::Lsp),
        value if value.ends_with(".nct") => parse_bare_run_command(args).map(Command::Run),
        _ => Err(format!("unknown command `{command}`")),
    }
}

fn check_command_from_options(options: CompileCommandOptions) -> Command {
    if options.json {
        Command::CheckJson(options.source)
    } else {
        Command::Check(options.source)
    }
}

fn fmt_command_from_options(options: FmtCommandOptions) -> Command {
    Command::Fmt {
        check: options.check,
        file: options.file,
    }
}

fn expect_no_extra(args: &[OsString], command: Command) -> Result<Command, String> {
    if args.len() == 1 {
        Ok(command)
    } else {
        Err(format!(
            "unexpected argument `{}`",
            args[1].to_string_lossy()
        ))
    }
}

#[cfg(test)]
mod tests;
