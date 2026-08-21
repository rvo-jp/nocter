//! Public process adapter for the Nocter compiler.
//!
//! This crate reads process facts once and composes existing installation and command boundaries.
//! It owns no argument grammar, package interpretation, compiler stage, or backend decision.

use std::ffi::OsString;
use std::path::PathBuf;

use nocter_command::{
    BuildCommandResult, CheckCommandPresentation, CheckCommandResult, CommandToolchain,
    DiagnosticFormat, ExecutedProgram, FetchCommandResult, parse_command_invocation,
};
use nocter_diagnostics::{
    DiagnosticJsonContext, DiagnosticRenderError, render_source_diagnostics_json,
};
use nocter_installation::{NocterHome, NocterHomeRequest};

mod dispatch;
mod error;
mod host;
mod presentation;
mod process;

pub use error::{InvocationError, InvocationErrorKind, InvocationFailureClass};
pub use host::build_host;
use presentation::InvocationDiagnosticPresentation;
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
    let command = parse_command_invocation(arguments).map_err(|failure| {
        let presentation = InvocationDiagnosticPresentation::from_argument_failure(&failure);
        InvocationError::new(InvocationErrorKind::Arguments(failure), presentation)
    })?;
    let mut presentation = InvocationDiagnosticPresentation::from_command(&command);
    let home = NocterHome::resolve(NocterHomeRequest::new(configured_home, executable)).map_err(
        |error| {
            InvocationError::new(
                InvocationErrorKind::Installation(error),
                presentation.clone(),
            )
        },
    )?;
    if let Some(presentation) = presentation.as_mut() {
        presentation.target = Some(home.manifest().default_target().name());
    }
    if home.manifest().host() != compiler_host.as_ref() {
        return Err(InvocationError::new(
            InvocationErrorKind::HostMismatch {
                compiler: compiler_host,
                installation: home.manifest().host().into(),
            },
            presentation,
        ));
    }
    let toolchain = CommandToolchain::new(
        home.manifest().default_target(),
        home.root(),
        home.standard_package(),
    );
    dispatch::execute_parsed_command(command, &current_directory, &toolchain, presentation)
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
