use crate::entry::{DEFAULT_ENTRY_NAME, validate_entry_name};
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceCommand {
    pub(super) file: PathBuf,
    pub(super) entry: String,
}

impl SourceCommand {
    pub(super) fn new(file: PathBuf) -> Self {
        Self {
            file,
            entry: DEFAULT_ENTRY_NAME.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CompileCommandKind {
    Build,
    Run,
    Check,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CompileCommandOptions {
    pub(super) source: SourceCommand,
    pub(super) json: bool,
}

pub(super) fn parse_compile_command(
    args: &[OsString],
    kind: CompileCommandKind,
) -> Result<CompileCommandOptions, String> {
    if args.len() == 1 {
        return Err("missing source file".to_string());
    }

    let mut source = SourceCommand::new(PathBuf::from(args[1].clone()));
    let mut json = false;
    parse_compile_options(args, 2, kind, &mut source, &mut json)?;

    Ok(CompileCommandOptions { source, json })
}

pub(super) fn parse_bare_run_command(args: &[OsString]) -> Result<SourceCommand, String> {
    let mut source = SourceCommand::new(PathBuf::from(args[0].clone()));
    let mut json = false;
    parse_compile_options(args, 1, CompileCommandKind::Run, &mut source, &mut json)?;
    Ok(source)
}

fn parse_compile_options(
    args: &[OsString],
    mut index: usize,
    kind: CompileCommandKind,
    source: &mut SourceCommand,
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
                source.entry = entry.into_owned();
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

fn is_arg(arg: &OsString, expected: &str) -> bool {
    arg.to_string_lossy() == expected
}
