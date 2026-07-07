use super::{Command, parse_command};
use crate::driver::compile_options::SourceCommand;
use std::ffi::OsString;
use std::path::PathBuf;

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
fn rejects_fmt_extra_argument_after_file() {
    let error = parse_command(&[
        OsString::from("fmt"),
        OsString::from("app.nct"),
        OsString::from("extra"),
    ])
    .unwrap_err();

    assert_eq!(error, "unexpected argument `extra`");
}

#[test]
fn rejects_fmt_check_extra_argument_after_file() {
    let error = parse_command(&[
        OsString::from("fmt"),
        OsString::from("--check"),
        OsString::from("app.nct"),
        OsString::from("extra"),
    ])
    .unwrap_err();

    assert_eq!(error, "unexpected argument `extra`");
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
    let error = parse_command(&[OsString::from("tokens"), OsString::from("app.nct")]).unwrap_err();
    assert_eq!(error, "missing `--format json`");
}
