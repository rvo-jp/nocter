use super::{Command, parse_command};
use crate::driver::compile_options::{BuildCommand, CompileInput, SourceCommand};
use std::ffi::OsString;
use std::path::PathBuf;

#[test]
fn parses_package_commands_from_current_directory() {
    assert_eq!(
        parse_command(&[OsString::from("build")]).unwrap(),
        Command::Build(BuildCommand {
            source: package_command("."),
            output: None,
        })
    );
    assert_eq!(
        parse_command(&[OsString::from("run")]).unwrap(),
        Command::Run(package_command("."))
    );
    assert_eq!(
        parse_command(&[OsString::from("check")]).unwrap(),
        Command::Check(package_command("."))
    );
}

#[test]
fn parses_explicit_package_root_and_executable() {
    let command = parse_command(&[
        OsString::from("run"),
        OsString::from("--root"),
        OsString::from("packages/tool"),
        OsString::from("--executable"),
        OsString::from("inspect"),
    ])
    .unwrap();
    let mut expected = package_command("packages/tool");
    expected.executable = Some("inspect".to_string());
    assert_eq!(command, Command::Run(expected));
}

#[test]
fn parses_fetch_and_reproducibility_flags() {
    let mut expected = package_command("packages/tool");
    expected.locked = true;
    expected.offline = true;
    assert_eq!(
        parse_command(&[
            OsString::from("fetch"),
            OsString::from("--root"),
            OsString::from("packages/tool"),
            OsString::from("--locked"),
            OsString::from("--offline"),
        ])
        .unwrap(),
        Command::Fetch(expected)
    );
}

#[test]
fn parses_explicit_single_file_mode() {
    assert_eq!(
        parse_command(&[
            OsString::from("check"),
            OsString::from("--file"),
            OsString::from("app.nct"),
        ])
        .unwrap(),
        Command::Check(SourceCommand::file(PathBuf::from("app.nct")))
    );
}

#[test]
fn parses_positional_source_as_explicit_single_file_mode_and_rejects_bare_run() {
    assert_eq!(
        parse_command(&[OsString::from("build"), OsString::from("app.nct")]).unwrap(),
        Command::Build(BuildCommand {
            source: SourceCommand::file(PathBuf::from("app.nct")),
            output: None,
        })
    );
    assert_eq!(
        parse_command(&[OsString::from("app.nct")]).unwrap_err(),
        "unknown command `app.nct`"
    );
}

#[test]
fn rejects_conflicting_package_and_file_selection() {
    assert_eq!(
        parse_command(&[
            OsString::from("check"),
            OsString::from("--root"),
            OsString::from("."),
            OsString::from("--file"),
            OsString::from("app.nct"),
        ])
        .unwrap_err(),
        "`--file` cannot be combined with `--root`"
    );
}

#[test]
fn rejects_executable_selection_in_file_mode() {
    assert_eq!(
        parse_command(&[
            OsString::from("run"),
            OsString::from("--file"),
            OsString::from("app.nct"),
            OsString::from("--executable"),
            OsString::from("app"),
        ])
        .unwrap_err(),
        "`--executable` cannot be combined with `--file`"
    );
}

#[test]
fn parses_build_output_path_and_target() {
    let command = parse_command(&[
        OsString::from("build"),
        OsString::from("--executable"),
        OsString::from("app"),
        OsString::from("-o"),
        OsString::from("bin/app"),
        OsString::from("--target"),
        OsString::from("arm64-darwin"),
    ])
    .unwrap();
    let mut source = package_command(".");
    source.executable = Some("app".to_string());
    assert_eq!(
        command,
        Command::Build(BuildCommand {
            source,
            output: Some(PathBuf::from("bin/app")),
        })
    );
}

#[test]
fn parses_package_check_json() {
    assert_eq!(
        parse_command(&[
            OsString::from("check"),
            OsString::from("--format"),
            OsString::from("json"),
        ])
        .unwrap(),
        Command::CheckJson(package_command("."))
    );
}

#[test]
fn rejects_unimplemented_reserved_target() {
    let error = parse_command(&[
        OsString::from("build"),
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
fn parses_fmt_and_json_tools() {
    assert_eq!(
        parse_command(&[
            OsString::from("fmt"),
            OsString::from("--check"),
            OsString::from("app.nct"),
        ])
        .unwrap(),
        Command::Fmt {
            check: true,
            file: PathBuf::from("app.nct"),
        }
    );
    assert_eq!(
        parse_command(&[
            OsString::from("tokens"),
            OsString::from("app.nct"),
            OsString::from("--format"),
            OsString::from("json"),
        ])
        .unwrap(),
        Command::Tokens(PathBuf::from("app.nct"))
    );
    assert_eq!(
        parse_command(&[
            OsString::from("ast"),
            OsString::from("app.nct"),
            OsString::from("--format"),
            OsString::from("json"),
        ])
        .unwrap(),
        Command::Ast(PathBuf::from("app.nct"))
    );
}

fn package_command(root: &str) -> SourceCommand {
    SourceCommand::package(PathBuf::from(root))
}

#[test]
fn file_constructor_uses_explicit_file_input() {
    assert_eq!(
        SourceCommand::file(PathBuf::from("app.nct")).input,
        CompileInput::File {
            file: PathBuf::from("app.nct")
        }
    );
}
