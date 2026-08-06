use super::compile_options::{
    BuildCommand, CompileCommandKind, CompileCommandOptions, SourceCommand, parse_compile_command,
    parse_fetch_command,
};
use super::fmt_options::{FmtCommandOptions, parse_fmt_command};
use super::json_tool_options::parse_json_tool_command;
use super::test_options::{TestCommand, parse_test_command};
use crate::target::DEFAULT_TARGET;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Command {
    Help,
    Version,
    Doctor,
    Fetch(SourceCommand),
    Build(BuildCommand),
    Run(SourceCommand),
    Check(SourceCommand),
    CheckJson(SourceCommand),
    Test(TestCommand),
    Fmt { check: bool, file: PathBuf },
    Tokens(PathBuf),
    Ast(PathBuf),
    Lsp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CommandError {
    message: String,
    command: Option<String>,
    root: Option<String>,
    target: Option<String>,
    json: bool,
    kind: CommandErrorKind,
}

impl CommandError {
    pub(super) fn message(&self) -> &str {
        &self.message
    }

    pub(super) fn command(&self) -> Option<&str> {
        self.command.as_deref()
    }

    pub(super) fn root(&self) -> Option<String> {
        self.root.clone()
    }

    pub(super) fn target(&self) -> Option<String> {
        self.target.clone()
    }

    pub(super) fn wants_json(&self) -> bool {
        self.json
    }

    pub(super) fn kind(&self) -> CommandErrorKind {
        self.kind
    }

    fn new(args: &[OsString], message: String) -> Self {
        Self {
            kind: command_error_kind(&message),
            message,
            command: command_name(args),
            root: root_argument(args),
            target: target_argument(args),
            json: wants_json_output(args),
        }
    }
}

impl PartialEq<&str> for CommandError {
    fn eq(&self, other: &&str) -> bool {
        self.message == *other
    }
}

impl PartialEq<CommandError> for &str {
    fn eq(&self, other: &CommandError) -> bool {
        *self == other.message
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CommandErrorKind {
    CommandLine,
    TargetSelection,
}

pub(super) fn parse_command(args: &[OsString]) -> Result<Command, CommandError> {
    if args.is_empty() {
        return Ok(Command::Help);
    }

    let command = args[0].to_string_lossy();
    let parsed = match command.as_ref() {
        "-h" | "--help" | "help" => Ok(Command::Help),
        "--version" | "version" => expect_no_extra(args, Command::Version),
        "doctor" => expect_no_extra(args, Command::Doctor),
        "fetch" => parse_fetch_command(args).map(Command::Fetch),
        "build" => parse_compile_command(args, CompileCommandKind::Build).map(|options| {
            Command::Build(BuildCommand {
                source: options.source,
                output: options.output,
            })
        }),
        "run" => parse_compile_command(args, CompileCommandKind::Run)
            .map(|options| Command::Run(options.source)),
        "check" => {
            parse_compile_command(args, CompileCommandKind::Check).map(check_command_from_options)
        }
        "test" => parse_test_command(args).map(Command::Test),
        "fmt" => parse_fmt_command(args).map(fmt_command_from_options),
        "tokens" => parse_json_tool_command(args).map(Command::Tokens),
        "ast" => parse_json_tool_command(args).map(Command::Ast),
        "lsp" => expect_no_extra(args, Command::Lsp),
        _ => Err(format!("unknown command `{command}`")),
    };

    parsed.map_err(|message| CommandError::new(args, message))
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

fn command_error_kind(message: &str) -> CommandErrorKind {
    if message.starts_with("target `") {
        CommandErrorKind::TargetSelection
    } else {
        CommandErrorKind::CommandLine
    }
}

fn command_name(args: &[OsString]) -> Option<String> {
    let first = args.first()?.to_string_lossy();
    let command = match first.as_ref() {
        "-h" | "--help" | "help" => "help",
        "--version" | "version" => "version",
        "doctor" => "doctor",
        "fetch" => "fetch",
        "build" => "build",
        "run" => "run",
        "check" => "check",
        "test" => "test",
        "fmt" => "fmt",
        "tokens" => "tokens",
        "ast" => "ast",
        "lsp" => "lsp",
        _ => return None,
    };

    Some(command.to_string())
}

fn root_argument(args: &[OsString]) -> Option<String> {
    let first = args.first()?.to_string_lossy();
    match first.as_ref() {
        "build" | "run" | "check" | "fetch" | "test" => root_option(args)
            .map(|root| format!("{root}/nocter.nct"))
            .or_else(|| file_option(args))
            .or_else(|| root_after_command(args, 1))
            .or_else(|| Some("./nocter.nct".to_string())),
        "tokens" | "ast" => root_after_command(args, 1),
        "fmt" => {
            if args
                .get(1)
                .is_some_and(|arg| arg.to_string_lossy() == "--check")
            {
                root_after_command(args, 2)
            } else {
                root_after_command(args, 1)
            }
        }
        _ => None,
    }
}

fn root_option(args: &[OsString]) -> Option<String> {
    option_value(args, "--root")
}

fn file_option(args: &[OsString]) -> Option<String> {
    option_value(args, "--file")
}

fn option_value(args: &[OsString], name: &str) -> Option<String> {
    args.windows(2).find_map(|window| {
        (window[0].to_string_lossy() == name).then(|| window[1].to_string_lossy().into_owned())
    })
}

fn root_after_command(args: &[OsString], index: usize) -> Option<String> {
    let value = args.get(index)?.to_string_lossy();
    if value.starts_with('-') {
        None
    } else {
        Some(value.into_owned())
    }
}

fn target_argument(args: &[OsString]) -> Option<String> {
    let target = args.windows(2).find_map(|window| {
        if window[0].to_string_lossy() == "--target" {
            Some(window[1].to_string_lossy().into_owned())
        } else {
            None
        }
    });

    target.or_else(|| {
        command_name(args).and_then(|command| match command.as_str() {
            "build" | "run" | "check" | "test" => Some(DEFAULT_TARGET.to_string()),
            _ => None,
        })
    })
}

fn wants_json_output(args: &[OsString]) -> bool {
    args.first()
        .is_some_and(|arg| matches!(arg.to_string_lossy().as_ref(), "check" | "test"))
        && args.windows(2).any(|window| {
            window[0].to_string_lossy() == "--format" && window[1].to_string_lossy() == "json"
        })
}

#[cfg(test)]
mod tests;
