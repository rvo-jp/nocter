use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use nocter_compile_input::ModuleIdentity;
use nocter_discovery::{DiscoveredUnit, DiscoveryError, DiscoveryRequest, discover};
use nocter_model::{CompilationTarget, PackageIdentity};
use nocter_package::{
    PackageGraphError, PackageResolutionError, PackageResolutionPolicy, PackageResolutionRequest,
    ResolvedPackageSelection, StandardPackage, resolve_standard_package,
};
use nocter_package_state::{PackageAcquisitionAuthority, PackageStateError, resolve_package_state};
use nocter_session::{ExecutableSelector, bundled_standard_toolchain};

use crate::{ResolutionOptions, ResolvedProgramInput};

#[derive(Clone, Copy)]
pub(crate) enum CommandCompileRoots<'a> {
    AllExecutables,
    Selected(&'a ExecutableSelector),
}

/// Explicit installation and target facts used by command source preparation.
///
/// Process environment and executable-path discovery remain outside this value. A future public
/// binary must resolve and validate those effects once, then construct this immutable input.
#[derive(Clone, Debug)]
pub struct CommandToolchain {
    target: CompilationTarget,
    nocter_home: PathBuf,
    standard: StandardPackage,
}

impl CommandToolchain {
    #[must_use]
    pub fn new(
        target: CompilationTarget,
        nocter_home: impl Into<PathBuf>,
        standard: StandardPackage,
    ) -> Self {
        Self {
            target,
            nocter_home: nocter_home.into(),
            standard,
        }
    }

    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    #[must_use]
    pub fn nocter_home(&self) -> &Path {
        &self.nocter_home
    }

    #[must_use]
    pub const fn standard(&self) -> &StandardPackage {
        &self.standard
    }
}

pub(crate) fn discover_command_source<A: PackageAcquisitionAuthority>(
    input: &ResolvedProgramInput,
    resolution: ResolutionOptions,
    toolchain: &CommandToolchain,
    compile_roots: CommandCompileRoots<'_>,
    authority: &mut A,
) -> Result<DiscoveredUnit, CommandSourceError> {
    match input {
        ResolvedProgramInput::Package(package) => {
            let selected = resolve_package_state(
                PackageResolutionRequest::new(
                    package.root(),
                    toolchain.nocter_home(),
                    toolchain.standard().clone(),
                    PackageResolutionPolicy::new(resolution.locked(), resolution.offline()),
                ),
                authority,
            )
            .map_err(command_package_state_error)?;
            discover_declared(selected, toolchain, compile_roots)
        }
        ResolvedProgramInput::SingleFile(source) => {
            let standard = toolchain.standard().identity().clone();
            let support_packages = resolve_standard_package(toolchain.standard().clone())
                .map_err(CommandSourceError::StandardPackage)?;
            discover(DiscoveryRequest::single_file(
                toolchain.target(),
                source.source(),
                support_packages,
                bundled_standard_toolchain(&standard),
            ))
            .map_err(CommandSourceError::Discovery)
        }
    }
}

fn command_package_state_error<E>(error: PackageStateError<E>) -> CommandSourceError
where
    E: std::error::Error + Send + Sync + 'static,
{
    match error {
        PackageStateError::Resolution(error) => CommandSourceError::PackageResolution(*error),
        error => CommandSourceError::PackageState(Box::new(error)),
    }
}

fn discover_declared(
    selected: ResolvedPackageSelection,
    toolchain: &CommandToolchain,
    compile_roots: CommandCompileRoots<'_>,
) -> Result<DiscoveredUnit, CommandSourceError> {
    let root = selected.root().clone();
    let standard = selected.standard().clone();
    let mut roots = BTreeSet::new();
    roots.insert(ModuleIdentity::new(root.clone(), Vec::<Box<str>>::new()));
    let package = selected
        .graph()
        .packages()
        .iter()
        .find(|package| package.identity() == &root)
        .ok_or_else(|| CommandSourceError::MissingCommandRoot(root.clone()))?;
    if let Some(declaration) = package.declaration() {
        for target in declaration.targets().iter().filter(|target| {
            target.kind() == nocter_model::PackageTargetKind::Executable
                && match compile_roots {
                    CommandCompileRoots::AllExecutables
                    | CommandCompileRoots::Selected(ExecutableSelector::Only) => true,
                    CommandCompileRoots::Selected(ExecutableSelector::Named(name)) => {
                        target.name().value() == name.as_ref()
                    }
                }
        }) {
            roots.insert(ModuleIdentity::new(
                root.clone(),
                target.module().iter().cloned(),
            ));
        }
    }
    let (packages, _, _) = selected.into_parts();
    discover(DiscoveryRequest::declared(
        toolchain.target(),
        packages,
        roots.into_iter().collect(),
        bundled_standard_toolchain(&standard),
    ))
    .map_err(CommandSourceError::Discovery)
}

#[derive(Debug)]
pub enum CommandSourceError {
    PackageResolution(PackageResolutionError),
    PackageState(Box<dyn std::error::Error + Send + Sync>),
    StandardPackage(PackageGraphError),
    MissingCommandRoot(PackageIdentity),
    Discovery(DiscoveryError),
}

impl fmt::Display for CommandSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PackageResolution(error) => error.fmt(formatter),
            Self::PackageState(error) => error.fmt(formatter),
            Self::StandardPackage(error) => {
                write!(formatter, "standard package is invalid: {error}")
            }
            Self::MissingCommandRoot(package) => write!(
                formatter,
                "resolved graph does not contain command-root package {}",
                package.as_str()
            ),
            Self::Discovery(error) => write!(formatter, "source discovery failed: {error}"),
        }
    }
}

impl std::error::Error for CommandSourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PackageResolution(error) => Some(error),
            Self::PackageState(error) => Some(&**error),
            Self::StandardPackage(error) => Some(error),
            Self::Discovery(error) => Some(error),
            Self::MissingCommandRoot(_) => None,
        }
    }
}
