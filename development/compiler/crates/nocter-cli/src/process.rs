use std::env;
use std::fmt;
use std::io;

use nocter_diagnostics::DiagnosticRenderError;

use crate::{Invocation, InvocationError, InvocationOutcome, build_host, execute_invocation};

/// Captures process-global command facts once and crosses the pure invocation boundary.
///
/// # Errors
///
/// Returns a typed process-state or invocation failure. It never searches for alternate working
/// directories, executables, or Nocter homes.
pub fn execute_current_process() -> Result<InvocationOutcome, CurrentProcessError> {
    let current_directory = env::current_dir().map_err(CurrentProcessError::CurrentDirectory)?;
    let executable = env::current_exe().map_err(CurrentProcessError::CurrentExecutable)?;
    let host = build_host().ok_or(CurrentProcessError::UnsupportedBuildHost)?;
    let invocation = Invocation::new(
        env::args_os().skip(1),
        current_directory,
        env::var_os("NOCTER_HOME"),
        executable,
        host,
    );
    execute_invocation(invocation).map_err(|error| CurrentProcessError::Invocation(Box::new(error)))
}

#[derive(Debug)]
pub enum CurrentProcessError {
    CurrentDirectory(io::Error),
    CurrentExecutable(io::Error),
    UnsupportedBuildHost,
    Invocation(Box<InvocationError>),
}

impl CurrentProcessError {
    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<&'static str> {
        match self {
            Self::CurrentDirectory(_) => Some("E0702"),
            Self::CurrentExecutable(_) | Self::UnsupportedBuildHost => Some("E0703"),
            Self::Invocation(error) => error.diagnostic_code(),
        }
    }

    /// Renders retained source diagnostics without classifying the nested compiler failure.
    ///
    /// # Errors
    ///
    /// Returns an integrity failure when a diagnostic and its invocation source snapshot disagree.
    pub fn render_source_diagnostics(&self) -> Result<Option<String>, DiagnosticRenderError> {
        match self {
            Self::Invocation(error) => error.render_source_diagnostics(),
            Self::CurrentDirectory(_) | Self::CurrentExecutable(_) | Self::UnsupportedBuildHost => {
                Ok(None)
            }
        }
    }
}

impl fmt::Display for CurrentProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentDirectory(error) => {
                write!(formatter, "cannot read the current directory: {error}")
            }
            Self::CurrentExecutable(error) => {
                write!(
                    formatter,
                    "cannot resolve the running Nocter executable: {error}"
                )
            }
            Self::UnsupportedBuildHost => {
                formatter.write_str("this compiler was built for an unsupported host")
            }
            Self::Invocation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CurrentProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CurrentDirectory(error) | Self::CurrentExecutable(error) => Some(error),
            Self::Invocation(error) => Some(error),
            Self::UnsupportedBuildHost => None,
        }
    }
}
