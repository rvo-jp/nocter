use super::{Command, parse_command};
use crate::driver::compile_options::{BuildCommand, SourceCommand};
use crate::entry::DEFAULT_ENTRY_FILE;
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
fn parses_build_output_path() {
    let command = parse_command(&[
        OsString::from("build"),
        OsString::from("app.nct"),
        OsString::from("-o"),
        OsString::from("bin/app"),
    ])
    .unwrap();

    assert_eq!(
        command,
        Command::Build(build_command(
            SourceCommand::new(PathBuf::from("app.nct")),
            Some(PathBuf::from("bin/app"))
        ))
    );
}

#[test]
fn parses_build_run_and_check_without_file_as_main_source() {
    assert_eq!(
        parse_command(&[OsString::from("build")]).unwrap(),
        Command::Build(build_command(
            SourceCommand::new(PathBuf::from(DEFAULT_ENTRY_FILE)),
            None,
        ))
    );
    assert_eq!(
        parse_command(&[OsString::from("run")]).unwrap(),
        Command::Run(SourceCommand::new(PathBuf::from(DEFAULT_ENTRY_FILE)))
    );
    assert_eq!(
        parse_command(&[OsString::from("check")]).unwrap(),
        Command::Check(SourceCommand::new(PathBuf::from(DEFAULT_ENTRY_FILE)))
    );
}

#[test]
fn parses_compile_target_option() {
    let command = parse_command(&[
        OsString::from("check"),
        OsString::from("app.nct"),
        OsString::from("--target"),
        OsString::from("arm64-darwin"),
    ])
    .unwrap();

    assert_eq!(
        command,
        Command::Check(source_command("app.nct", "arm64-darwin"))
    );
}

#[test]
fn parses_check_json_with_default_source() {
    let command = parse_command(&[
        OsString::from("check"),
        OsString::from("--format"),
        OsString::from("json"),
    ])
    .unwrap();

    assert_eq!(
        command,
        Command::CheckJson(SourceCommand::new(PathBuf::from(DEFAULT_ENTRY_FILE)))
    );
}

#[test]
fn rejects_unimplemented_reserved_target() {
    let error = parse_command(&[
        OsString::from("build"),
        OsString::from("app.nct"),
        OsString::from("--target"),
        OsString::from("x64-linux"),
    ])
    .unwrap_err();

    assert_eq!(
        error,
        "target `x64-linux` is recognized but not implemented"
    );
}

#[test]
fn rejects_output_path_for_non_build_commands() {
    let error = parse_command(&[
        OsString::from("run"),
        OsString::from("app.nct"),
        OsString::from("-o"),
        OsString::from("app"),
    ])
    .unwrap_err();

    assert_eq!(error, "unexpected argument `-o`");
}

#[test]
fn rejects_entry_option() {
    let error = parse_command(&[
        OsString::from("run"),
        OsString::from("app.nct"),
        OsString::from("--entry"),
        OsString::from("start"),
    ])
    .unwrap_err();

    assert_eq!(error, "unexpected argument `--entry`");
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

fn source_command(file: &str, target: &str) -> SourceCommand {
    SourceCommand {
        file: PathBuf::from(file),
        target: target.to_string(),
    }
}

fn build_command(source: SourceCommand, output: Option<PathBuf>) -> BuildCommand {
    BuildCommand { source, output }
}
