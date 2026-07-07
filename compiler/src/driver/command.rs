use crate::entry::{DEFAULT_ENTRY_NAME, validate_entry_name};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceCommand {
    pub(super) file: PathBuf,
    pub(super) entry: String,
}

impl SourceCommand {
    fn new(file: PathBuf) -> Self {
        Self {
            file,
            entry: DEFAULT_ENTRY_NAME.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompileCommandKind {
    Build,
    Run,
    Check,
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
        "build" => parse_compile_command(args, CompileCommandKind::Build),
        "run" => parse_compile_command(args, CompileCommandKind::Run),
        "check" => parse_compile_command(args, CompileCommandKind::Check),
        "fmt" => parse_fmt(args),
        "tokens" => parse_json_tool_command(args, Command::Tokens),
        "ast" => parse_json_tool_command(args, Command::Ast),
        "lsp" => expect_no_extra(args, Command::Lsp),
        value if value.ends_with(".nct") => parse_bare_run_command(args),
        _ => Err(format!("unknown command `{command}`")),
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

fn parse_fmt(args: &[OsString]) -> Result<Command, String> {
    match args {
        [_, flag] if flag.to_string_lossy() == "--check" => Err("missing source file".to_string()),
        [_, flag, file] if flag.to_string_lossy() == "--check" => Ok(Command::Fmt {
            check: true,
            file: PathBuf::from(file.clone()),
        }),
        [_, file] => Ok(Command::Fmt {
            check: false,
            file: PathBuf::from(file.clone()),
        }),
        [_] => Err("missing source file".to_string()),
        [_, extra, ..] => Err(format!("unexpected argument `{}`", extra.to_string_lossy())),
        [] => unreachable!("parse_fmt requires a command"),
    }
}

fn parse_compile_command(args: &[OsString], kind: CompileCommandKind) -> Result<Command, String> {
    if args.len() == 1 {
        return Err("missing source file".to_string());
    }

    let mut command = SourceCommand::new(PathBuf::from(args[1].clone()));
    let mut json = false;
    parse_compile_options(args, 2, kind, &mut command, &mut json)?;

    match (kind, json) {
        (CompileCommandKind::Build, false) => Ok(Command::Build(command)),
        (CompileCommandKind::Run, false) => Ok(Command::Run(command)),
        (CompileCommandKind::Check, false) => Ok(Command::Check(command)),
        (CompileCommandKind::Check, true) => Ok(Command::CheckJson(command)),
        (CompileCommandKind::Build | CompileCommandKind::Run, true) => {
            unreachable!("parse_compile_options accepts --format only for check commands")
        }
    }
}

fn parse_bare_run_command(args: &[OsString]) -> Result<Command, String> {
    let mut command = SourceCommand::new(PathBuf::from(args[0].clone()));
    let mut json = false;
    parse_compile_options(args, 1, CompileCommandKind::Run, &mut command, &mut json)?;
    Ok(Command::Run(command))
}

fn parse_compile_options(
    args: &[OsString],
    mut index: usize,
    kind: CompileCommandKind,
    command: &mut SourceCommand,
    json: &mut bool,
) -> Result<(), String> {
    while index < args.len() {
        let flag = args[index].to_string_lossy();
        match flag.as_ref() {
            "--entry" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("expected entry name after `--entry`".to_string());
                };
                let entry = value.to_string_lossy();
                validate_entry_name(&entry)?;
                command.entry = entry.into_owned();
                index += 2;
            }
            "--format" if kind == CompileCommandKind::Check => {
                let Some(value) = args.get(index + 1) else {
                    return Err("expected `--format json`".to_string());
                };
                if !is_arg(value, "json") {
                    return Err("expected `--format json`".to_string());
                }
                *json = true;
                index += 2;
            }
            "--format" => return Err("unexpected argument `--format`".to_string()),
            _ => {
                return Err(format!(
                    "unexpected argument `{}`",
                    args[index].to_string_lossy()
                ));
            }
        }
    }

    Ok(())
}

fn parse_json_tool_command(
    args: &[OsString],
    make_command: fn(PathBuf) -> Command,
) -> Result<Command, String> {
    if args.is_empty() {
        unreachable!("parse_json_tool_command requires a command");
    }

    if args.len() == 1 {
        return Err("missing source file".to_string());
    }

    if args.len() == 2 {
        return Err("missing `--format json`".to_string());
    }

    if !is_arg(&args[2], "--format") {
        return Err(format!(
            "unexpected argument `{}`",
            args[2].to_string_lossy()
        ));
    }

    if args.len() == 3 {
        return Err("expected `--format json`".to_string());
    }

    if !is_arg(&args[3], "json") {
        return Err("expected `--format json`".to_string());
    }

    if args.len() > 4 {
        return Err(format!(
            "unexpected argument `{}`",
            args[4].to_string_lossy()
        ));
    }

    Ok(make_command(PathBuf::from(args[1].clone())))
}

fn is_arg(arg: &OsString, expected: &str) -> bool {
    arg.to_string_lossy() == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_source_as_run() {
        let command = parse_command(&[OsString::from("app.nct")]).unwrap();
        assert_eq!(
            command,
            Command::Run(SourceCommand::new(PathBuf::from("app.nct")))
        );
    }

    #[test]
    fn parses_bare_source_entry_option_as_run() {
        let command = parse_command(&[
            OsString::from("app.nct"),
            OsString::from("--entry"),
            OsString::from("start"),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::Run(SourceCommand {
                file: PathBuf::from("app.nct"),
                entry: "start".to_string(),
            })
        );
    }

    #[test]
    fn parses_build_entry_option() {
        let command = parse_command(&[
            OsString::from("build"),
            OsString::from("app.nct"),
            OsString::from("--entry"),
            OsString::from("start"),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::Build(SourceCommand {
                file: PathBuf::from("app.nct"),
                entry: "start".to_string(),
            })
        );
    }

    #[test]
    fn parses_check_json_entry_option_in_either_order() {
        let command = parse_command(&[
            OsString::from("check"),
            OsString::from("app.nct"),
            OsString::from("--entry"),
            OsString::from("start"),
            OsString::from("--format"),
            OsString::from("json"),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::CheckJson(SourceCommand {
                file: PathBuf::from("app.nct"),
                entry: "start".to_string(),
            })
        );

        let command = parse_command(&[
            OsString::from("check"),
            OsString::from("app.nct"),
            OsString::from("--format"),
            OsString::from("json"),
            OsString::from("--entry"),
            OsString::from("start"),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::CheckJson(SourceCommand {
                file: PathBuf::from("app.nct"),
                entry: "start".to_string(),
            })
        );
    }

    #[test]
    fn rejects_invalid_entry_name() {
        let error = parse_command(&[
            OsString::from("run"),
            OsString::from("app.nct"),
            OsString::from("--entry"),
            OsString::from("not-valid"),
        ])
        .unwrap_err();

        assert_eq!(
            error,
            "entry name `not-valid` is not a valid Nocter identifier"
        );
    }

    #[test]
    fn parses_fmt_check() {
        let command = parse_command(&[
            OsString::from("fmt"),
            OsString::from("--check"),
            OsString::from("app.nct"),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::Fmt {
                check: true,
                file: PathBuf::from("app.nct")
            }
        );
    }

    #[test]
    fn rejects_fmt_check_without_file() {
        let error = parse_command(&[OsString::from("fmt"), OsString::from("--check")]).unwrap_err();
        assert_eq!(error, "missing source file");
    }

    #[test]
    fn parses_tokens_json() {
        let command = parse_command(&[
            OsString::from("tokens"),
            OsString::from("app.nct"),
            OsString::from("--format"),
            OsString::from("json"),
        ])
        .unwrap();

        assert_eq!(command, Command::Tokens(PathBuf::from("app.nct")));
    }

    #[test]
    fn parses_check_json() {
        let command = parse_command(&[
            OsString::from("check"),
            OsString::from("app.nct"),
            OsString::from("--format"),
            OsString::from("json"),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::CheckJson(SourceCommand::new(PathBuf::from("app.nct")))
        );
    }

    #[test]
    fn parses_ast_json() {
        let command = parse_command(&[
            OsString::from("ast"),
            OsString::from("app.nct"),
            OsString::from("--format"),
            OsString::from("json"),
        ])
        .unwrap();

        assert_eq!(command, Command::Ast(PathBuf::from("app.nct")));
    }

    #[test]
    fn rejects_tokens_without_json_format() {
        let error =
            parse_command(&[OsString::from("tokens"), OsString::from("app.nct")]).unwrap_err();
        assert_eq!(error, "missing `--format json`");
    }
}
