use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Command {
    Help,
    Version,
    Doctor,
    Build(PathBuf),
    Run(PathBuf),
    Check(PathBuf),
    CheckJson(PathBuf),
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
        "build" => parse_one_path(args, Command::Build),
        "run" => parse_one_path(args, Command::Run),
        "check" => parse_check(args),
        "fmt" => parse_fmt(args),
        "tokens" => parse_json_tool_command(args, Command::Tokens),
        "ast" => parse_json_tool_command(args, Command::Ast),
        "lsp" => expect_no_extra(args, Command::Lsp),
        value if value.ends_with(".nct") => {
            expect_no_extra(args, Command::Run(PathBuf::from(args[0].clone())))
        }
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

fn parse_one_path(
    args: &[OsString],
    make_command: fn(PathBuf) -> Command,
) -> Result<Command, String> {
    match args {
        [_, file] => Ok(make_command(PathBuf::from(file.clone()))),
        [_] => Err("missing source file".to_string()),
        [_, _, extra, ..] => Err(format!("unexpected argument `{}`", extra.to_string_lossy())),
        [] => unreachable!("parse_one_path requires a command"),
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

fn parse_check(args: &[OsString]) -> Result<Command, String> {
    match args {
        [_, file] => Ok(Command::Check(PathBuf::from(file.clone()))),
        [_] => Err("missing source file".to_string()),
        [_, file, flag, format] if is_arg(flag, "--format") && is_arg(format, "json") => {
            Ok(Command::CheckJson(PathBuf::from(file.clone())))
        }
        [_, _, flag] if is_arg(flag, "--format") => Err("expected `--format json`".to_string()),
        [_, _, flag, ..] if is_arg(flag, "--format") => Err("expected `--format json`".to_string()),
        [_, _, extra, ..] => Err(format!("unexpected argument `{}`", extra.to_string_lossy())),
        [] => unreachable!("parse_check requires a command"),
    }
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
        assert_eq!(command, Command::Run(PathBuf::from("app.nct")));
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

        assert_eq!(command, Command::CheckJson(PathBuf::from("app.nct")));
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
