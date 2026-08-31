use std::fmt;
use std::sync::Arc;

use nocter_diagnostics::DiagnosticCode;
use nocter_model::PackageTargetKind;
use nocter_model::{PackageIdentity, PackageTargetId};
use nocter_target_program::{ExecutableProgram, ExecutableProgramError, TargetProgram};

use crate::{CompiledExecutable, CompiledTarget};

/// Resolver-stable identity and authored name of one selected executable target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableIdentity {
    package: PackageIdentity,
    target: PackageTargetId,
    name: Box<str>,
}

impl ExecutableIdentity {
    #[must_use]
    pub const fn package(&self) -> &PackageIdentity {
        &self.package
    }

    #[must_use]
    pub const fn target(&self) -> PackageTargetId {
        self.target
    }

    #[must_use]
    pub const fn name(&self) -> &str {
        &self.name
    }
}

/// User-visible selection accepted by executable-producing commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutableSelector {
    Only,
    Named(Box<str>),
}

impl ExecutableSelector {
    #[must_use]
    pub fn named(name: impl Into<Box<str>>) -> Self {
        Self::Named(name.into())
    }
}

/// One closed request to compile and specialize a process executable.
#[derive(Debug)]
pub struct ExecutableCompileRequest {
    target: CompiledTarget,
    selector: ExecutableSelector,
}

impl ExecutableCompileRequest {
    #[must_use]
    pub const fn new(target: CompiledTarget, selector: ExecutableSelector) -> Self {
        Self { target, selector }
    }

    #[must_use]
    pub const fn only(target: CompiledTarget) -> Self {
        Self::new(target, ExecutableSelector::Only)
    }

    #[must_use]
    pub fn named(target: CompiledTarget, name: impl Into<Box<str>>) -> Self {
        Self::new(target, ExecutableSelector::named(name))
    }
}

/// Selects and closes one executable root from a compiled target.
///
/// # Errors
///
/// Returns the exact command-selection or executable-closure failure. A command layer never
/// receives a partially selected target program.
pub fn compile_executable(
    request: ExecutableCompileRequest,
) -> Result<CompiledExecutable, ExecutableSessionError> {
    let ExecutableCompileRequest { target, selector } = request;
    let (target, source_index) = target.into_parts();
    let selected = select_executable(&target, &selector)?;
    let program = ExecutableProgram::for_executable(target, selected.target())?;
    Ok(CompiledExecutable::new(selected, program, source_index))
}

fn select_executable(
    program: &TargetProgram,
    selector: &ExecutableSelector,
) -> Result<ExecutableIdentity, ExecutableSelectionError> {
    let candidates = root_executables(program);
    match selector {
        ExecutableSelector::Only => match candidates.as_slice() {
            [] => Err(ExecutableSelectionError::NoExecutable),
            [selected] => Ok(selected.clone()),
            _ => Err(ExecutableSelectionError::MultipleExecutables),
        },
        ExecutableSelector::Named(name) => {
            let matches = candidates
                .into_iter()
                .filter(|target| target.name() == name.as_ref())
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [selected] => Ok(selected.clone()),
                [] => Err(ExecutableSelectionError::UnknownName(name.clone())),
                _ => Err(ExecutableSelectionError::AmbiguousName(name.clone())),
            }
        }
    }
}

/// Returns executable identities owned by root packages in declaration order.
///
/// # Panics
///
/// Panics only if `program` violates its validated package-target integrity guarantees.
#[must_use]
fn root_executables(program: &TargetProgram) -> Vec<ExecutableIdentity> {
    let graph = program.checked().graph();
    let root_packages = graph.root_packages();
    graph
        .package_targets()
        .iter()
        .filter(|(_, target)| {
            target.kind() == PackageTargetKind::Executable
                && root_packages.contains(&target.package())
        })
        .map(|(id, target)| {
            let package = graph
                .packages()
                .get(target.package())
                .expect("validated package target retains its package");
            let name = graph
                .symbols()
                .spelling(target.name())
                .expect("validated package target retains its name");
            ExecutableIdentity {
                package: package.identity().clone(),
                target: id,
                name: name.into(),
            }
        })
        .collect()
}

/// Closes every root executable while retaining the target program that declared each identity.
///
/// # Errors
///
/// Returns an executable-closure error if any selected root cannot produce a closed program.
pub fn close_root_executables(
    target: TargetProgram,
) -> Result<Box<[RootExecutableProgram]>, RootExecutableClosureError> {
    let identities = root_executables(&target);
    let target = Arc::new(target);
    identities
        .into_iter()
        .map(|identity| {
            let executable =
                ExecutableProgram::for_executable(Arc::clone(&target), identity.target()).map_err(
                    |source| RootExecutableClosureError {
                        executable: identity.clone(),
                        source,
                    },
                )?;
            Ok(RootExecutableProgram {
                identity,
                executable,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

/// One executable identity inseparably closed over the target program that declared it.
#[derive(Debug)]
pub struct RootExecutableProgram {
    identity: ExecutableIdentity,
    executable: ExecutableProgram,
}

impl RootExecutableProgram {
    #[must_use]
    pub fn into_parts(self) -> (ExecutableIdentity, ExecutableProgram) {
        (self.identity, self.executable)
    }
}

#[derive(Debug)]
pub struct RootExecutableClosureError {
    executable: ExecutableIdentity,
    source: ExecutableProgramError,
}

impl RootExecutableClosureError {
    #[must_use]
    pub fn into_parts(self) -> (ExecutableIdentity, ExecutableProgramError) {
        (self.executable, self.source)
    }
}

impl fmt::Display for RootExecutableClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "executable {} closure failed: {}",
            self.executable.name(),
            self.source,
        )
    }
}

impl std::error::Error for RootExecutableClosureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutableSelectionError {
    NoExecutable,
    MultipleExecutables,
    UnknownName(Box<str>),
    AmbiguousName(Box<str>),
}

impl fmt::Display for ExecutableSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoExecutable => formatter.write_str("compile unit declares no executable"),
            Self::MultipleExecutables => formatter
                .write_str("compile unit declares multiple executables; select one by name"),
            Self::UnknownName(name) => {
                write!(formatter, "compile unit has no executable named {name}")
            }
            Self::AmbiguousName(name) => {
                write!(
                    formatter,
                    "executable name {name} is ambiguous across compile roots"
                )
            }
        }
    }
}

impl std::error::Error for ExecutableSelectionError {}

#[derive(Debug)]
pub enum ExecutableSessionError {
    Selection(ExecutableSelectionError),
    Executable(ExecutableProgramError),
}

impl ExecutableSessionError {
    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<DiagnosticCode> {
        match self {
            Self::Selection(_) => Some(DiagnosticCode::E0800),
            Self::Executable(_) => None,
        }
    }
}

impl fmt::Display for ExecutableSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Selection(error) => write!(formatter, "executable selection failed: {error}"),
            Self::Executable(error) => write!(formatter, "executable closure failed: {error}"),
        }
    }
}

impl std::error::Error for ExecutableSessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Selection(error) => Some(error),
            Self::Executable(error) => Some(error),
        }
    }
}

impl From<ExecutableSelectionError> for ExecutableSessionError {
    fn from(error: ExecutableSelectionError) -> Self {
        Self::Selection(error)
    }
}

impl From<ExecutableProgramError> for ExecutableSessionError {
    fn from(error: ExecutableProgramError) -> Self {
        Self::Executable(error)
    }
}
