use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use nocter_package::{PackageGraphError, PackageResolutionFailure};

pub(crate) fn preparation_diagnostics(
    error: &WorkspaceAnalysisError,
) -> Result<Box<[nocter_diagnostics::SourceDiagnostic]>, WorkspaceDiagnosticError> {
    let Some(failure) = error.package_failure() else {
        return Ok(Box::new([]));
    };
    let nocter_package::PackageResolutionError::Graph(PackageGraphError::Declaration(error)) =
        failure.error()
    else {
        return Ok(Box::new([]));
    };
    let subject = error.subject();
    let source = failure
        .reached()
        .sources()
        .get(subject.source())
        .ok_or_else(|| WorkspaceDiagnosticError::missing_source(subject.source().index()))?;
    let tree = failure
        .reached()
        .syntax_trees()
        .iter()
        .find(|tree| tree.node(subject).is_some())
        .ok_or_else(|| {
            WorkspaceDiagnosticError::missing_syntax_subject(
                subject.source().index(),
                subject.index(),
            )
        })?;
    let node = tree.node(subject).ok_or_else(|| {
        WorkspaceDiagnosticError::missing_syntax_subject(subject.source().index(), subject.index())
    })?;
    Ok(Box::new([nocter_diagnostics::SourceDiagnostic::new(
        "E0800",
        error.to_string(),
        source.span(node.range()),
        [],
        None::<Box<str>>,
    )]))
}

/// A package diagnostic identity absent from the exact preparation snapshot that produced it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceDiagnosticError {
    kind: WorkspaceDiagnosticErrorKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspaceDiagnosticErrorKind {
    MissingSource(u32),
    MissingSyntaxSubject { source: u32, node: usize },
}

impl WorkspaceDiagnosticError {
    const fn missing_source(source: u32) -> Self {
        Self {
            kind: WorkspaceDiagnosticErrorKind::MissingSource(source),
        }
    }

    const fn missing_syntax_subject(source: u32, node: usize) -> Self {
        Self {
            kind: WorkspaceDiagnosticErrorKind::MissingSyntaxSubject { source, node },
        }
    }
}

impl fmt::Display for WorkspaceDiagnosticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            WorkspaceDiagnosticErrorKind::MissingSource(source) => {
                write!(
                    formatter,
                    "package diagnostic refers to missing source {source}"
                )
            }
            WorkspaceDiagnosticErrorKind::MissingSyntaxSubject { source, node } => {
                write!(
                    formatter,
                    "package diagnostic refers to missing syntax node {source}:{node}"
                )
            }
        }
    }
}

impl std::error::Error for WorkspaceDiagnosticError {}

/// A protocol-independent workspace preparation failure.
///
/// The concrete package and discovery failures remain private to this crate. Editor protocol
/// layers may render this value and use its stable diagnostic code, but cannot branch on
/// compiler-orchestration internals.
#[derive(Debug)]
pub struct WorkspaceAnalysisError {
    kind: WorkspaceAnalysisErrorKind,
}

#[derive(Debug)]
enum WorkspaceAnalysisErrorKind {
    OutsideWorkspace(PathBuf),
    UnsupportedSource(PathBuf),
    MissingRootPackage(nocter_model::PackageIdentity),
    Package(PackageResolutionFailure),
    StandardPackage(PackageGraphError),
    PackageRootProbe(Arc<nocter_package::PackageRootProbeError>),
    ModuleOwner(nocter_discovery::DiscoveryError),
    Computation(nocter_computation::ComputationError),
    SemanticComputation(nocter_semantic_computation::ScopeInputError),
    DeclarationRejection(nocter_session::DeclarationRejectionDomainError),
}

impl WorkspaceAnalysisError {
    pub(crate) fn missing_root_package(package: nocter_model::PackageIdentity) -> Self {
        Self {
            kind: WorkspaceAnalysisErrorKind::MissingRootPackage(package),
        }
    }

    pub(crate) fn module_owner(error: nocter_discovery::DiscoveryError) -> Self {
        Self {
            kind: WorkspaceAnalysisErrorKind::ModuleOwner(error),
        }
    }

    pub(crate) fn computation(error: nocter_computation::ComputationError) -> Self {
        Self {
            kind: WorkspaceAnalysisErrorKind::Computation(error),
        }
    }

    pub(crate) fn semantic_computation(
        error: nocter_semantic_computation::ScopeInputError,
    ) -> Self {
        Self {
            kind: WorkspaceAnalysisErrorKind::SemanticComputation(error),
        }
    }

    pub(crate) fn declaration_rejection(
        error: nocter_session::DeclarationRejectionDomainError,
    ) -> Self {
        Self {
            kind: WorkspaceAnalysisErrorKind::DeclarationRejection(error),
        }
    }

    pub(crate) fn outside_workspace(path: PathBuf) -> Self {
        Self {
            kind: WorkspaceAnalysisErrorKind::OutsideWorkspace(path),
        }
    }

    pub(crate) fn unsupported_source(path: PathBuf) -> Self {
        Self {
            kind: WorkspaceAnalysisErrorKind::UnsupportedSource(path),
        }
    }

    pub(crate) fn package_root_probe(error: Arc<nocter_package::PackageRootProbeError>) -> Self {
        Self {
            kind: WorkspaceAnalysisErrorKind::PackageRootProbe(error),
        }
    }

    pub(crate) fn package_failure(&self) -> Option<&PackageResolutionFailure> {
        match &self.kind {
            WorkspaceAnalysisErrorKind::Package(failure) => Some(failure),
            WorkspaceAnalysisErrorKind::OutsideWorkspace(_)
            | WorkspaceAnalysisErrorKind::UnsupportedSource(_)
            | WorkspaceAnalysisErrorKind::MissingRootPackage(_)
            | WorkspaceAnalysisErrorKind::StandardPackage(_)
            | WorkspaceAnalysisErrorKind::PackageRootProbe(_)
            | WorkspaceAnalysisErrorKind::ModuleOwner(_)
            | WorkspaceAnalysisErrorKind::Computation(_)
            | WorkspaceAnalysisErrorKind::SemanticComputation(_)
            | WorkspaceAnalysisErrorKind::DeclarationRejection(_) => None,
        }
    }

    #[must_use]
    pub const fn diagnostic_code(&self) -> &'static str {
        match &self.kind {
            WorkspaceAnalysisErrorKind::OutsideWorkspace(_)
            | WorkspaceAnalysisErrorKind::UnsupportedSource(_)
            | WorkspaceAnalysisErrorKind::ModuleOwner(_) => "E0702",
            WorkspaceAnalysisErrorKind::Package(_)
            | WorkspaceAnalysisErrorKind::PackageRootProbe(_) => "E0800",
            WorkspaceAnalysisErrorKind::StandardPackage(_) => "E0703",
            WorkspaceAnalysisErrorKind::MissingRootPackage(_)
            | WorkspaceAnalysisErrorKind::Computation(_)
            | WorkspaceAnalysisErrorKind::SemanticComputation(_)
            | WorkspaceAnalysisErrorKind::DeclarationRejection(_) => "E0900",
        }
    }
}

impl From<PackageResolutionFailure> for WorkspaceAnalysisError {
    fn from(error: PackageResolutionFailure) -> Self {
        Self {
            kind: WorkspaceAnalysisErrorKind::Package(error),
        }
    }
}

impl From<PackageGraphError> for WorkspaceAnalysisError {
    fn from(error: PackageGraphError) -> Self {
        Self {
            kind: WorkspaceAnalysisErrorKind::StandardPackage(error),
        }
    }
}

impl fmt::Display for WorkspaceAnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            WorkspaceAnalysisErrorKind::OutsideWorkspace(path) => write!(
                formatter,
                "document is outside every initialized workspace root: {}",
                path.display()
            ),
            WorkspaceAnalysisErrorKind::UnsupportedSource(path) => {
                write!(
                    formatter,
                    "document is not a Nocter source file: {}",
                    path.display()
                )
            }
            WorkspaceAnalysisErrorKind::MissingRootPackage(package) => write!(
                formatter,
                "resolved package graph is missing root package {}",
                package.as_str()
            ),
            WorkspaceAnalysisErrorKind::Package(error) => error.fmt(formatter),
            WorkspaceAnalysisErrorKind::StandardPackage(error) => {
                write!(formatter, "standard package is invalid: {error}")
            }
            WorkspaceAnalysisErrorKind::PackageRootProbe(error) => error.fmt(formatter),
            WorkspaceAnalysisErrorKind::ModuleOwner(error) => {
                write!(formatter, "cannot determine source module: {error}")
            }
            WorkspaceAnalysisErrorKind::Computation(error) => {
                write!(formatter, "workspace computation failed: {error}")
            }
            WorkspaceAnalysisErrorKind::SemanticComputation(error) => {
                write!(formatter, "semantic computation input is invalid: {error}")
            }
            WorkspaceAnalysisErrorKind::DeclarationRejection(error) => {
                write!(formatter, "declaration rejection input is invalid: {error}")
            }
        }
    }
}

impl std::error::Error for WorkspaceAnalysisError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            WorkspaceAnalysisErrorKind::Package(error) => Some(error),
            WorkspaceAnalysisErrorKind::StandardPackage(error) => Some(error),
            WorkspaceAnalysisErrorKind::PackageRootProbe(error) => Some(error.as_ref()),
            WorkspaceAnalysisErrorKind::ModuleOwner(error) => Some(error),
            WorkspaceAnalysisErrorKind::Computation(error) => Some(error),
            WorkspaceAnalysisErrorKind::SemanticComputation(error) => Some(error),
            WorkspaceAnalysisErrorKind::DeclarationRejection(error) => Some(error),
            WorkspaceAnalysisErrorKind::OutsideWorkspace(_)
            | WorkspaceAnalysisErrorKind::UnsupportedSource(_)
            | WorkspaceAnalysisErrorKind::MissingRootPackage(_) => None,
        }
    }
}
