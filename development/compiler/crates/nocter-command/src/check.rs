use std::fmt;
use std::path::{Path, PathBuf};

use nocter_model::CompilationTarget;
use nocter_package_state::PackageAcquisitionAuthority;
use nocter_session::{CompileSessionError, CompiledTarget, compile_target};

use crate::failure::command_compilation_failure;
use crate::source::{CommandCompileRoots, discover_command_source};
use crate::{
    CommandCompilationFailure, CommandSourceError, CommandToolchain, DiagnosticFormat,
    PreparedCheckCommand,
};

/// Target-validated result of one check command.
#[derive(Debug)]
pub struct CheckCommandResult {
    target: CompiledTarget,
    presentation: CheckCommandPresentation,
}

impl CheckCommandResult {
    #[must_use]
    pub const fn target(&self) -> &CompiledTarget {
        &self.target
    }

    #[must_use]
    pub const fn format(&self) -> DiagnosticFormat {
        self.presentation.format()
    }

    #[must_use]
    pub const fn presentation(&self) -> &CheckCommandPresentation {
        &self.presentation
    }

    #[must_use]
    pub fn into_target(self) -> CompiledTarget {
        self.target
    }
}

/// Stable presentation facts retained independently from successful or failed checking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckCommandPresentation {
    format: DiagnosticFormat,
    target: CompilationTarget,
    root: PathBuf,
}

impl CheckCommandPresentation {
    fn new(format: DiagnosticFormat, target: CompilationTarget, root: PathBuf) -> Self {
        Self {
            format,
            target,
            root,
        }
    }

    #[must_use]
    pub const fn format(&self) -> DiagnosticFormat {
        self.format
    }

    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Resolves package/source input and checks it through the complete target-program boundary.
///
/// # Errors
///
/// Returns the exact source or compilation-session boundary that failed.
pub fn execute_prepared_check<A: PackageAcquisitionAuthority>(
    command: PreparedCheckCommand,
    toolchain: &CommandToolchain,
    authority: &mut A,
) -> Result<CheckCommandResult, CheckCommandExecutionError> {
    let (plan, resolution, format) = command.into_parts();
    let (input, executable) = plan.into_parts();
    let root = match &input {
        crate::ResolvedProgramInput::Package(package) => package.declaration(),
        crate::ResolvedProgramInput::SingleFile(source) => source.source(),
    };
    let presentation = CheckCommandPresentation::new(format, toolchain.target(), root.into());
    let roots = executable.as_deref().map_or(
        CommandCompileRoots::AllExecutables,
        CommandCompileRoots::NamedExecutable,
    );
    let unit = discover_command_source(&input, resolution, toolchain, roots, authority).map_err(
        |error| CheckCommandExecutionError::Source {
            presentation: presentation.clone(),
            error: Box::new(error),
        },
    )?;
    match compile_target(&unit) {
        Ok(target) => Ok(CheckCommandResult {
            target,
            presentation,
        }),
        Err(error) => Err(CheckCommandExecutionError::Check {
            presentation,
            failure: Box::new(command_compilation_failure(error, unit)),
        }),
    }
}

#[derive(Debug)]
pub enum CheckCommandExecutionError {
    Source {
        presentation: CheckCommandPresentation,
        error: Box<CommandSourceError>,
    },
    Check {
        presentation: CheckCommandPresentation,
        failure: Box<CommandCompilationFailure<CompileSessionError>>,
    },
}

impl CheckCommandExecutionError {
    #[must_use]
    pub const fn format(&self) -> DiagnosticFormat {
        self.presentation().format()
    }

    #[must_use]
    pub const fn presentation(&self) -> &CheckCommandPresentation {
        match self {
            Self::Source { presentation, .. } | Self::Check { presentation, .. } => presentation,
        }
    }

    #[must_use]
    pub fn source_diagnostics(
        &self,
    ) -> Option<(
        &[nocter_diagnostics::SourceDiagnostic],
        &nocter_source::SourceMap,
    )> {
        match self {
            Self::Source { error, .. } => error.source_diagnostics(),
            Self::Check { failure, .. } => Some((failure.diagnostics(), failure.sources())),
        }
    }

    /// Returns a spanless code when checking failed outside authored source diagnostics.
    #[must_use]
    pub fn diagnostic_code(&self) -> Option<&'static str> {
        match self {
            Self::Source { error, .. } => error.diagnostic_code(),
            Self::Check { failure, .. } if failure.diagnostics().is_empty() => {
                failure.error().diagnostic_code()
            }
            Self::Check { .. } => None,
        }
    }

    #[must_use]
    pub fn is_user_failure(&self) -> bool {
        match self {
            Self::Source { error, .. } => error.is_user_failure(),
            Self::Check { failure, .. } => failure.error().diagnostic_code().is_some(),
        }
    }
}

impl fmt::Display for CheckCommandExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source { error, .. } => write!(formatter, "check input failed: {error}"),
            Self::Check { failure, .. } => failure.fmt(formatter),
        }
    }
}

impl std::error::Error for CheckCommandExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source { error, .. } => Some(error),
            Self::Check { failure, .. } => Some(failure),
        }
    }
}
