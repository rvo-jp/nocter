//! Public process adapter for the Nocter compiler.
//!
//! This crate reads process facts once and composes existing installation and command boundaries.
//! It owns no argument grammar, package interpretation, compiler stage, or backend decision.

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use nocter_command::{
    BuildCommandExecutionError, BuildCommandResult, CheckCommandExecutionError,
    CheckCommandPresentation, CheckCommandResult, CommandArgumentError, CommandToolchain,
    DiagnosticFormat, ExecutedProgram, FetchCommandExecutionError, FetchCommandResult,
    ParsedCommand, PreparedCommandError, ProgramInputError, RunCommandExecutionError,
    execute_prepared_build, execute_prepared_check, execute_prepared_fetch, execute_prepared_run,
    parse_command_arguments,
};
use nocter_diagnostics::{
    DiagnosticJsonContext, DiagnosticRenderError, render_source_diagnostic,
    render_source_diagnostics_json,
};
use nocter_installation::{NocterHome, NocterHomeError, NocterHomeRequest};
use nocter_package_acquisition::{EmbeddedPackageAcquisition, PackageAcquisitionError};

mod host;
mod process;

pub use host::build_host;
pub use process::{CurrentProcessError, execute_current_process};

/// Explicit process facts for one command invocation.
#[derive(Clone, Debug)]
pub struct Invocation {
    arguments: Vec<OsString>,
    current_directory: PathBuf,
    configured_home: Option<OsString>,
    executable: PathBuf,
    compiler_host: Box<str>,
}

impl Invocation {
    #[must_use]
    pub fn new(
        arguments: impl IntoIterator<Item = OsString>,
        current_directory: impl Into<PathBuf>,
        configured_home: Option<OsString>,
        executable: impl Into<PathBuf>,
        compiler_host: impl Into<Box<str>>,
    ) -> Self {
        Self {
            arguments: arguments.into_iter().collect(),
            current_directory: current_directory.into(),
            configured_home,
            executable: executable.into(),
            compiler_host: compiler_host.into(),
        }
    }
}

/// Completed command outcome. A run retains the child status independently from orchestration
/// success.
#[derive(Debug)]
pub enum InvocationOutcome {
    Fetch(FetchCommandResult),
    Check(Box<CheckCommandResult>),
    Build(BuildCommandResult),
    Run(ExecutedProgram),
}

impl InvocationOutcome {
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Fetch(_) | Self::Check(_) | Self::Build(_) => 0,
            Self::Run(executed) => executed.status().code().unwrap_or(1),
        }
    }

    /// Renders successful machine-readable output when selected by the command.
    ///
    /// # Errors
    ///
    /// Returns a presentation-integrity failure for a non-Unicode root path.
    pub fn render_json_diagnostics(&self) -> Result<Option<String>, DiagnosticRenderError> {
        match self {
            Self::Check(result) if result.format() == DiagnosticFormat::Json => {
                render_source_diagnostics_json(
                    check_json_context(result.presentation())?,
                    &[],
                    &nocter_source::SourceMap::new(),
                )
                .map(Some)
            }
            Self::Fetch(_) | Self::Check(_) | Self::Build(_) | Self::Run(_) => Ok(None),
        }
    }
}

/// Executes one explicit invocation through the sole installation and command pipelines.
///
/// Argument syntax is checked before installation or source filesystem access. Installation
/// validation and host matching complete before the selected user input is prepared.
///
/// # Errors
///
/// Returns the exact argument, installation, host, preparation, build, or run boundary failure.
pub fn execute_invocation(invocation: Invocation) -> Result<InvocationOutcome, InvocationError> {
    let Invocation {
        arguments,
        current_directory,
        configured_home,
        executable,
        compiler_host,
    } = invocation;
    let command = parse_command_arguments(arguments).map_err(InvocationError::Arguments)?;
    let home = NocterHome::resolve(NocterHomeRequest::new(configured_home, executable))
        .map_err(InvocationError::Installation)?;
    if home.manifest().host() != compiler_host.as_ref() {
        return Err(InvocationError::HostMismatch {
            compiler: compiler_host,
            installation: home.manifest().host().into(),
        });
    }
    let toolchain = CommandToolchain::new(
        home.manifest().default_target(),
        home.root(),
        home.standard_package(),
    );
    match command {
        ParsedCommand::Fetch(command) => {
            let command = command
                .prepare(current_directory)
                .map_err(InvocationError::Preparation)?;
            let mut acquisition = EmbeddedPackageAcquisition::new()
                .map_err(InvocationError::AcquisitionInitialization)?;
            execute_prepared_fetch(command, toolchain.packages(), &mut acquisition)
                .map(InvocationOutcome::Fetch)
                .map_err(|error| InvocationError::Fetch(Box::new(error)))
        }
        ParsedCommand::Check(command) => {
            let command = command
                .prepare(current_directory)
                .map_err(InvocationError::Preparation)?;
            let mut acquisition = EmbeddedPackageAcquisition::new()
                .map_err(InvocationError::AcquisitionInitialization)?;
            execute_prepared_check(command, &toolchain, &mut acquisition)
                .map(|result| InvocationOutcome::Check(Box::new(result)))
                .map_err(|error| InvocationError::Check(Box::new(error)))
        }
        ParsedCommand::Build(command) => {
            let command = command
                .prepare(current_directory)
                .map_err(InvocationError::Preparation)?;
            let mut acquisition = EmbeddedPackageAcquisition::new()
                .map_err(InvocationError::AcquisitionInitialization)?;
            execute_prepared_build(command, &toolchain, &mut acquisition)
                .map(InvocationOutcome::Build)
                .map_err(|error| InvocationError::Build(Box::new(error)))
        }
        ParsedCommand::Run(command) => {
            let command = command
                .prepare(current_directory)
                .map_err(InvocationError::Preparation)?;
            let mut acquisition = EmbeddedPackageAcquisition::new()
                .map_err(InvocationError::AcquisitionInitialization)?;
            execute_prepared_run(command, &toolchain, &mut acquisition)
                .map(InvocationOutcome::Run)
                .map_err(|error| InvocationError::Run(Box::new(error)))
        }
    }
}

#[derive(Debug)]
pub enum InvocationError {
    Arguments(CommandArgumentError),
    Installation(NocterHomeError),
    HostMismatch {
        compiler: Box<str>,
        installation: Box<str>,
    },
    AcquisitionInitialization(PackageAcquisitionError),
    Preparation(PreparedCommandError),
    Fetch(Box<FetchCommandExecutionError>),
    Check(Box<CheckCommandExecutionError>),
    Build(Box<BuildCommandExecutionError>),
    Run(Box<RunCommandExecutionError>),
}

impl InvocationError {
    /// Returns a spanless CLI code only when this boundary can classify it without erasing a
    /// source-backed diagnostic owned by a compiler stage.
    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<&'static str> {
        match self {
            Self::Arguments(_)
            | Self::Preparation(
                PreparedCommandError::Plan(_)
                | PreparedCommandError::Input(
                    ProgramInputError::ConflictingFileForms
                    | ProgramInputError::RootWithFile
                    | ProgramInputError::InvalidSourceExtension(_),
                ),
            ) => Some("E0700"),
            Self::Installation(_) | Self::HostMismatch { .. } => Some("E0703"),
            Self::Preparation(PreparedCommandError::Input(
                ProgramInputError::MissingPackageDeclaration(_)
                | ProgramInputError::PackageDeclarationNotFile(_),
            )) => Some("E0800"),
            Self::Preparation(PreparedCommandError::Input(
                ProgramInputError::PackageRootNotDirectory(_)
                | ProgramInputError::SourceNotFile(_)
                | ProgramInputError::Filesystem { .. },
            )) => Some("E0702"),
            Self::AcquisitionInitialization(_)
            | Self::Fetch(_)
            | Self::Check(_)
            | Self::Build(_)
            | Self::Run(_) => None,
        }
    }

    /// Renders diagnostics already selected by source-processing phases.
    ///
    /// The process boundary does not inspect compiler error variants or reopen source files.
    ///
    /// # Errors
    ///
    /// Returns an integrity failure when a retained diagnostic does not belong to its invocation
    /// source snapshot.
    pub fn render_source_diagnostics(&self) -> Result<Option<String>, DiagnosticRenderError> {
        let context = match self {
            Self::Build(error) => error.source_diagnostics(),
            Self::Check(error) => error.source_diagnostics(),
            Self::Run(error) => error.source_diagnostics(),
            Self::Arguments(_)
            | Self::Installation(_)
            | Self::HostMismatch { .. }
            | Self::AcquisitionInitialization(_)
            | Self::Fetch(_)
            | Self::Preparation(_) => None,
        };
        let Some((diagnostics, sources)) =
            context.filter(|(diagnostics, _)| !diagnostics.is_empty())
        else {
            return Ok(None);
        };
        let mut output = String::new();
        for diagnostic in diagnostics {
            output.push_str(&render_source_diagnostic(diagnostic, sources)?);
        }
        Ok(Some(output))
    }

    /// Renders a failed JSON-formatted check from its retained diagnostic snapshot.
    ///
    /// # Errors
    ///
    /// Returns a source/range or root-path presentation-integrity failure.
    pub fn render_json_diagnostics(&self) -> Result<Option<String>, DiagnosticRenderError> {
        let Self::Check(error) = self else {
            return Ok(None);
        };
        if error.format() != DiagnosticFormat::Json {
            return Ok(None);
        }
        let Some((diagnostics, sources)) = error.source_diagnostics() else {
            return Ok(None);
        };
        render_source_diagnostics_json(
            check_json_context(error.presentation())?,
            diagnostics,
            sources,
        )
        .map(Some)
    }
}

fn check_json_context(
    presentation: &CheckCommandPresentation,
) -> Result<DiagnosticJsonContext<'_>, DiagnosticRenderError> {
    let root = presentation
        .root()
        .to_str()
        .ok_or(DiagnosticRenderError::NonUnicodePath)?;
    Ok(DiagnosticJsonContext::new(
        "check",
        Some(presentation.target().name()),
        Some(root),
        Some(root),
    ))
}

impl fmt::Display for InvocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arguments(error) => error.fmt(formatter),
            Self::Installation(error) => error.fmt(formatter),
            Self::HostMismatch {
                compiler,
                installation,
            } => write!(
                formatter,
                "Nocter home host `{installation}` does not match compiler host `{compiler}`"
            ),
            Self::AcquisitionInitialization(error) => {
                write!(formatter, "cannot initialize package acquisition: {error}")
            }
            Self::Preparation(error) => error.fmt(formatter),
            Self::Fetch(error) => error.fmt(formatter),
            Self::Check(error) => error.fmt(formatter),
            Self::Build(error) => error.fmt(formatter),
            Self::Run(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for InvocationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Arguments(error) => Some(error),
            Self::Installation(error) => Some(error),
            Self::Preparation(error) => Some(error),
            Self::AcquisitionInitialization(error) => Some(error),
            Self::Fetch(error) => Some(error),
            Self::Check(error) => Some(error),
            Self::Build(error) => Some(error),
            Self::Run(error) => Some(error),
            Self::HostMismatch { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests;
