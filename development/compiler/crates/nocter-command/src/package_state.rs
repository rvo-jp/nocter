use std::fmt;
use std::path::{Path, PathBuf};

use nocter_package::{
    PackageResolutionError, PackageResolutionPolicy, PackageResolutionRequest,
    ResolvedPackageSelection,
};
use nocter_package_state::{
    PackageAcquisitionAuthority, PackageStateError, resolve_package_state_with_driver,
};

use crate::compiler::CommandCompiler;
use crate::{PackageCommandInput, ResolutionOptions};

/// Installation-owned package facts shared by every public package command.
#[derive(Clone, Debug)]
pub struct CommandPackageContext {
    nocter_home: PathBuf,
    standard: nocter_package::StandardPackage,
}

impl CommandPackageContext {
    #[must_use]
    pub fn new(nocter_home: impl Into<PathBuf>, standard: nocter_package::StandardPackage) -> Self {
        Self {
            nocter_home: nocter_home.into(),
            standard,
        }
    }

    #[must_use]
    pub fn nocter_home(&self) -> &Path {
        &self.nocter_home
    }

    #[must_use]
    pub const fn standard(&self) -> &nocter_package::StandardPackage {
        &self.standard
    }
}

/// Runs the sole package-state transaction used by public package commands.
pub(crate) fn resolve_command_package_state<A: PackageAcquisitionAuthority>(
    input: &PackageCommandInput,
    resolution: ResolutionOptions,
    context: &CommandPackageContext,
    authority: &mut A,
    compiler: &mut CommandCompiler,
) -> Result<ResolvedPackageSelection, CommandPackageStateError> {
    resolve_package_state_with_driver(
        PackageResolutionRequest::new(
            input.root(),
            context.nocter_home(),
            context.standard().clone(),
            PackageResolutionPolicy::new(resolution.locked(), resolution.offline()),
        ),
        authority,
        compiler,
    )
    .map_err(command_package_state_error)
}

fn command_package_state_error<E>(error: PackageStateError<E>) -> CommandPackageStateError
where
    E: std::error::Error + Send + Sync + 'static,
{
    match error {
        PackageStateError::Resolution(error) => CommandPackageStateError::Resolution(*error),
        PackageStateError::ResolutionInfrastructure(error) => {
            CommandPackageStateError::Transaction(error)
        }
        error => CommandPackageStateError::Transaction(Box::new(error)),
    }
}

/// Package resolution and mutable-state failures shared by every public package command.
#[derive(Debug)]
pub enum CommandPackageStateError {
    Resolution(PackageResolutionError),
    Transaction(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for CommandPackageStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resolution(error) => error.fmt(formatter),
            Self::Transaction(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CommandPackageStateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resolution(error) => Some(error),
            Self::Transaction(error) => Some(&**error),
        }
    }
}

impl CommandPackageStateError {
    /// Returns the public spanless diagnostic family owned by package-state orchestration.
    #[must_use]
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::Resolution(PackageResolutionError::Filesystem { .. }) => "E0702",
            Self::Resolution(_) | Self::Transaction(_) => "E0800",
        }
    }
}
