use std::fmt;

use nocter_declarations::{DefinitionError, ProgramBuildError};
use nocter_source::SourceId;
use nocter_source_index::{DuplicateDocumentation, DuplicateSourceBinding};
use nocter_syntax::NodeId;

use crate::{
    DeclarationLoweringRecovery, LoweredDeclarations, PreparedTypes, SurfaceDeclarationId,
};

mod allocation;
mod declarations;
mod diagnostic;
mod projection;
mod syntax;
mod violation;

#[cfg(test)]
mod tests;

pub use diagnostic::DeclarationDiagnostics;
pub use violation::{DefinitionRule, DefinitionViolation};

#[derive(Debug)]
pub(crate) struct HeaderDefinitionFailure {
    error: Box<HeaderDefinitionError>,
    recovery: Option<Box<DeclarationLoweringRecovery>>,
}

impl HeaderDefinitionFailure {
    pub(super) fn new(
        error: HeaderDefinitionError,
        recovery: Option<DeclarationLoweringRecovery>,
    ) -> Self {
        Self {
            error: Box::new(error),
            recovery: recovery.map(Box::new),
        }
    }

    pub(super) fn without_recovery(error: HeaderDefinitionError) -> Self {
        Self::new(error, None)
    }

    #[must_use]
    pub fn into_parts(self) -> (HeaderDefinitionError, Option<DeclarationLoweringRecovery>) {
        (*self.error, self.recovery.map(|recovery| *recovery))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HeaderDefinitionError {
    Rule(DefinitionViolation),
    MissingSource(SurfaceDeclarationId),
    MissingName(SurfaceDeclarationId),
    MissingSite(SurfaceDeclarationId),
    MissingType(NodeId),
    MissingCallableResult(SurfaceDeclarationId),
    InvalidOwner(SurfaceDeclarationId),
    InvalidSurface(SurfaceDeclarationId),
    InvalidTypePattern(SurfaceDeclarationId),
    InvalidProvenance(SurfaceDeclarationId),
    InconsistentSource(SourceId),
    MissingDiagnosticSubject(nocter_model::DeclarationSiteId),
    Declaration(DeclarationDiagnostics),
    Definition(DefinitionError),
    Program(ProgramBuildError),
    DuplicateSourceBinding(DuplicateSourceBinding),
    DuplicateDocumentation(DuplicateDocumentation),
}

impl fmt::Display for HeaderDefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(violation) => write!(
                formatter,
                "{}: {}",
                violation.rule().code(),
                violation.rule().message()
            ),
            Self::MissingSource(declaration) => {
                write!(formatter, "declaration {declaration:?} has no source")
            }
            Self::MissingName(declaration) => {
                write!(
                    formatter,
                    "declaration {declaration:?} has no resolved name"
                )
            }
            Self::MissingSite(declaration) => {
                write!(
                    formatter,
                    "declaration {declaration:?} has no declaration site"
                )
            }
            Self::MissingType(node) => write!(formatter, "type syntax {node:?} was not normalized"),
            Self::MissingCallableResult(declaration) => write!(
                formatter,
                "callable declaration {declaration:?} has no normalized result"
            ),
            Self::InvalidOwner(declaration) => {
                write!(
                    formatter,
                    "declaration {declaration:?} has an invalid owner"
                )
            }
            Self::InvalidSurface(declaration) => {
                write!(
                    formatter,
                    "declaration {declaration:?} has an invalid header shape"
                )
            }
            Self::InvalidTypePattern(declaration) => write!(
                formatter,
                "declaration {declaration:?} has an invalid normalized type pattern"
            ),
            Self::InvalidProvenance(declaration) => write!(
                formatter,
                "callable {declaration:?} has an invalid result provenance contract"
            ),
            Self::InconsistentSource(source) => {
                write!(formatter, "{source} has an inconsistent declaration origin")
            }
            Self::MissingDiagnosticSubject(site) => {
                write!(
                    formatter,
                    "declaration site {site:?} has no source projection"
                )
            }
            Self::Declaration(diagnostic) => diagnostic.fmt(formatter),
            Self::Definition(error) => error.fmt(formatter),
            Self::Program(error) => error.fmt(formatter),
            Self::DuplicateSourceBinding(error) => error.fmt(formatter),
            Self::DuplicateDocumentation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for HeaderDefinitionError {}

impl From<ProgramBuildError> for HeaderDefinitionError {
    fn from(error: ProgramBuildError) -> Self {
        Self::Program(error)
    }
}

impl From<DefinitionError> for HeaderDefinitionError {
    fn from(error: DefinitionError) -> Self {
        Self::Definition(error)
    }
}

impl From<DuplicateSourceBinding> for HeaderDefinitionError {
    fn from(error: DuplicateSourceBinding) -> Self {
        Self::DuplicateSourceBinding(error)
    }
}

impl From<DuplicateDocumentation> for HeaderDefinitionError {
    fn from(error: DuplicateDocumentation) -> Self {
        Self::DuplicateDocumentation(error)
    }
}

impl From<DefinitionViolation> for HeaderDefinitionError {
    fn from(violation: DefinitionViolation) -> Self {
        Self::Rule(violation)
    }
}

/// Completes declaration headers while retaining a structurally valid program rejected only by
/// an authored declaration rule.
///
/// # Errors
///
/// Returns the exact production definition error and an optional lowering snapshot suitable for
/// editor recovery. Internal integrity failures never carry recovery.
pub(crate) fn define_declaration_headers_recovering(
    mut types: PreparedTypes<'_>,
) -> Result<LoweredDeclarations, HeaderDefinitionFailure> {
    let mut allocation =
        allocation::allocate(&mut types).map_err(HeaderDefinitionFailure::without_recovery)?;
    declarations::define(&mut types, &mut allocation)
        .map_err(HeaderDefinitionFailure::without_recovery)?;
    allocation::finish_recovering(types, allocation)
}
