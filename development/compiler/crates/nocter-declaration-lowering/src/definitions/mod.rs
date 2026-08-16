use std::fmt;

use nocter_declarations::{DefinitionError, ProgramBuildError};
use nocter_model::TypeId;
use nocter_source::SourceId;
use nocter_source_index::DuplicateSourceBinding;
use nocter_syntax::NodeId;

use crate::{LoweredDeclarations, PreparedTypes, SurfaceDeclarationId};

mod allocation;
mod declarations;
mod diagnostic;
mod projection;
mod syntax;

#[cfg(test)]
mod tests;

pub use diagnostic::DeclarationDiagnostic;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HeaderDefinitionError {
    MissingSource(SurfaceDeclarationId),
    MissingName(SurfaceDeclarationId),
    MissingSite(SurfaceDeclarationId),
    MissingType(NodeId),
    MissingCallableResult(SurfaceDeclarationId),
    InvalidOwner(SurfaceDeclarationId),
    InvalidSurface(SurfaceDeclarationId),
    InvalidTypePattern(SurfaceDeclarationId),
    InvalidTargetGate(SurfaceDeclarationId),
    DuplicateDefault(SurfaceDeclarationId),
    InvalidProvenance(SurfaceDeclarationId),
    AmbiguousProvenance(SurfaceDeclarationId),
    UnknownAssociatedType(SurfaceDeclarationId),
    DuplicateAssociatedType(SurfaceDeclarationId),
    InconsistentType(TypeId),
    InconsistentSource(SourceId),
    MissingDiagnosticSubject(nocter_model::DeclarationSiteId),
    Declaration(DeclarationDiagnostic),
    Definition(DefinitionError),
    Program(ProgramBuildError),
    DuplicateSourceBinding(DuplicateSourceBinding),
}

impl fmt::Display for HeaderDefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::InvalidTargetGate(declaration) => {
                write!(
                    formatter,
                    "declaration {declaration:?} has an invalid target gate"
                )
            }
            Self::DuplicateDefault(declaration) => write!(
                formatter,
                "construction {declaration:?} has more than one default member"
            ),
            Self::InvalidProvenance(declaration) => write!(
                formatter,
                "callable {declaration:?} has an invalid result provenance contract"
            ),
            Self::AmbiguousProvenance(declaration) => write!(
                formatter,
                "bodyless callable {declaration:?} requires explicit result provenance"
            ),
            Self::UnknownAssociatedType(declaration) => write!(
                formatter,
                "conformance {declaration:?} binds an unknown associated type"
            ),
            Self::DuplicateAssociatedType(declaration) => write!(
                formatter,
                "conformance {declaration:?} repeats an associated type binding"
            ),
            Self::InconsistentType(ty) => write!(formatter, "type {ty:?} is inconsistent"),
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

/// Completes every reserved declaration header and freezes the immutable declaration graph.
///
/// Source syntax is consumed at this boundary. The returned semantic program and source index are
/// independent immutable values, and later stages cannot reconstruct header decisions from syntax.
///
/// # Errors
///
/// Returns [`HeaderDefinitionError`] for an incomplete or inconsistent header, source projection,
/// provenance contract, associated binding, or declaration-program invariant.
pub fn define_declaration_headers(
    mut types: PreparedTypes<'_>,
) -> Result<LoweredDeclarations, HeaderDefinitionError> {
    let mut allocation = allocation::allocate(&mut types)?;
    declarations::define(&mut types, &mut allocation)?;
    allocation::finish(types, allocation)
}
