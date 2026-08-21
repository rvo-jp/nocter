use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};

use nocter_model::CompilationTarget;

use crate::command_schema::{CommandKind, CommandOption, CommandSchema, option_schema};
use crate::{
    BuildCommandOptions, BuildCommandPlan, CheckCommandOptions, CheckCommandPlan, CommandPlanError,
    ProgramInputError, ProgramInputOptions, RunCommandOptions, RunCommandPlan, TestCommandOptions,
    TestCommandPlan, resolve_package_input, resolve_program_input,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DiagnosticFormat {
    #[default]
    Human,
    Json,
}

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
    Help(crate::HelpRequest),
    Version,
    Doctor,
    Fetch(ParsedFetchCommand),
    Check(ParsedCheckCommand),
    Build(ParsedBuildCommand),
    Run(ParsedRunCommand),
    Test(ParsedTestCommand),
}

impl ParsedCommand {
    #[must_use]
    pub const fn requested_target(&self) -> Option<CompilationTarget> {
        match self {
            Self::Check(command) => command.target,
            Self::Build(command) => command.target,
            Self::Run(command) => command.target,
            Self::Test(command) => command.target,
            Self::Help(_) | Self::Version | Self::Doctor | Self::Fetch(_) => None,
        }
    }
}

#[derive(Debug)]
pub struct ParsedTestCommand {
    root: Option<PathBuf>,
    command: TestCommandOptions,
    resolution: ResolutionOptions,
    format: DiagnosticFormat,
    target: Option<CompilationTarget>,
}

impl ParsedTestCommand {
    #[must_use]
    pub const fn format(&self) -> DiagnosticFormat {
        self.format
    }

    #[must_use]
    pub fn root_hint(&self) -> PathBuf {
        self.root.as_deref().map_or_else(
            || PathBuf::from("nocter.nct"),
            |root| root.join("nocter.nct"),
        )
    }

    /// Resolves one exact package and closes semantic test selection policy.
    ///
    /// # Errors
    ///
    /// Returns package input or test-plan validation failure.
    pub fn prepare(
        self,
        current_directory: impl AsRef<Path>,
    ) -> Result<PreparedTestCommand, PreparedCommandError> {
        let input = resolve_package_input(current_directory, self.root.as_deref())
            .map_err(PreparedCommandError::Input)?;
        let plan = TestCommandPlan::new(input, self.command).map_err(PreparedCommandError::Plan)?;
        Ok(PreparedTestCommand {
            plan,
            resolution: self.resolution,
            format: self.format,
            target: self.target,
        })
    }
}

#[derive(Debug)]
pub struct ParsedCheckCommand {
    input: ProgramInputOptions,
    command: CheckCommandOptions,
    resolution: ResolutionOptions,
    format: DiagnosticFormat,
    target: Option<CompilationTarget>,
}

impl ParsedCheckCommand {
    #[must_use]
    pub const fn format(&self) -> DiagnosticFormat {
        self.format
    }

    #[must_use]
    pub fn root_hint(&self) -> Option<PathBuf> {
        self.input.selected_root_hint()
    }

    /// Resolves filesystem identity and closes check selection after pure argument parsing.
    ///
    /// # Errors
    ///
    /// Returns the exact input-resolution or check-planning failure.
    pub fn prepare(
        self,
        current_directory: impl AsRef<Path>,
    ) -> Result<PreparedCheckCommand, PreparedCommandError> {
        let input = resolve_program_input(current_directory, self.input)
            .map_err(PreparedCommandError::Input)?;
        let plan =
            CheckCommandPlan::new(input, self.command).map_err(PreparedCommandError::Plan)?;
        Ok(PreparedCheckCommand {
            plan,
            resolution: self.resolution,
            format: self.format,
            target: self.target,
        })
    }
}

#[derive(Debug)]
pub struct ParsedFetchCommand {
    root: Option<PathBuf>,
    resolution: ResolutionOptions,
}

impl ParsedFetchCommand {
    /// Resolves the exact package selected by a fetch invocation.
    ///
    /// # Errors
    ///
    /// Returns the exact package-root input failure.
    pub fn prepare(
        self,
        current_directory: impl AsRef<Path>,
    ) -> Result<PreparedFetchCommand, PreparedCommandError> {
        let input = resolve_package_input(current_directory, self.root.as_deref())
            .map_err(PreparedCommandError::Input)?;
        Ok(PreparedFetchCommand {
            input,
            resolution: self.resolution,
        })
    }
}

#[derive(Debug)]
pub struct ParsedBuildCommand {
    input: ProgramInputOptions,
    command: BuildCommandOptions,
    resolution: ResolutionOptions,
    target: Option<CompilationTarget>,
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
            target: self.target,
        })
    }
}

#[derive(Debug)]
pub struct ParsedRunCommand {
    input: ProgramInputOptions,
    command: RunCommandOptions,
    resolution: ResolutionOptions,
    target: Option<CompilationTarget>,
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
            target: self.target,
        })
    }
}

#[derive(Debug)]
pub struct PreparedBuildCommand {
    plan: BuildCommandPlan,
    resolution: ResolutionOptions,
    target: Option<CompilationTarget>,
}

#[derive(Debug)]
pub struct PreparedCheckCommand {
    plan: CheckCommandPlan,
    resolution: ResolutionOptions,
    format: DiagnosticFormat,
    target: Option<CompilationTarget>,
}

#[derive(Debug)]
pub struct PreparedTestCommand {
    plan: TestCommandPlan,
    resolution: ResolutionOptions,
    format: DiagnosticFormat,
    target: Option<CompilationTarget>,
}

impl PreparedTestCommand {
    #[must_use]
    pub const fn plan(&self) -> &TestCommandPlan {
        &self.plan
    }

    #[must_use]
    pub const fn format(&self) -> DiagnosticFormat {
        self.format
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        TestCommandPlan,
        ResolutionOptions,
        DiagnosticFormat,
        Option<CompilationTarget>,
    ) {
        (self.plan, self.resolution, self.format, self.target)
    }
}

impl PreparedCheckCommand {
    #[must_use]
    pub const fn plan(&self) -> &CheckCommandPlan {
        &self.plan
    }

    #[must_use]
    pub const fn resolution(&self) -> ResolutionOptions {
        self.resolution
    }

    #[must_use]
    pub const fn format(&self) -> DiagnosticFormat {
        self.format
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        CheckCommandPlan,
        ResolutionOptions,
        DiagnosticFormat,
        Option<CompilationTarget>,
    ) {
        (self.plan, self.resolution, self.format, self.target)
    }
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

    pub(crate) fn into_parts(
        self,
    ) -> (
        BuildCommandPlan,
        ResolutionOptions,
        Option<CompilationTarget>,
    ) {
        (self.plan, self.resolution, self.target)
    }
}

#[derive(Debug)]
pub struct PreparedRunCommand {
    plan: RunCommandPlan,
    resolution: ResolutionOptions,
    target: Option<CompilationTarget>,
}

#[derive(Debug)]
pub struct PreparedFetchCommand {
    input: crate::PackageCommandInput,
    resolution: ResolutionOptions,
}

impl PreparedFetchCommand {
    #[must_use]
    pub const fn input(&self) -> &crate::PackageCommandInput {
        &self.input
    }

    #[must_use]
    pub const fn resolution(&self) -> ResolutionOptions {
        self.resolution
    }

    pub(crate) fn into_parts(self) -> (crate::PackageCommandInput, ResolutionOptions) {
        (self.input, self.resolution)
    }
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

    pub(crate) fn into_parts(
        self,
    ) -> (RunCommandPlan, ResolutionOptions, Option<CompilationTarget>) {
        (self.plan, self.resolution, self.target)
    }
}

/// Parses public command arguments without reading process state or the filesystem.
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
    parse_command_invocation(arguments).map_err(CommandArgumentFailure::into_error)
}

/// Parses public command arguments while retaining any successfully selected presentation mode
/// beside a failure.
///
/// This is the process-adapter entry point. Unlike [`parse_command_arguments`], its error keeps
/// enough information to honor `check --format json` without inspecting argv a second time.
///
/// # Errors
///
/// Returns the first structural argument error and the presentation state reached by the same
/// parse.
pub fn parse_command_invocation(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<ParsedCommand, CommandArgumentFailure> {
    let mut arguments = arguments.into_iter();
    let Some(command) = arguments.next() else {
        return Err(CommandArgumentFailure::new(
            CommandArgumentError::MissingCommand,
            None,
            DiagnosticFormat::Human,
            None,
        ));
    };
    let arguments = arguments.collect::<Vec<_>>();
    if command == OsStr::new("--help") {
        return match arguments.into_iter().next() {
            Some(_) => Err(CommandArgumentFailure::new(
                CommandArgumentError::HelpMustBeUsedAlone("--help"),
                Some("--help"),
                DiagnosticFormat::Human,
                None,
            )),
            None => Ok(ParsedCommand::Help(crate::HelpRequest::overview())),
        };
    }
    let Some(kind) = CommandKind::from_invocation(&command) else {
        return Err(CommandArgumentFailure::new(
            CommandArgumentError::UnknownCommand(command),
            None,
            DiagnosticFormat::Human,
            None,
        ));
    };
    if arguments.len() == 1
        && arguments[0] == OsStr::new("--help")
        && kind.schema().accepts(CommandOption::Help)
    {
        return Ok(ParsedCommand::Help(crate::HelpRequest::command(kind)));
    }
    match kind {
        CommandKind::Help => parse_help(arguments.into_iter())
            .map(ParsedCommand::Help)
            .map_err(|failure| failure.for_command("help")),
        CommandKind::Version => parse_empty_command(arguments.into_iter(), kind.schema())
            .map(|()| ParsedCommand::Version)
            .map_err(|failure| failure.for_command("--version")),
        CommandKind::Doctor => parse_empty_command(arguments.into_iter(), kind.schema())
            .map(|()| ParsedCommand::Doctor)
            .map_err(|failure| failure.for_command("doctor")),
        CommandKind::Fetch => parse_fetch(arguments.into_iter())
            .map(ParsedCommand::Fetch)
            .map_err(|failure| failure.for_command("fetch")),
        CommandKind::Check => parse_check(arguments.into_iter())
            .map(ParsedCommand::Check)
            .map_err(|failure| failure.for_command("check")),
        CommandKind::Build => parse_build(arguments.into_iter())
            .map(ParsedCommand::Build)
            .map_err(|failure| failure.for_command("build")),
        CommandKind::Run => parse_run(arguments.into_iter())
            .map(ParsedCommand::Run)
            .map_err(|failure| failure.for_command("run")),
        CommandKind::Test => parse_test(arguments.into_iter())
            .map(ParsedCommand::Test)
            .map_err(|failure| failure.for_command("test")),
    }
}

fn parse_empty_command(
    arguments: impl Iterator<Item = OsString>,
    schema: &'static CommandSchema,
) -> Result<(), OptionsParseFailure> {
    parse_options(arguments, schema).map(|_| ())
}

fn parse_help(
    mut arguments: impl Iterator<Item = OsString>,
) -> Result<crate::HelpRequest, OptionsParseFailure> {
    let Some(topic) = arguments.next() else {
        return Ok(crate::HelpRequest::overview());
    };
    let Some(command) = CommandKind::from_invocation(&topic) else {
        return Err(OptionsParseFailure::plain(
            CommandArgumentError::UnknownHelpTopic(topic),
        ));
    };
    if let Some(extra) = arguments.next() {
        return Err(OptionsParseFailure::plain(
            CommandArgumentError::MultipleHelpTopics(extra),
        ));
    }
    Ok(crate::HelpRequest::command(command))
}

fn parse_check(
    arguments: impl Iterator<Item = OsString>,
) -> Result<ParsedCheckCommand, OptionsParseFailure> {
    let ParsedOptions {
        root,
        file,
        positional,
        executable,
        output: _,
        resolution,
        format,
        target,
        test: _,
        case: _,
    } = parse_options(arguments, CommandKind::Check.schema())?;
    Ok(ParsedCheckCommand {
        input: ProgramInputOptions::new(root, positional, file),
        command: CheckCommandOptions::new(executable),
        resolution,
        format: format.unwrap_or_default(),
        target,
    })
}

fn parse_fetch(
    arguments: impl Iterator<Item = OsString>,
) -> Result<ParsedFetchCommand, OptionsParseFailure> {
    let ParsedOptions {
        root,
        file: _,
        positional: _,
        executable: _,
        output: _,
        resolution,
        format: _,
        target: _,
        test: _,
        case: _,
    } = parse_options(arguments, CommandKind::Fetch.schema())?;
    Ok(ParsedFetchCommand { root, resolution })
}

fn parse_build(
    arguments: impl Iterator<Item = OsString>,
) -> Result<ParsedBuildCommand, OptionsParseFailure> {
    let ParsedOptions {
        root,
        file,
        positional,
        executable,
        output,
        resolution,
        format: _,
        target,
        test: _,
        case: _,
    } = parse_options(arguments, CommandKind::Build.schema())?;
    Ok(ParsedBuildCommand {
        input: ProgramInputOptions::new(root, positional, file),
        command: BuildCommandOptions::new(executable, output),
        resolution,
        target,
    })
}

fn parse_run(
    arguments: impl Iterator<Item = OsString>,
) -> Result<ParsedRunCommand, OptionsParseFailure> {
    let ParsedOptions {
        root,
        file,
        positional,
        executable,
        output: _,
        resolution,
        format: _,
        target,
        test: _,
        case: _,
    } = parse_options(arguments, CommandKind::Run.schema())?;
    Ok(ParsedRunCommand {
        input: ProgramInputOptions::new(root, positional, file),
        command: RunCommandOptions::new(executable),
        resolution,
        target,
    })
}

fn parse_test(
    arguments: impl Iterator<Item = OsString>,
) -> Result<ParsedTestCommand, OptionsParseFailure> {
    let ParsedOptions {
        root,
        file: _,
        positional: _,
        executable: _,
        output: _,
        resolution,
        format,
        target,
        test,
        case,
    } = parse_options(arguments, CommandKind::Test.schema())?;
    Ok(ParsedTestCommand {
        root,
        command: TestCommandOptions::new(test, case),
        resolution,
        format: format.unwrap_or_default(),
        target,
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
    format: Option<DiagnosticFormat>,
    target: Option<CompilationTarget>,
    test: Option<Box<str>>,
    case: Option<Box<str>>,
}

fn parse_options(
    mut arguments: impl Iterator<Item = OsString>,
    schema: &'static CommandSchema,
) -> Result<ParsedOptions, OptionsParseFailure> {
    let mut parsed = ParsedOptions::default();
    let mut first_error = None;
    let mut positional_only = false;
    while let Some(argument) = arguments.next() {
        if !positional_only && argument == OsStr::new("--") {
            positional_only = true;
            continue;
        }
        if !positional_only {
            if let Some((name, value)) = split_long_option(&argument) {
                retain_first_error(
                    &mut first_error,
                    parse_valued_option(&mut parsed, name, Some(value), &mut arguments, schema),
                );
                continue;
            }
            if let Some(name) = argument.to_str().filter(|value| value.starts_with('-')) {
                match option_schema(name).copied() {
                    Some(option) if option.takes_value() => {
                        retain_first_error(
                            &mut first_error,
                            parse_valued_option(&mut parsed, name, None, &mut arguments, schema),
                        );
                    }
                    Some(option) => retain_first_error(
                        &mut first_error,
                        parse_flag_option(&mut parsed, option, schema),
                    ),
                    None => {
                        first_error.get_or_insert(CommandArgumentError::UnknownOption(argument));
                    }
                }
                continue;
            }
        }
        if !schema.accepts_positional() {
            first_error.get_or_insert(CommandArgumentError::PositionalNotAccepted {
                command: schema.name(),
                argument,
            });
            continue;
        }
        if parsed
            .positional
            .replace(PathBuf::from(&argument))
            .is_some()
        {
            first_error.get_or_insert(CommandArgumentError::MultiplePositionalSources(argument));
        }
    }
    match first_error {
        Some(error) => {
            let format = parsed.format.unwrap_or_default();
            let root_hint = ProgramInputOptions::new(
                parsed.root.clone(),
                parsed.positional.clone(),
                parsed.file.clone(),
            )
            .selected_root_hint();
            Err(OptionsParseFailure {
                error,
                format,
                root_hint,
            })
        }
        None => Ok(parsed),
    }
}

fn retain_first_error(
    first_error: &mut Option<CommandArgumentError>,
    result: Result<(), CommandArgumentError>,
) {
    if let Err(error) = result {
        first_error.get_or_insert(error);
    }
}

struct OptionsParseFailure {
    error: CommandArgumentError,
    format: DiagnosticFormat,
    root_hint: Option<PathBuf>,
}

impl OptionsParseFailure {
    fn plain(error: CommandArgumentError) -> Self {
        Self {
            error,
            format: DiagnosticFormat::Human,
            root_hint: None,
        }
    }

    fn for_command(self, command: &'static str) -> CommandArgumentFailure {
        let root_hint = if matches!(command, "check" | "test") {
            self.root_hint
        } else {
            None
        };
        CommandArgumentFailure::new(self.error, Some(command), self.format, root_hint)
    }
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
    command: &'static CommandSchema,
) -> Result<(), CommandArgumentError> {
    let Some(schema) = option_schema(name).copied() else {
        return Err(CommandArgumentError::UnknownOption(name.into()));
    };
    if !command.accepts(schema.option()) {
        return Err(CommandArgumentError::OptionNotAccepted {
            option: schema.canonical_name(),
            command: command.name(),
        });
    }
    if !schema.takes_value() {
        return Err(CommandArgumentError::OptionDoesNotTakeValue(
            schema.canonical_name(),
        ));
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
    match schema.option() {
        CommandOption::Root => set_path(&mut parsed.root, value, schema.canonical_name()),
        CommandOption::File => set_path(&mut parsed.file, value, schema.canonical_name()),
        CommandOption::Executable => {
            let value = value
                .into_string()
                .map_err(CommandArgumentError::NonUnicodeExecutable)?;
            if value.is_empty() {
                return Err(CommandArgumentError::EmptyExecutable);
            }
            set_value(
                &mut parsed.executable,
                value.into(),
                schema.canonical_name(),
            )
        }
        CommandOption::Output => set_path(&mut parsed.output, value, schema.canonical_name()),
        CommandOption::Format => {
            let value = value
                .into_string()
                .map_err(CommandArgumentError::NonUnicodeFormat)?;
            let format = match value.as_str() {
                "json" => DiagnosticFormat::Json,
                _ => return Err(CommandArgumentError::UnsupportedFormat(value.into())),
            };
            set_value(&mut parsed.format, format, schema.canonical_name())
        }
        CommandOption::Target => {
            let value = value
                .into_string()
                .map_err(CommandArgumentError::NonUnicodeTarget)?;
            let target = CompilationTarget::from_name(&value)
                .ok_or_else(|| CommandArgumentError::UnknownTarget(value.into()))?;
            set_value(&mut parsed.target, target, schema.canonical_name())
        }
        CommandOption::Test => set_name(
            &mut parsed.test,
            value,
            schema.canonical_name(),
            CommandArgumentError::NonUnicodeTest,
            CommandArgumentError::EmptyTest,
        ),
        CommandOption::Case => set_name(
            &mut parsed.case,
            value,
            schema.canonical_name(),
            CommandArgumentError::NonUnicodeCase,
            CommandArgumentError::EmptyCase,
        ),
        CommandOption::Help | CommandOption::Locked | CommandOption::Offline => {
            unreachable!("flag option passed the valued-option boundary")
        }
    }
}

fn parse_flag_option(
    parsed: &mut ParsedOptions,
    option: crate::command_schema::OptionSchema,
    command: &'static CommandSchema,
) -> Result<(), CommandArgumentError> {
    if !command.accepts(option.option()) {
        return Err(CommandArgumentError::OptionNotAccepted {
            option: option.canonical_name(),
            command: command.name(),
        });
    }
    match option.option() {
        CommandOption::Help => Err(CommandArgumentError::HelpMustBeUsedAlone(command.name())),
        CommandOption::Locked => set_flag(&mut parsed.resolution.locked, option.canonical_name()),
        CommandOption::Offline => set_flag(&mut parsed.resolution.offline, option.canonical_name()),
        CommandOption::Root
        | CommandOption::File
        | CommandOption::Executable
        | CommandOption::Output
        | CommandOption::Format
        | CommandOption::Target
        | CommandOption::Test
        | CommandOption::Case => unreachable!("valued option passed the flag-option boundary"),
    }
}

fn set_path(
    slot: &mut Option<PathBuf>,
    value: OsString,
    name: &'static str,
) -> Result<(), CommandArgumentError> {
    set_value(slot, PathBuf::from(value), name)
}

fn set_name(
    slot: &mut Option<Box<str>>,
    value: OsString,
    option: &'static str,
    non_unicode: fn(OsString) -> CommandArgumentError,
    empty: CommandArgumentError,
) -> Result<(), CommandArgumentError> {
    let value = value.into_string().map_err(non_unicode)?;
    if value.is_empty() {
        return Err(empty);
    }
    set_value(slot, value.into(), option)
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
    UnknownHelpTopic(OsString),
    MultipleHelpTopics(OsString),
    OptionNotAccepted {
        option: &'static str,
        command: &'static str,
    },
    MissingValue(Box<str>),
    OptionDoesNotTakeValue(&'static str),
    DuplicateOption(&'static str),
    HelpMustBeUsedAlone(&'static str),
    MultiplePositionalSources(OsString),
    PositionalNotAccepted {
        command: &'static str,
        argument: OsString,
    },
    NonUnicodeExecutable(OsString),
    NonUnicodeFormat(OsString),
    UnsupportedFormat(Box<str>),
    EmptyExecutable,
    NonUnicodeTarget(OsString),
    UnknownTarget(Box<str>),
    NonUnicodeTest(OsString),
    EmptyTest,
    NonUnicodeCase(OsString),
    EmptyCase,
}

/// A pure argument failure plus the output selection completed by the same parse.
#[derive(Debug, Eq, PartialEq)]
pub struct CommandArgumentFailure {
    error: CommandArgumentError,
    command: Option<&'static str>,
    format: DiagnosticFormat,
    root_hint: Option<PathBuf>,
}

impl CommandArgumentFailure {
    fn new(
        error: CommandArgumentError,
        command: Option<&'static str>,
        format: DiagnosticFormat,
        root_hint: Option<PathBuf>,
    ) -> Self {
        Self {
            error,
            command,
            format,
            root_hint,
        }
    }

    #[must_use]
    pub const fn error(&self) -> &CommandArgumentError {
        &self.error
    }

    #[must_use]
    pub const fn command(&self) -> Option<&'static str> {
        self.command
    }

    #[must_use]
    pub const fn format(&self) -> DiagnosticFormat {
        self.format
    }

    #[must_use]
    pub fn root_hint(&self) -> Option<&Path> {
        self.root_hint.as_deref()
    }

    #[must_use]
    pub fn into_error(self) -> CommandArgumentError {
        self.error
    }
}

impl fmt::Display for CommandArgumentFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for CommandArgumentFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
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
            Self::UnknownHelpTopic(topic) => {
                write!(formatter, "unknown help topic {}", topic.to_string_lossy())
            }
            Self::MultipleHelpTopics(topic) => write!(
                formatter,
                "help accepts at most one command; unexpected {}",
                topic.to_string_lossy()
            ),
            Self::OptionNotAccepted { option, command } => {
                write!(formatter, "{option} is not accepted by {command}")
            }
            Self::MissingValue(option) => write!(formatter, "{option} requires a value"),
            Self::OptionDoesNotTakeValue(option) => {
                write!(formatter, "{option} does not accept a value")
            }
            Self::DuplicateOption(option) => {
                write!(formatter, "{option} was provided more than once")
            }
            Self::HelpMustBeUsedAlone(command) => {
                write!(formatter, "--help must be used alone after {command}")
            }
            Self::MultiplePositionalSources(source) => write!(
                formatter,
                "more than one positional source was provided; unexpected {}",
                source.to_string_lossy()
            ),
            Self::PositionalNotAccepted { command, argument } => write!(
                formatter,
                "{command} does not accept a source path; unexpected {}",
                argument.to_string_lossy()
            ),
            Self::NonUnicodeExecutable(name) => write!(
                formatter,
                "executable name is not Unicode: {}",
                name.to_string_lossy()
            ),
            Self::NonUnicodeFormat(format) => write!(
                formatter,
                "diagnostic format is not Unicode: {}",
                format.to_string_lossy()
            ),
            Self::UnsupportedFormat(format) => {
                write!(formatter, "unsupported diagnostic format {format}")
            }
            Self::EmptyExecutable => formatter.write_str("executable name cannot be empty"),
            Self::NonUnicodeTarget(target) => write!(
                formatter,
                "compilation target is not Unicode: {}",
                target.to_string_lossy()
            ),
            Self::UnknownTarget(target) => {
                write!(formatter, "unknown compilation target {target}")
            }
            Self::NonUnicodeTest(name) => write!(
                formatter,
                "test target name is not Unicode: {}",
                name.to_string_lossy()
            ),
            Self::EmptyTest => formatter.write_str("test target name cannot be empty"),
            Self::NonUnicodeCase(name) => write!(
                formatter,
                "test case name is not Unicode: {}",
                name.to_string_lossy()
            ),
            Self::EmptyCase => formatter.write_str("test case name cannot be empty"),
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

    #[test]
    fn installation_reports_have_an_exact_argument_surface() {
        assert!(matches!(
            parse_command_arguments(arguments(&["--version"])).unwrap(),
            ParsedCommand::Version
        ));
        assert!(matches!(
            parse_command_arguments(arguments(&["doctor"])).unwrap(),
            ParsedCommand::Doctor
        ));
        assert_eq!(
            parse_command_arguments(arguments(&["--version", "extra"])).unwrap_err(),
            CommandArgumentError::PositionalNotAccepted {
                command: "--version",
                argument: "extra".into(),
            }
        );
        assert_eq!(
            parse_command_arguments(arguments(&["doctor", "--offline"])).unwrap_err(),
            CommandArgumentError::OptionNotAccepted {
                option: "--offline",
                command: "doctor",
            }
        );
    }

    #[test]
    fn target_and_test_options_keep_distinct_typed_namespaces() {
        let parsed = parse_command_arguments(arguments(&[
            "test",
            "--test",
            "unit",
            "--case",
            "pushes",
            "--target",
            "arm64-darwin",
            "--format",
            "json",
        ]))
        .unwrap();
        assert_eq!(
            parsed.requested_target(),
            Some(CompilationTarget::Arm64Darwin)
        );
        assert!(matches!(parsed, ParsedCommand::Test(_)));
        assert_eq!(
            parse_command_arguments(arguments(&["test", "source.nct"])).unwrap_err(),
            CommandArgumentError::PositionalNotAccepted {
                command: "test",
                argument: "source.nct".into(),
            }
        );
        assert_eq!(
            parse_command_arguments(arguments(&["test", "--file", "source.nct"])).unwrap_err(),
            CommandArgumentError::OptionNotAccepted {
                option: "--file",
                command: "test",
            }
        );
        assert!(matches!(
            parse_command_arguments(arguments(&["check", "--target", "mips-templeos"])),
            Err(CommandArgumentError::UnknownTarget(target)) if target.as_ref() == "mips-templeos"
        ));
    }

    #[test]
    fn every_help_spelling_converges_on_the_schema_owned_report() {
        let ParsedCommand::Help(global_option) =
            parse_command_arguments(arguments(&["--help"])).unwrap()
        else {
            panic!("expected help command");
        };
        let ParsedCommand::Help(help_command) =
            parse_command_arguments(arguments(&["help"])).unwrap()
        else {
            panic!("expected help command");
        };
        assert_eq!(global_option, help_command);

        let ParsedCommand::Help(selected_form) =
            parse_command_arguments(arguments(&["help", "check"])).unwrap()
        else {
            panic!("expected selected help command");
        };
        let ParsedCommand::Help(option_form) =
            parse_command_arguments(arguments(&["check", "--help"])).unwrap()
        else {
            panic!("expected selected help command");
        };
        assert_eq!(selected_form, option_form);
        assert!(
            selected_form
                .render()
                .contains("nocter check [OPTIONS] [SOURCE]")
        );
        assert!(selected_form.render().contains("--format <FORMAT>"));
        assert!(!selected_form.render().contains("--output"));

        assert!(matches!(
            parse_command_arguments(arguments(&["help", "missing"])),
            Err(CommandArgumentError::UnknownHelpTopic(_))
        ));
        assert!(matches!(
            parse_command_arguments(arguments(&["help", "check", "build"])),
            Err(CommandArgumentError::MultipleHelpTopics(_))
        ));
        assert_eq!(
            parse_command_arguments(arguments(&["check", "--help", "app.nct"])).unwrap_err(),
            CommandArgumentError::HelpMustBeUsedAlone("check")
        );
        assert_eq!(
            parse_command_arguments(arguments(&["--help", "check"])).unwrap_err(),
            CommandArgumentError::HelpMustBeUsedAlone("--help")
        );
        assert_eq!(
            parse_command_arguments(arguments(&["fetch", "--locked=true"])).unwrap_err(),
            CommandArgumentError::OptionDoesNotTakeValue("--locked")
        );
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
    fn fetch_accepts_only_package_and_resolution_options() {
        let root = package_root();
        let parsed =
            parse_command_arguments(arguments(&["fetch", "--root=.", "--locked", "--offline"]))
                .unwrap();
        let ParsedCommand::Fetch(parsed) = parsed else {
            panic!("expected fetch command");
        };
        let prepared = parsed.prepare(&root).unwrap();

        assert_eq!(prepared.input().root(), &fs::canonicalize(&root).unwrap());
        assert!(prepared.resolution().locked());
        assert!(prepared.resolution().offline());
        assert_eq!(
            parse_command_arguments(arguments(&["fetch", "source.nct"])).unwrap_err(),
            CommandArgumentError::PositionalNotAccepted {
                command: "fetch",
                argument: "source.nct".into(),
            }
        );
        assert_eq!(
            parse_command_arguments(arguments(&["fetch", "--file", "source.nct"])).unwrap_err(),
            CommandArgumentError::OptionNotAccepted {
                option: "--file",
                command: "fetch",
            }
        );
        assert_eq!(
            parse_command_arguments(arguments(&["fetch", "--executable", "app"])).unwrap_err(),
            CommandArgumentError::OptionNotAccepted {
                option: "--executable",
                command: "fetch",
            }
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn check_prepares_package_and_file_selection_without_output_authority() {
        let root = package_root();
        let parsed = parse_command_arguments(arguments(&[
            "check",
            "--root=.",
            "--executable",
            "tool",
            "--locked",
            "--format=json",
        ]))
        .unwrap();
        let ParsedCommand::Check(parsed) = parsed else {
            panic!("expected check command");
        };
        let prepared = parsed.prepare(&root).unwrap();
        assert_eq!(prepared.plan().executable(), Some("tool"));
        assert!(prepared.resolution().locked());
        assert_eq!(prepared.format(), DiagnosticFormat::Json);
        assert_eq!(
            parse_command_arguments(arguments(&["check", "--output", "program"])).unwrap_err(),
            CommandArgumentError::OptionNotAccepted {
                option: "--output",
                command: "check",
            }
        );
        assert_eq!(
            parse_command_arguments(arguments(&["check", "--format", "xml"])).unwrap_err(),
            CommandArgumentError::UnsupportedFormat("xml".into())
        );
        assert_eq!(
            parse_command_arguments(arguments(&["build", "--format", "json"])).unwrap_err(),
            CommandArgumentError::OptionNotAccepted {
                option: "--format",
                command: "build",
            }
        );

        fs::write(root.join("app.nct"), "func main(): void { return }\n").unwrap();
        let ParsedCommand::Check(parsed) =
            parse_command_arguments(arguments(&["check", "app.nct"])).unwrap()
        else {
            panic!("expected check command");
        };
        assert!(matches!(
            parsed.prepare(&root).unwrap().plan().input(),
            ResolvedProgramInput::SingleFile(_)
        ));
        fs::remove_dir_all(root).unwrap();
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
