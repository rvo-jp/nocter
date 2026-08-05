use crate::target::{DEFAULT_TARGET, validate_requested_target};
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CompileInput {
    Package { root: PathBuf },
    File { file: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceCommand {
    pub(super) input: CompileInput,
    pub(super) executable: Option<String>,
    pub(super) target: String,
}

impl SourceCommand {
    pub(super) fn package(root: PathBuf) -> Self {
        Self {
            input: CompileInput::Package { root },
            executable: None,
            target: DEFAULT_TARGET.to_string(),
        }
    }

    #[cfg(test)]
    pub(super) fn file(file: PathBuf) -> Self {
        Self {
            input: CompileInput::File { file },
            executable: None,
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
    let mut source = SourceCommand::package(PathBuf::from("."));
    let mut json = false;
    let mut output = None;
    let mut root_was_set = false;
    let mut file_was_set = false;
    let mut index = 1;

    while index < args.len() {
        let flag = args[index].to_string_lossy();
        match flag.as_ref() {
            "--root" => {
                let value = required_value(args, index, "expected package root after `--root`")?;
                if root_was_set {
                    return Err("package root specified more than once".to_string());
                }
                if file_was_set {
                    return Err("`--root` cannot be combined with `--file`".to_string());
                }
                source.input = CompileInput::Package {
                    root: PathBuf::from(value),
                };
                root_was_set = true;
                index += 2;
            }
            "--file" => {
                let value = required_value(args, index, "expected source file after `--file`")?;
                if file_was_set {
                    return Err("source file specified more than once".to_string());
                }
                if root_was_set {
                    return Err("`--file` cannot be combined with `--root`".to_string());
                }
                source.input = CompileInput::File {
                    file: PathBuf::from(value),
                };
                file_was_set = true;
                index += 2;
            }
            "--executable" => {
                let value =
                    required_value(args, index, "expected executable name after `--executable`")?;
                if source.executable.is_some() {
                    return Err("executable specified more than once".to_string());
                }
                source.executable = Some(value.to_string_lossy().into_owned());
                index += 2;
            }
            "--format" if kind == CompileCommandKind::Check => {
                let value = required_value(args, index, "expected `--format json`")?;
                if !is_arg(value, "json") {
                    return Err("expected `--format json`".to_string());
                }
                json = true;
                index += 2;
            }
            "-o" if kind == CompileCommandKind::Build => {
                let value = required_value(args, index, "expected output path after `-o`")?;
                if output.is_some() {
                    return Err("output path specified more than once".to_string());
                }
                output = Some(PathBuf::from(value));
                index += 2;
            }
            "-o" => return Err("unexpected argument `-o`".to_string()),
            "--target" => {
                let value = required_value(args, index, "expected target after `--target`")?;
                let target = value.to_string_lossy();
                validate_requested_target(&target)?;
                source.target = target.into_owned();
                index += 2;
            }
            "--format" => return Err("unexpected argument `--format`".to_string()),
            _ if !flag.starts_with('-') => {
                if file_was_set {
                    return Err(format!("unexpected additional source `{flag}`"));
                }
                if root_was_set {
                    return Err("a positional source cannot be combined with `--root`".to_string());
                }
                source.input = CompileInput::File {
                    file: PathBuf::from(&args[index]),
                };
                file_was_set = true;
                index += 1;
            }
            _ => return Err(format!("unexpected argument `{flag}`")),
        }
    }

    if file_was_set && source.executable.is_some() {
        return Err("`--executable` cannot be combined with `--file`".to_string());
    }

    Ok(CompileCommandOptions {
        source,
        json,
        output,
    })
}

fn required_value<'a>(
    args: &'a [OsString],
    index: usize,
    message: &str,
) -> Result<&'a OsString, String> {
    args.get(index + 1).ok_or_else(|| message.to_string())
}

fn is_arg(arg: &OsString, expected: &str) -> bool {
    arg.to_string_lossy() == expected
}
