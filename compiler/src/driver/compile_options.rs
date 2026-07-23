use crate::entry::DEFAULT_ENTRY_FILE;
use crate::target::{DEFAULT_TARGET, validate_requested_target};
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceCommand {
    pub(super) file: PathBuf,
    pub(super) target: String,
}

impl SourceCommand {
    pub(super) fn new(file: PathBuf) -> Self {
        Self {
            file,
            target: DEFAULT_TARGET.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BuildCommand {
    pub(super) source: SourceCommand,
    pub(super) output: Option<PathBuf>,
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
    pub(super) output: Option<PathBuf>,
}

pub(super) fn parse_compile_command(
    args: &[OsString],
    kind: CompileCommandKind,
) -> Result<CompileCommandOptions, String> {
    let (mut source, option_start) = match args.get(1) {
        Some(value) if is_source_argument(value) => {
            (SourceCommand::new(PathBuf::from(value.clone())), 2)
        }
        _ => (SourceCommand::new(PathBuf::from(DEFAULT_ENTRY_FILE)), 1),
    };
    let mut json = false;
    let mut output = None;
    parse_compile_options(
        args,
        option_start,
        kind,
        &mut source,
        &mut json,
        &mut output,
    )?;

    Ok(CompileCommandOptions {
        source,
        json,
        output,
    })
}

pub(super) fn parse_bare_run_command(args: &[OsString]) -> Result<SourceCommand, String> {
    let mut source = SourceCommand::new(PathBuf::from(args[0].clone()));
    let mut json = false;
    let mut output = None;
    parse_compile_options(
        args,
        1,
        CompileCommandKind::Run,
        &mut source,
        &mut json,
        &mut output,
    )?;
    Ok(source)
}

fn parse_compile_options(
    args: &[OsString],
    mut index: usize,
    kind: CompileCommandKind,
    source: &mut SourceCommand,
    json: &mut bool,
    output: &mut Option<PathBuf>,
) -> Result<(), String> {
    while index < args.len() {
        let flag = args[index].to_string_lossy();
        match flag.as_ref() {
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
            "-o" if kind == CompileCommandKind::Build => {
                let Some(value) = args.get(index + 1) else {
                    return Err("expected output path after `-o`".to_string());
                };
                if output.is_some() {
                    return Err("output path specified more than once".to_string());
                }
                *output = Some(PathBuf::from(value.clone()));
                index += 2;
            }
            "-o" => return Err("unexpected argument `-o`".to_string()),
            "--target" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("expected target after `--target`".to_string());
                };
                let target = value.to_string_lossy();
                validate_requested_target(&target)?;
                source.target = target.into_owned();
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

fn is_source_argument(arg: &OsString) -> bool {
    !arg.to_string_lossy().starts_with('-')
}
