//! Public process adapter for the Nocter compiler.
//!
//! This crate reads process facts once and composes existing installation and command boundaries.
//! It owns no argument grammar, package interpretation, compiler stage, or backend decision.

use std::ffi::OsString;
use std::path::PathBuf;

use nocter_command::{
    BuildCommandResult, CheckCommandPresentation, CheckCommandResult, CommandToolchain,
    DiagnosticFormat, ExecutedProgram, FetchCommandResult, FormatCommandResult, GraphCommandResult,
    HelpRequest, InitCommandResult, SourceInspectionCommandResult, parse_command_invocation,
};
use nocter_diagnostics::{
    DiagnosticJsonContext, DiagnosticRenderError, render_source_diagnostics_json,
};
use nocter_installation::{NocterHome, NocterHomeRequest};

mod dispatch;
mod error;
mod host;
mod lsp;
mod presentation;
mod process;
mod report;
mod test_report;

pub use error::{InvocationError, InvocationErrorKind, InvocationFailureClass};
pub use host::build_host;
pub use lsp::{LanguageServerLaunch, run_language_server_stdio};
use presentation::InvocationDiagnosticPresentation;
pub use process::{CurrentProcessError, execute_current_process};
pub use report::{DoctorReport, VersionReport};

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
    Help(HelpRequest),
    Version(VersionReport),
    Doctor(DoctorReport),
    Init(InitCommandResult),
    Graph(GraphCommandResult),
    Fetch(FetchCommandResult),
    Check(Box<CheckCommandResult>),
    Build(BuildCommandResult),
    Run(ExecutedProgram),
    Test(Box<nocter_command::TestCommandResult>),
    SourceInspection(SourceInspectionCommandResult),
    Format(FormatCommandResult),
    LanguageServer(Box<LanguageServerLaunch>),
}

impl InvocationOutcome {
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Help(_)
            | Self::Version(_)
            | Self::Doctor(_)
            | Self::Init(_)
            | Self::Graph(_)
            | Self::Fetch(_)
            | Self::Check(_)
            | Self::Build(_)
            | Self::Format(_)
            | Self::LanguageServer(_) => 0,
            Self::Run(executed) => executed.status().code().unwrap_or(1),
            Self::Test(result) => i32::from(!result.succeeded()),
            Self::SourceInspection(result) => i32::from(!result.succeeded()),
        }
    }

    /// Renders successful human output for commands whose result is a report.
    #[must_use]
    pub fn render_standard_output(&self) -> Option<String> {
        match self {
            Self::Help(request) => Some(request.render()),
            Self::Version(report) => Some(report.render()),
            Self::Doctor(report) => Some(report.render()),
            Self::Init(result) => Some(result.render()),
            Self::Graph(result) => Some(result.render()),
            Self::Test(result) => test_report::render_test_human(result),
            Self::SourceInspection(result) => Some(result.json().to_owned()),
            Self::Fetch(_)
            | Self::Check(_)
            | Self::Build(_)
            | Self::Run(_)
            | Self::Format(_)
            | Self::LanguageServer(_) => None,
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
            Self::Test(result) if result.presentation().format() == DiagnosticFormat::Json => {
                test_report::render_test_json(result).map(Some)
            }
            Self::Help(_)
            | Self::Version(_)
            | Self::Doctor(_)
            | Self::Init(_)
            | Self::Graph(_)
            | Self::Fetch(_)
            | Self::Check(_)
            | Self::Build(_)
            | Self::Run(_)
            | Self::Test(_)
            | Self::SourceInspection(_)
            | Self::Format(_)
            | Self::LanguageServer(_) => Ok(None),
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
    let command = parse_command_invocation(arguments).map_err(|failure| {
        let presentation = InvocationDiagnosticPresentation::from_argument_failure(&failure);
        InvocationError::new(InvocationErrorKind::Arguments(failure), presentation)
    })?;
    let requested_target = command.requested_target();
    let command = match dispatch::route(command) {
        dispatch::CommandRoute::Direct(command) => {
            return dispatch::execute_direct_command(command, &current_directory);
        }
        dispatch::CommandRoute::Installed(command) => command,
    };
    let mut presentation = InvocationDiagnosticPresentation::from_command(&command);
    let home = NocterHome::resolve(NocterHomeRequest::new(configured_home, executable)).map_err(
        |error| {
            InvocationError::new(
                InvocationErrorKind::Installation(error),
                presentation.clone(),
            )
        },
    )?;
    let installation = home.for_compiler(&compiler_host).map_err(|error| {
        InvocationError::new(
            InvocationErrorKind::InstallationCompatibility(error),
            presentation.clone(),
        )
    })?;
    if let Some(presentation) = presentation.as_mut() {
        presentation.target = Some(
            requested_target
                .unwrap_or(installation.manifest().default_target())
                .name(),
        );
    }
    let toolchain = CommandToolchain::new(
        installation.manifest().default_target(),
        installation.root(),
        installation.standard_package(),
    );
    dispatch::execute_installed_command(
        command,
        &current_directory,
        &installation,
        &toolchain,
        presentation,
    )
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

#[cfg(test)]
mod tests;
