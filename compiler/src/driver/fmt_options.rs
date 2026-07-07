use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FmtCommandOptions {
    pub(super) check: bool,
    pub(super) file: PathBuf,
}

pub(super) fn parse_fmt_command(args: &[OsString]) -> Result<FmtCommandOptions, String> {
    match args {
        [] => unreachable!("parse_fmt_command requires a command"),
        [_] => Err("missing source file".to_string()),
        [_, flag] if is_arg(flag, "--check") => Err("missing source file".to_string()),
        [_, flag, file] if is_arg(flag, "--check") => Ok(FmtCommandOptions {
            check: true,
            file: PathBuf::from(file.clone()),
        }),
        [_, flag, _, extra, ..] if is_arg(flag, "--check") => {
            Err(format!("unexpected argument `{}`", extra.to_string_lossy()))
        }
        [_, file] => Ok(FmtCommandOptions {
            check: false,
            file: PathBuf::from(file.clone()),
        }),
        [_, _, extra, ..] => Err(format!("unexpected argument `{}`", extra.to_string_lossy())),
    }
}

fn is_arg(arg: &OsString, expected: &str) -> bool {
    arg.to_string_lossy() == expected
}
