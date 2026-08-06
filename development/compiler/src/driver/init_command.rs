use super::init_templates;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InitCommand {
    pub(super) directory: PathBuf,
    pub(super) name: Option<String>,
    pub(super) library: bool,
}

pub(super) fn parse_init_command(args: &[OsString]) -> Result<InitCommand, String> {
    let mut command = InitCommand {
        directory: PathBuf::from("."),
        name: None,
        library: false,
    };
    let mut directory_set = false;
    let mut index = 1;
    while index < args.len() {
        let argument = args[index].to_string_lossy();
        match argument.as_ref() {
            "--library" => {
                command.library = true;
                index += 1;
            }
            "--name" => {
                let value = args
                    .get(index + 1)
                    .ok_or("expected package name after `--name`")?;
                command.name = Some(value.to_string_lossy().into_owned());
                index += 2;
            }
            _ if !argument.starts_with('-') && !directory_set => {
                command.directory = PathBuf::from(argument.as_ref());
                directory_set = true;
                index += 1;
            }
            _ => return Err(format!("unexpected argument `{argument}`")),
        }
    }
    Ok(command)
}

pub(super) fn run_init_command(command: &InitCommand) -> ExitCode {
    let name = command
        .name
        .clone()
        .or_else(|| package_name(&command.directory));
    let Some(name) = name.filter(|name| valid_name(name)) else {
        eprintln!(
            "error[E0700]: package name must contain only ASCII letters, digits, `-`, or `_`"
        );
        return ExitCode::from(2);
    };
    let package_file = command.directory.join("nocter.nct");
    let test_file = command.directory.join("tests/unit.nct");
    if package_file.exists() || test_file.exists() {
        eprintln!(
            "error[E0700]: package initialization target already exists in `{}`",
            command.directory.display()
        );
        return ExitCode::from(2);
    }
    if let Err(error) = fs::create_dir_all(&command.directory) {
        eprintln!(
            "error[E0700]: failed to create `{}`: {error}",
            command.directory.display()
        );
        return ExitCode::from(2);
    }
    let source = if command.library {
        init_templates::library(&name)
    } else {
        init_templates::executable(&name)
    };
    if let Some(parent) = test_file.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!(
                "error[E0700]: failed to create `{}`: {error}",
                parent.display()
            );
            return ExitCode::from(2);
        }
    }
    if let Err(error) = fs::write(&test_file, init_templates::test()) {
        eprintln!(
            "error[E0700]: failed to write `{}`: {error}",
            test_file.display()
        );
        return ExitCode::from(2);
    }
    if let Err(error) = fs::write(&package_file, source) {
        eprintln!(
            "error[E0700]: failed to write `{}`: {error}",
            package_file.display()
        );
        return ExitCode::from(2);
    }
    println!("created {}", package_file.display());
    ExitCode::SUCCESS
}

fn package_name(directory: &Path) -> Option<String> {
    directory
        .canonicalize()
        .ok()
        .or_else(|| Some(directory.to_path_buf()))?
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}
