use std::fmt;

use nocter_diagnostics::DiagnosticCode;
use nocter_model::PackageIdentity;
use nocter_package_state::PackageAcquisitionAuthority;

use crate::compiler::CommandCompiler;
use crate::package_state::resolve_command_package_state;
use crate::{CommandPackageContext, CommandPackageStateError, PreparedFetchCommand};

/// Completed package-state result of one fetch command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchCommandResult {
    root: PackageIdentity,
}

impl FetchCommandResult {
    #[must_use]
    pub const fn root(&self) -> &PackageIdentity {
        &self.root
    }
}

/// Resolves, acquires, validates, publishes, and locks one prepared package graph.
///
/// This command deliberately stops at the package-state boundary. It does not discover or compile
/// source, and it cannot use a transaction path different from build or run.
///
/// # Errors
///
/// Returns the exact shared package-state failure.
pub fn execute_prepared_fetch<A: PackageAcquisitionAuthority>(
    command: PreparedFetchCommand,
    context: &CommandPackageContext,
    authority: &mut A,
) -> Result<FetchCommandResult, FetchCommandExecutionError> {
    let (input, resolution) = command.into_parts();
    let mut compiler = CommandCompiler::default();
    let selected =
        resolve_command_package_state(&input, resolution, context, authority, &mut compiler)
            .map_err(FetchCommandExecutionError::Package)?;
    Ok(FetchCommandResult {
        root: selected.root().clone(),
    })
}

#[derive(Debug)]
pub enum FetchCommandExecutionError {
    Package(CommandPackageStateError),
}

impl FetchCommandExecutionError {
    #[must_use]
    pub const fn diagnostic_code(&self) -> DiagnosticCode {
        match self {
            Self::Package(error) => error.diagnostic_code(),
        }
    }
}

impl fmt::Display for FetchCommandExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Package(error) => write!(formatter, "fetch failed: {error}"),
        }
    }
}

impl std::error::Error for FetchCommandExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Package(error) => Some(error),
        }
    }
}
