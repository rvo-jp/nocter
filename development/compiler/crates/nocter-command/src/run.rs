use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use nocter_diagnostics::SourceDiagnostic;
use nocter_native_session::{NativeSessionError, compile_native_image};
use nocter_session::ExecutableCompileRequest;
use nocter_source_index::SourceIndex;

use crate::{ArtifactError, stage_temporary_image};

/// One completed child process and the independent source projection from its compile session.
#[derive(Debug)]
pub struct ExecutedProgram {
    status: ExitStatus,
    source_index: SourceIndex,
}

impl ExecutedProgram {
    #[must_use]
    pub const fn status(&self) -> ExitStatus {
        self.status
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    #[must_use]
    pub fn into_parts(self) -> (ExitStatus, SourceIndex) {
        (self.status, self.source_index)
    }
}

/// Compiles, stages, launches, waits for, and removes one selected executable.
///
/// Standard input, output, and error are inherited from the command process. A program's nonzero
/// status is a successful command orchestration result and remains available through
/// [`ExecutedProgram::status`].
///
/// # Errors
///
/// Returns the exact compile, temporary-artifact, launch, or cleanup failure. If launch and cleanup
/// both fail, both errors remain available.
pub fn run_executable(
    request: ExecutableCompileRequest<'_>,
    working_directory: impl AsRef<Path>,
) -> Result<ExecutedProgram, RunCommandError> {
    let compiled = compile_native_image(request)?;
    let (image, source_index) = compiled.into_parts();
    let artifact = stage_temporary_image(image.bytes())?;
    let executable = artifact.path().to_path_buf();
    let launched = Command::new(&executable)
        .current_dir(working_directory)
        .status();
    let removed = artifact.remove();
    match (launched, removed) {
        (Ok(status), Ok(())) => Ok(ExecutedProgram {
            status,
            source_index,
        }),
        (Err(source), Ok(())) => Err(RunCommandError::Launch { executable, source }),
        (Ok(_), Err(error)) => Err(RunCommandError::Cleanup(error)),
        (Err(source), Err(cleanup)) => Err(RunCommandError::LaunchAndCleanup {
            executable,
            source,
            cleanup,
        }),
    }
}

#[derive(Debug)]
pub enum RunCommandError {
    Compile(NativeSessionError),
    Artifact(ArtifactError),
    Launch {
        executable: PathBuf,
        source: io::Error,
    },
    Cleanup(ArtifactError),
    LaunchAndCleanup {
        executable: PathBuf,
        source: io::Error,
        cleanup: ArtifactError,
    },
}

impl RunCommandError {
    #[must_use]
    pub fn source_diagnostics(&self) -> &[SourceDiagnostic] {
        match self {
            Self::Compile(error) => error.source_diagnostics(),
            Self::Artifact(_)
            | Self::Launch { .. }
            | Self::Cleanup(_)
            | Self::LaunchAndCleanup { .. } => &[],
        }
    }

    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<&'static str> {
        match self {
            Self::Compile(error) => error.diagnostic_code(),
            Self::Artifact(_)
            | Self::Launch { .. }
            | Self::Cleanup(_)
            | Self::LaunchAndCleanup { .. } => Some("E0704"),
        }
    }
}

impl fmt::Display for RunCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile(error) => write!(formatter, "native compilation failed: {error}"),
            Self::Artifact(error) => write!(formatter, "temporary executable failed: {error}"),
            Self::Launch { executable, source } => {
                write!(
                    formatter,
                    "failed to launch {}: {source}",
                    executable.display()
                )
            }
            Self::Cleanup(error) => {
                write!(formatter, "temporary executable cleanup failed: {error}")
            }
            Self::LaunchAndCleanup {
                executable,
                source,
                cleanup,
            } => write!(
                formatter,
                "failed to launch {}: {source}; cleanup also failed: {cleanup}",
                executable.display()
            ),
        }
    }
}

impl std::error::Error for RunCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Compile(error) => Some(error),
            Self::Artifact(error) | Self::Cleanup(error) => Some(error),
            Self::Launch { source, .. } | Self::LaunchAndCleanup { source, .. } => Some(source),
        }
    }
}

impl From<NativeSessionError> for RunCommandError {
    fn from(error: NativeSessionError) -> Self {
        Self::Compile(error)
    }
}

impl From<ArtifactError> for RunCommandError {
    fn from(error: ArtifactError) -> Self {
        Self::Artifact(error)
    }
}
