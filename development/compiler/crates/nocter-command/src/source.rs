use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use nocter_compile_input::ModuleIdentity;
use nocter_discovery::{
    DiscoveredUnit, DiscoveryError, DiscoveryFailure, DiscoveryRequest, discover,
};
use nocter_model::{CompilationTarget, PackageIdentity};
use nocter_package::{
    PackageGraphError, ResolvedPackageSelection, StandardPackage, resolve_standard_package,
};
use nocter_package_state::PackageAcquisitionAuthority;
use nocter_session::{ExecutableSelector, TestTargetSelector, bundled_standard_toolchain};

use crate::package_state::resolve_command_package_state;
use crate::{
    CommandPackageContext, CommandPackageStateError, ResolutionOptions, ResolvedProgramInput,
};

#[derive(Clone, Copy)]
pub(crate) enum CommandCompileRoots<'a> {
    AllExecutables,
    NamedExecutable(&'a str),
    NamedTest(&'a str),
}

impl<'a> CommandCompileRoots<'a> {
    pub(crate) fn for_selector(selector: &'a ExecutableSelector) -> Self {
        match selector {
            ExecutableSelector::Only => Self::AllExecutables,
            ExecutableSelector::Named(name) => Self::NamedExecutable(name),
        }
    }
}

/// Explicit installation and target facts used by command source preparation.
///
/// Process environment and executable-path discovery remain outside this value. A future public
/// binary must resolve and validate those effects once, then construct this immutable input.
#[derive(Clone, Debug)]
pub struct CommandToolchain {
    target: CompilationTarget,
    packages: CommandPackageContext,
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
            packages: CommandPackageContext::new(nocter_home, standard),
        }
    }

    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    #[must_use]
    pub fn nocter_home(&self) -> &Path {
        self.packages.nocter_home()
    }

    #[must_use]
    pub const fn standard(&self) -> &StandardPackage {
        self.packages.standard()
    }

    #[must_use]
    pub const fn packages(&self) -> &CommandPackageContext {
        &self.packages
    }

    #[must_use]
    pub fn for_requested_target(&self, target: Option<CompilationTarget>) -> Self {
        Self {
            target: target.unwrap_or(self.target),
            packages: self.packages.clone(),
        }
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
            let selected =
                resolve_command_package_state(package, resolution, toolchain.packages(), authority)
                    .map_err(CommandSourceError::Package)?;
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

pub(crate) struct CommandTestSource {
    package: PackageIdentity,
    target: Box<str>,
    discovery: Result<DiscoveredUnit, DiscoveryFailure>,
}

impl CommandTestSource {
    pub(crate) fn into_parts(
        self,
    ) -> (
        PackageIdentity,
        Box<str>,
        Result<DiscoveredUnit, DiscoveryFailure>,
    ) {
        (self.package, self.target, self.discovery)
    }
}

/// Resolves package state once, then forks its immutable manifest snapshot into one discovery
/// request per selected test target.
///
/// A target-local syntax or discovery failure remains beside that target, allowing later targets
/// to enter their own compiler sessions. Package resolution and target-name selection failures
/// remain command-wide.
pub(crate) fn discover_command_tests<A: PackageAcquisitionAuthority>(
    input: &crate::PackageCommandInput,
    resolution: ResolutionOptions,
    toolchain: &CommandToolchain,
    selector: &TestTargetSelector,
    authority: &mut A,
) -> Result<Vec<CommandTestSource>, CommandSourceError> {
    let selected =
        resolve_command_package_state(input, resolution, toolchain.packages(), authority)
            .map_err(CommandSourceError::Package)?;
    let root = selected.root().clone();
    let package = selected
        .graph()
        .packages()
        .iter()
        .find(|package| package.identity() == &root)
        .ok_or_else(|| CommandSourceError::MissingCommandRoot(root.clone()))?;
    let tests = package
        .declaration()
        .into_iter()
        .flat_map(nocter_package::PackageDeclaration::targets)
        .filter(|target| target.kind() == nocter_model::PackageTargetKind::Test)
        .filter(|target| match selector {
            TestTargetSelector::All => true,
            TestTargetSelector::Named(name) => target.name().value() == name.as_ref(),
        })
        .map(|target| Box::<str>::from(target.name().value()))
        .collect::<Vec<_>>();
    if tests.is_empty() {
        return Err(match selector {
            TestTargetSelector::All => CommandSourceError::MissingCommandTests(root),
            TestTargetSelector::Named(name) => CommandSourceError::MissingCommandTest {
                package: root,
                name: name.clone(),
            },
        });
    }
    let mut sources = Vec::with_capacity(tests.len());
    for target in tests {
        let discovery = match discover_declared(
            selected.clone(),
            toolchain,
            CommandCompileRoots::NamedTest(&target),
        ) {
            Ok(unit) => Ok(unit),
            Err(CommandSourceError::Discovery(failure)) => Err(failure),
            Err(error) => return Err(error),
        };
        sources.push(CommandTestSource {
            package: selected.root().clone(),
            target,
            discovery,
        });
    }
    Ok(sources)
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
    let mut selected_named_target = false;
    if let Some(declaration) = package.declaration() {
        for target in declaration
            .targets()
            .iter()
            .filter(|target| match compile_roots {
                CommandCompileRoots::AllExecutables => {
                    target.kind() == nocter_model::PackageTargetKind::Executable
                }
                CommandCompileRoots::NamedExecutable(name) => {
                    target.kind() == nocter_model::PackageTargetKind::Executable
                        && target.name().value() == name
                }
                CommandCompileRoots::NamedTest(name) => {
                    target.kind() == nocter_model::PackageTargetKind::Test
                        && target.name().value() == name
                }
            })
        {
            selected_named_target = true;
            roots.insert(ModuleIdentity::new(
                root.clone(),
                target.module().iter().cloned(),
            ));
        }
    }
    if !selected_named_target {
        match compile_roots {
            CommandCompileRoots::NamedExecutable(name) => {
                return Err(CommandSourceError::MissingCommandExecutable {
                    package: root,
                    name: name.into(),
                });
            }
            CommandCompileRoots::NamedTest(name) => {
                return Err(CommandSourceError::MissingCommandTest {
                    package: root,
                    name: name.into(),
                });
            }
            CommandCompileRoots::AllExecutables => {}
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
    Package(CommandPackageStateError),
    StandardPackage(PackageGraphError),
    MissingCommandRoot(PackageIdentity),
    MissingCommandExecutable {
        package: PackageIdentity,
        name: Box<str>,
    },
    MissingCommandTest {
        package: PackageIdentity,
        name: Box<str>,
    },
    MissingCommandTests(PackageIdentity),
    Discovery(DiscoveryFailure),
}

impl fmt::Display for CommandSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Package(error) => error.fmt(formatter),
            Self::StandardPackage(error) => {
                write!(formatter, "standard package is invalid: {error}")
            }
            Self::MissingCommandRoot(package) => write!(
                formatter,
                "resolved graph does not contain command-root package {}",
                package.as_str()
            ),
            Self::MissingCommandExecutable { package, name } => write!(
                formatter,
                "command-root package {} does not declare executable {name}",
                package.as_str()
            ),
            Self::MissingCommandTest { package, name } => write!(
                formatter,
                "command-root package {} does not declare test target {name}",
                package.as_str()
            ),
            Self::MissingCommandTests(package) => write!(
                formatter,
                "command-root package {} does not declare a test target",
                package.as_str()
            ),
            Self::Discovery(error) => write!(formatter, "source discovery failed: {error}"),
        }
    }
}

impl std::error::Error for CommandSourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Package(error) => Some(error),
            Self::StandardPackage(error) => Some(error),
            Self::Discovery(error) => Some(error),
            Self::MissingCommandRoot(_)
            | Self::MissingCommandExecutable { .. }
            | Self::MissingCommandTest { .. }
            | Self::MissingCommandTests(_) => None,
        }
    }
}

impl CommandSourceError {
    #[must_use]
    pub fn source_diagnostics(
        &self,
    ) -> Option<(
        &[nocter_diagnostics::SourceDiagnostic],
        &nocter_source::SourceMap,
    )> {
        match self {
            Self::Discovery(failure) if !failure.diagnostics().is_empty() => {
                Some((failure.diagnostics(), failure.sources()))
            }
            Self::Package(_)
            | Self::StandardPackage(_)
            | Self::MissingCommandRoot(_)
            | Self::MissingCommandExecutable { .. }
            | Self::MissingCommandTest { .. }
            | Self::MissingCommandTests(_)
            | Self::Discovery(_) => None,
        }
    }

    /// Returns a spanless code only for source-preparation failures whose public family is fixed.
    /// Authored import failures and internal graph inconsistencies remain unclassified until their
    /// source-backed diagnostic boundary selects an exact rule.
    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<&'static str> {
        match self {
            Self::Package(error) => Some(error.diagnostic_code()),
            Self::StandardPackage(_) => Some("E0703"),
            Self::Discovery(failure) if matches!(failure.error(), DiscoveryError::Toolchain(_)) => {
                Some("E0703")
            }
            Self::MissingCommandExecutable { .. }
            | Self::MissingCommandTest { .. }
            | Self::MissingCommandTests(_) => Some("E0800"),
            Self::Discovery(failure)
                if matches!(failure.error(), DiscoveryError::TargetSelection(_)) =>
            {
                Some("E0701")
            }
            Self::Discovery(failure)
                if matches!(
                    failure.error(),
                    DiscoveryError::InvalidPackageRoot { .. }
                        | DiscoveryError::InvalidSingleFileExtension(_)
                        | DiscoveryError::MissingModuleRoot { .. }
                        | DiscoveryError::InvalidModulePath { .. }
                        | DiscoveryError::NonUnicodeCanonicalPath(_)
                        | DiscoveryError::Filesystem { .. }
                        | DiscoveryError::Source { .. }
                ) =>
            {
                Some("E0702")
            }
            Self::MissingCommandRoot(_) | Self::Discovery(_) => None,
        }
    }

    /// Distinguishes authored/environment failures from compiler consistency failures.
    #[must_use]
    pub const fn is_user_failure(&self) -> bool {
        match self {
            Self::Package(_)
            | Self::StandardPackage(_)
            | Self::MissingCommandExecutable { .. }
            | Self::MissingCommandTest { .. }
            | Self::MissingCommandTests(_) => true,
            Self::Discovery(failure) => !matches!(
                failure.error(),
                DiscoveryError::DuplicatePackage(_)
                    | DiscoveryError::UnknownPackage(_)
                    | DiscoveryError::ConflictingSourceOwner { .. }
                    | DiscoveryError::InconsistentSyntax(_)
            ),
            Self::MissingCommandRoot(_) => false,
        }
    }
}
