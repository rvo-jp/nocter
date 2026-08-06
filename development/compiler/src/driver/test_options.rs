use super::compile_options::{is_arg, required_value};
use crate::target::{DEFAULT_TARGET, validate_requested_target};
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TestOutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TestCommand {
    pub(super) root: PathBuf,
    pub(super) selected: Option<String>,
    pub(super) target: String,
    pub(super) locked: bool,
    pub(super) offline: bool,
    pub(super) format: TestOutputFormat,
}

pub(super) fn parse_test_command(args: &[OsString]) -> Result<TestCommand, String> {
    let mut command = TestCommand {
        root: PathBuf::from("."),
        selected: None,
        target: DEFAULT_TARGET.to_string(),
        locked: false,
        offline: false,
        format: TestOutputFormat::Human,
    };
    let mut root_was_set = false;
    let mut index = 1;
    while index < args.len() {
        let flag = args[index].to_string_lossy();
        match flag.as_ref() {
            "--root" => {
                let value = required_value(args, index, "expected package root after `--root`")?;
                if root_was_set {
                    return Err("package root specified more than once".to_string());
                }
                command.root = PathBuf::from(value);
                root_was_set = true;
                index += 2;
            }
            "--test" => {
                let value = required_value(args, index, "expected test name after `--test`")?;
                if command.selected.is_some() {
                    return Err("test specified more than once".to_string());
                }
                command.selected = Some(value.to_string_lossy().into_owned());
                index += 2;
            }
            "--target" => {
                let value = required_value(args, index, "expected target after `--target`")?;
                let target = value.to_string_lossy();
                validate_requested_target(&target)?;
                command.target = target.into_owned();
                index += 2;
            }
            "--locked" => {
                command.locked = true;
                index += 1;
            }
            "--offline" => {
                command.offline = true;
                index += 1;
            }
            "--format" => {
                let value = required_value(args, index, "expected `--format json`")?;
                if !is_arg(value, "json") {
                    return Err("expected `--format json`".to_string());
                }
                if command.format == TestOutputFormat::Json {
                    return Err("output format specified more than once".to_string());
                }
                command.format = TestOutputFormat::Json;
                index += 2;
            }
            _ => return Err(format!("unexpected argument `{flag}`")),
        }
    }
    Ok(command)
}
