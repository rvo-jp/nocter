use std::fmt;

use nocter_declarations::PackageTargetKind;
use nocter_discovery::DiscoveredUnit;
use nocter_model::PackageTargetId;
use nocter_target_program::{ExecutableProgram, ExecutableProgramError, TargetProgram};

use crate::{CompileSessionError, CompiledExecutable, compile_target};

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
    let program = ExecutableProgram::for_executable(target, selected)?;
    Ok(CompiledExecutable::new(program, source_index))
}

fn select_executable(
    program: &TargetProgram,
    selector: &ExecutableSelector,
) -> Result<PackageTargetId, ExecutableSelectionError> {
    let graph = program.checked().graph();
    let mut candidates = graph
        .package_targets()
        .iter()
        .filter(|(_, target)| target.kind() == PackageTargetKind::Executable);
    match selector {
        ExecutableSelector::Only => {
            let Some((selected, _)) = candidates.next() else {
                return Err(ExecutableSelectionError::NoExecutable);
            };
            if candidates.next().is_some() {
                return Err(ExecutableSelectionError::MultipleExecutables);
            }
            Ok(selected)
        }
        ExecutableSelector::Named(name) => {
            let matches = candidates
                .filter(|(_, target)| graph.symbols().spelling(target.name()) == Some(name))
                .map(|(id, _)| id)
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [selected] => Ok(*selected),
                [] => Err(ExecutableSelectionError::UnknownName(name.clone())),
                _ => Err(ExecutableSelectionError::AmbiguousName(name.clone())),
            }
        }
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
    Compile(CompileSessionError),
    Selection(ExecutableSelectionError),
    Executable(ExecutableProgramError),
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
