use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};

use crate::{
    BuildCommandOptions, BuildCommandPlan, CommandPlanError, ProgramInputError,
    ProgramInputOptions, RunCommandOptions, RunCommandPlan, resolve_program_input,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResolutionOptions {
    locked: bool,
    offline: bool,
}

impl ResolutionOptions {
    #[must_use]
    pub const fn locked(self) -> bool {
        self.locked
    }

    #[must_use]
    pub const fn offline(self) -> bool {
        self.offline
    }
}

#[derive(Debug)]
pub enum ParsedCommand {
    Build(ParsedBuildCommand),
    Run(ParsedRunCommand),
}

#[derive(Debug)]
pub struct ParsedBuildCommand {
    input: ProgramInputOptions,
    command: BuildCommandOptions,
    resolution: ResolutionOptions,
}

impl ParsedBuildCommand {
    /// Resolves filesystem identity and closes build selection after pure argument parsing.
    ///
    /// # Errors
    ///
    /// Returns the exact input-resolution or build-planning failure.
    pub fn prepare(
        self,
        current_directory: impl AsRef<Path>,
    ) -> Result<PreparedBuildCommand, PreparedCommandError> {
        let input = resolve_program_input(current_directory, self.input)
            .map_err(PreparedCommandError::Input)?;
        let plan =
            BuildCommandPlan::new(input, self.command).map_err(PreparedCommandError::Plan)?;
        Ok(PreparedBuildCommand {
            plan,
            resolution: self.resolution,
        })
    }
}

#[derive(Debug)]
pub struct ParsedRunCommand {
    input: ProgramInputOptions,
    command: RunCommandOptions,
    resolution: ResolutionOptions,
}

impl ParsedRunCommand {
    /// Resolves filesystem identity and closes run selection after pure argument parsing.
    ///
    /// # Errors
    ///
    /// Returns the exact input-resolution or run-planning failure.
    pub fn prepare(
        self,
        current_directory: impl AsRef<Path>,
    ) -> Result<PreparedRunCommand, PreparedCommandError> {
        let input = resolve_program_input(current_directory, self.input)
            .map_err(PreparedCommandError::Input)?;
        let plan = RunCommandPlan::new(input, self.command).map_err(PreparedCommandError::Plan)?;
        Ok(PreparedRunCommand {
            plan,
            resolution: self.resolution,
        })
    }
}

#[derive(Debug)]
pub struct PreparedBuildCommand {
    plan: BuildCommandPlan,
    resolution: ResolutionOptions,
}

impl PreparedBuildCommand {
    #[must_use]
    pub const fn plan(&self) -> &BuildCommandPlan {
        &self.plan
    }

    #[must_use]
    pub const fn resolution(&self) -> ResolutionOptions {
        self.resolution
    }
}

#[derive(Debug)]
pub struct PreparedRunCommand {
    plan: RunCommandPlan,
    resolution: ResolutionOptions,
}

impl PreparedRunCommand {
    #[must_use]
    pub const fn plan(&self) -> &RunCommandPlan {
        &self.plan
    }

    #[must_use]
    pub const fn resolution(&self) -> ResolutionOptions {
        self.resolution
    }
}

/// Parses build/run arguments without reading process state or the filesystem.
///
/// The iterator starts with the command name and excludes the process executable name.
///
/// # Errors
///
/// Rejects missing/unknown commands, unknown or duplicate options, missing values, and multiple
/// positional source paths.
pub fn parse_command_arguments(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<ParsedCommand, CommandArgumentError> {
    let mut arguments = arguments.into_iter();
    let command = arguments
        .next()
        .ok_or(CommandArgumentError::MissingCommand)?;
    match command.to_str() {
        Some("build") => parse_build(arguments).map(ParsedCommand::Build),
        Some("run") => parse_run(arguments).map(ParsedCommand::Run),
        _ => Err(CommandArgumentError::UnknownCommand(command)),
    }
}

fn parse_build(
    arguments: impl Iterator<Item = OsString>,
) -> Result<ParsedBuildCommand, CommandArgumentError> {
    let ParsedOptions {
        root,
        file,
        positional,
        executable,
        output,
        resolution,
    } = parse_options(arguments, true)?;
    Ok(ParsedBuildCommand {
        input: ProgramInputOptions::new(root, positional, file),
        command: BuildCommandOptions::new(executable, output),
        resolution,
    })
}

fn parse_run(
    arguments: impl Iterator<Item = OsString>,
) -> Result<ParsedRunCommand, CommandArgumentError> {
    let ParsedOptions {
        root,
        file,
        positional,
        executable,
        output: _,
        resolution,
    } = parse_options(arguments, false)?;
    Ok(ParsedRunCommand {
        input: ProgramInputOptions::new(root, positional, file),
        command: RunCommandOptions::new(executable),
        resolution,
    })
}

#[derive(Default)]
struct ParsedOptions {
    root: Option<PathBuf>,
    file: Option<PathBuf>,
    positional: Option<PathBuf>,
    executable: Option<Box<str>>,
    output: Option<PathBuf>,
    resolution: ResolutionOptions,
}

fn parse_options(
    mut arguments: impl Iterator<Item = OsString>,
    accepts_output: bool,
) -> Result<ParsedOptions, CommandArgumentError> {
    let mut parsed = ParsedOptions::default();
    let mut positional_only = false;
    while let Some(argument) = arguments.next() {
        if !positional_only && argument == OsStr::new("--") {
            positional_only = true;
            continue;
        }
        if !positional_only {
            if let Some((name, value)) = split_long_option(&argument) {
                parse_valued_option(
                    &mut parsed,
                    name,
                    Some(value),
                    &mut arguments,
                    accepts_output,
                )?;
                continue;
            }
            if let Some(name) = argument.to_str().filter(|value| value.starts_with('-')) {
                match name {
                    "--root" | "--file" | "--executable" | "--output" | "-o" => {
                        parse_valued_option(
                            &mut parsed,
                            name,
                            None,
                            &mut arguments,
                            accepts_output,
                        )?;
                    }
                    "--locked" => set_flag(&mut parsed.resolution.locked, "--locked")?,
                    "--offline" => set_flag(&mut parsed.resolution.offline, "--offline")?,
                    _ => return Err(CommandArgumentError::UnknownOption(argument)),
                }
                continue;
            }
        }
        if parsed
            .positional
            .replace(PathBuf::from(&argument))
            .is_some()
        {
            return Err(CommandArgumentError::MultiplePositionalSources(argument));
        }
    }
    Ok(parsed)
}

fn split_long_option(argument: &OsStr) -> Option<(&str, &OsStr)> {
    let argument = argument.to_str()?;
    let (name, value) = argument.split_once('=')?;
    name.starts_with("--").then(|| (name, OsStr::new(value)))
}

fn parse_valued_option(
    parsed: &mut ParsedOptions,
    name: &str,
    inline_value: Option<&OsStr>,
    arguments: &mut impl Iterator<Item = OsString>,
    accepts_output: bool,
) -> Result<(), CommandArgumentError> {
    if matches!(name, "--output" | "-o") && !accepts_output {
        return Err(CommandArgumentError::OptionNotAccepted {
            option: "--output",
            command: "run",
        });
    }
    let value = match inline_value {
        Some(value) if value.is_empty() => {
            return Err(CommandArgumentError::MissingValue(name.into()));
        }
        Some(value) => value.to_owned(),
        None => arguments
            .next()
            .ok_or_else(|| CommandArgumentError::MissingValue(name.into()))?,
    };
    match name {
        "--root" => set_path(&mut parsed.root, value, "--root"),
        "--file" => set_path(&mut parsed.file, value, "--file"),
        "--executable" => {
            let value = value
                .into_string()
                .map_err(CommandArgumentError::NonUnicodeExecutable)?;
            if value.is_empty() {
                return Err(CommandArgumentError::EmptyExecutable);
            }
            set_value(&mut parsed.executable, value.into(), "--executable")
        }
        "--output" | "-o" => set_path(&mut parsed.output, value, "--output"),
        _ => Err(CommandArgumentError::UnknownOption(name.into())),
    }
}

fn set_path(
    slot: &mut Option<PathBuf>,
    value: OsString,
    name: &'static str,
) -> Result<(), CommandArgumentError> {
    set_value(slot, PathBuf::from(value), name)
}

fn set_value<T>(
    slot: &mut Option<T>,
    value: T,
    name: &'static str,
) -> Result<(), CommandArgumentError> {
    if slot.replace(value).is_some() {
        Err(CommandArgumentError::DuplicateOption(name))
    } else {
        Ok(())
    }
}

fn set_flag(flag: &mut bool, name: &'static str) -> Result<(), CommandArgumentError> {
    if std::mem::replace(flag, true) {
        Err(CommandArgumentError::DuplicateOption(name))
    } else {
        Ok(())
    }
}

#[derive(Debug)]
pub enum PreparedCommandError {
    Input(ProgramInputError),
    Plan(CommandPlanError),
}

impl fmt::Display for PreparedCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(error) => error.fmt(formatter),
            Self::Plan(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PreparedCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input(error) => Some(error),
            Self::Plan(error) => Some(error),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CommandArgumentError {
    MissingCommand,
    UnknownCommand(OsString),
    UnknownOption(OsString),
    OptionNotAccepted {
        option: &'static str,
        command: &'static str,
    },
    MissingValue(Box<str>),
    DuplicateOption(&'static str),
    MultiplePositionalSources(OsString),
    NonUnicodeExecutable(OsString),
    EmptyExecutable,
}

impl fmt::Display for CommandArgumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCommand => formatter.write_str("missing command"),
            Self::UnknownCommand(command) => {
                write!(formatter, "unknown command {}", command.to_string_lossy())
            }
            Self::UnknownOption(option) => {
                write!(formatter, "unknown option {}", option.to_string_lossy())
            }
            Self::OptionNotAccepted { option, command } => {
                write!(formatter, "{option} is not accepted by {command}")
            }
            Self::MissingValue(option) => write!(formatter, "{option} requires a value"),
            Self::DuplicateOption(option) => {
                write!(formatter, "{option} was provided more than once")
            }
            Self::MultiplePositionalSources(source) => write!(
                formatter,
                "more than one positional source was provided; unexpected {}",
                source.to_string_lossy()
            ),
            Self::NonUnicodeExecutable(name) => write!(
                formatter,
                "executable name is not Unicode: {}",
                name.to_string_lossy()
            ),
            Self::EmptyExecutable => formatter.write_str("executable name cannot be empty"),
        }
    }
}

impl std::error::Error for CommandArgumentError {}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use nocter_session::ExecutableSelector;

    use super::*;
    use crate::{BuildOperation, ResolvedProgramInput, SelectedBuildOutput};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    fn arguments(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    fn package_root() -> PathBuf {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "nocter-command-arguments-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("nocter.nct"), "#name: \"app\"\n").unwrap();
        root
    }

    #[test]
    fn build_arguments_prepare_the_existing_closed_plan() {
        let root = package_root();
        let parsed = parse_command_arguments(arguments(&[
            "build",
            "--root=.",
            "--executable",
            "tool",
            "--output",
            "bin/tool",
            "--locked",
            "--offline",
        ]))
        .unwrap();
        let ParsedCommand::Build(parsed) = parsed else {
            panic!("expected build command");
        };
        let prepared = parsed.prepare(&root).unwrap();

        assert!(prepared.resolution().locked());
        assert!(prepared.resolution().offline());
        assert!(matches!(
            prepared.plan().input(),
            ResolvedProgramInput::Package(_)
        ));
        assert!(matches!(
            prepared.plan().operation(),
            BuildOperation::Selected {
                selector: ExecutableSelector::Named(name),
                output: SelectedBuildOutput::Exact(path),
            } if name.as_ref() == "tool" && path == &fs::canonicalize(&root).unwrap().join("bin/tool")
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn run_arguments_reject_output_and_preserve_option_boundaries() {
        assert_eq!(
            parse_command_arguments(arguments(&["run", "-o", "app"])).unwrap_err(),
            CommandArgumentError::OptionNotAccepted {
                option: "--output",
                command: "run",
            }
        );
        assert_eq!(
            parse_command_arguments(arguments(&["build", "--root", ".", "--root", "."]))
                .unwrap_err(),
            CommandArgumentError::DuplicateOption("--root")
        );
        assert_eq!(
            parse_command_arguments(arguments(&["build", "a.nct", "b.nct"])).unwrap_err(),
            CommandArgumentError::MultiplePositionalSources("b.nct".into())
        );
    }

    #[test]
    fn end_of_options_keeps_a_dash_prefixed_source_positional() {
        let parsed = parse_command_arguments(arguments(&["run", "--", "--script.nct"])).unwrap();
        let ParsedCommand::Run(parsed) = parsed else {
            panic!("expected run command");
        };
        let root = package_root();
        fs::write(root.join("--script.nct"), "func main(): void { return }\n").unwrap();
        let prepared = parsed.prepare(&root).unwrap();
        assert!(matches!(
            prepared.plan().selector(),
            ExecutableSelector::Only
        ));
        assert!(matches!(
            prepared.plan().input(),
            ResolvedProgramInput::SingleFile(_)
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
