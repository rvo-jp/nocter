use std::fmt;
use std::sync::Arc;

use nocter_declarations::PackageTargetKind;
use nocter_diagnostics::SourceDiagnostic;
use nocter_discovery::DiscoveredUnit;
use nocter_model::{PackageIdentity, PackageTargetId};
use nocter_target_program::{ExecutableProgram, ExecutableProgramError, TargetProgram};

use crate::{CompileSessionError, CompiledExecutable, compile_target};

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
pub struct ExecutableCompileRequest<'unit> {
    unit: &'unit DiscoveredUnit,
    selector: ExecutableSelector,
}

impl<'unit> ExecutableCompileRequest<'unit> {
    #[must_use]
    pub const fn new(unit: &'unit DiscoveredUnit, selector: ExecutableSelector) -> Self {
        Self { unit, selector }
    }

    #[must_use]
    pub const fn only(unit: &'unit DiscoveredUnit) -> Self {
        Self::new(unit, ExecutableSelector::Only)
    }

    #[must_use]
    pub fn named(unit: &'unit DiscoveredUnit, name: impl Into<Box<str>>) -> Self {
        Self::new(unit, ExecutableSelector::named(name))
    }
}

/// Compiles one discovery snapshot and closes the requested executable root.
///
/// # Errors
///
/// Returns the exact compilation, command-selection, or executable-closure failure. A command
/// layer never receives a partially selected target program.
pub fn compile_executable(
    request: ExecutableCompileRequest<'_>,
) -> Result<CompiledExecutable, ExecutableSessionError> {
    let ExecutableCompileRequest { unit, selector } = request;
    let compiled = compile_target(unit)?;
    let (target, source_index) = compiled.into_parts();
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

pub(crate) fn root_executables(program: &TargetProgram) -> Vec<ExecutableIdentity> {
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

pub(crate) fn close_executable(
    target: Arc<TargetProgram>,
    selected: &ExecutableIdentity,
) -> Result<ExecutableProgram, ExecutableProgramError> {
    ExecutableProgram::for_executable(target, selected.target())
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
    Compile(CompileSessionError),
    Selection(ExecutableSelectionError),
    Executable(ExecutableProgramError),
}

impl ExecutableSessionError {
    #[must_use]
    pub fn source_diagnostic(&self) -> Option<&SourceDiagnostic> {
        match self {
            Self::Compile(error) => error.source_diagnostic(),
            Self::Selection(_) | Self::Executable(_) => None,
        }
    }
}

impl fmt::Display for ExecutableSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile(error) => write!(formatter, "target compilation failed: {error}"),
            Self::Selection(error) => write!(formatter, "executable selection failed: {error}"),
            Self::Executable(error) => write!(formatter, "executable closure failed: {error}"),
        }
    }
}

impl std::error::Error for ExecutableSessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Compile(error) => Some(error),
            Self::Selection(error) => Some(error),
            Self::Executable(error) => Some(error),
        }
    }
}

impl From<CompileSessionError> for ExecutableSessionError {
    fn from(error: CompileSessionError) -> Self {
        Self::Compile(error)
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
