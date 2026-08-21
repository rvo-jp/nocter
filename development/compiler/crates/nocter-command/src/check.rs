use std::fmt;

use nocter_package_state::PackageAcquisitionAuthority;
use nocter_session::{CompileSessionError, CompiledTarget, compile_target};

use crate::failure::command_compilation_failure;
use crate::source::{CommandCompileRoots, discover_command_source};
use crate::{
    CommandCompilationFailure, CommandSourceError, CommandToolchain, PreparedCheckCommand,
};

/// Target-validated result of one check command.
#[derive(Debug)]
pub struct CheckCommandResult {
    target: CompiledTarget,
}

impl CheckCommandResult {
    #[must_use]
    pub const fn target(&self) -> &CompiledTarget {
        &self.target
    }

    #[must_use]
    pub fn into_target(self) -> CompiledTarget {
        self.target
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
    let (plan, resolution) = command.into_parts();
    let (input, executable) = plan.into_parts();
    let roots = executable.as_deref().map_or(
        CommandCompileRoots::AllExecutables,
        CommandCompileRoots::NamedExecutable,
    );
    let unit = discover_command_source(&input, resolution, toolchain, roots, authority)
        .map_err(CheckCommandExecutionError::Source)?;
    match compile_target(&unit) {
        Ok(target) => Ok(CheckCommandResult { target }),
        Err(error) => Err(CheckCommandExecutionError::Check(Box::new(
            command_compilation_failure(error, unit),
        ))),
    }
}

#[derive(Debug)]
pub enum CheckCommandExecutionError {
    Source(CommandSourceError),
    Check(Box<CommandCompilationFailure<CompileSessionError>>),
}

impl CheckCommandExecutionError {
    #[must_use]
    pub fn source_diagnostics(
        &self,
    ) -> Option<(
        &[nocter_diagnostics::SourceDiagnostic],
        &nocter_source::SourceMap,
    )> {
        match self {
            Self::Source(_) => None,
            Self::Check(failure) => Some((failure.diagnostics(), failure.sources())),
        }
    }
}

impl fmt::Display for CheckCommandExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "check input failed: {error}"),
            Self::Check(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CheckCommandExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Check(error) => Some(error),
        }
    }
}
