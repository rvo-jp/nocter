use std::fmt;

use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{BodyId, BodyNodeId, LocalBindingId, TypeId};
use nocter_source_index::{DuplicateSourceBinding, SemanticEntity, SyntaxOrigin};
use nocter_syntax::{NodeId, NodeKind};

use crate::checked::BuildCheckedBodyError;
use crate::{CopyabilityError, ExpectedTypeError, NameTarget};

#[derive(Debug)]
pub enum BodyCheckError {
    Rule(SourceDiagnostic),
    Internal(BodyCheckInternalError),
}

impl BodyCheckError {
    #[must_use]
    pub const fn source_diagnostic(&self) -> Option<&SourceDiagnostic> {
        match self {
            Self::Rule(diagnostic) => Some(diagnostic),
            Self::Internal(_) => None,
        }
    }
}

impl fmt::Display for BodyCheckError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(diagnostic) => {
                write!(formatter, "{}: {}", diagnostic.code(), diagnostic.message())
            }
            Self::Internal(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BodyCheckError {}

impl From<SourceDiagnostic> for BodyCheckError {
    fn from(diagnostic: SourceDiagnostic) -> Self {
        Self::Rule(diagnostic)
    }
}

impl From<BodyCheckInternalError> for BodyCheckError {
    fn from(error: BodyCheckInternalError) -> Self {
        Self::Internal(error)
    }
}

#[derive(Debug)]
pub enum BodyCheckInternalError {
    MissingBodySource(BodyId),
    MissingBodyNames(BodyId),
    BodyIdentityMismatch(BodyId),
    InvalidSyntax(NodeId),
    UnsupportedSyntax(NodeId, NodeKind),
    MissingNameUse(NodeId),
    DuplicateNameUse(SyntaxOrigin),
    UnsupportedNameTarget(NodeId, NameTarget),
    MissingParameterType(NameTarget),
    MissingLocalDeclaration(NodeId),
    DuplicateLocalDeclaration(SyntaxOrigin),
    MissingLocalType(LocalBindingId),
    InvalidLiteral(NodeId),
    UnknownType(TypeId),
    MissingNode(BodyNodeId),
    Copyability(CopyabilityError),
    ExpectedType(ExpectedTypeError),
    Construction(BuildCheckedBodyError),
    DuplicateProjection(DuplicateSourceBinding),
    MissingSource(SemanticEntity),
    UnconsumedNameUses(BodyId),
    NonCanonicalBody(BodyId),
    OwnershipState,
}

impl fmt::Display for BodyCheckInternalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "body checking invariant failed: {self:?}")
    }
}

impl std::error::Error for BodyCheckInternalError {}

impl From<BuildCheckedBodyError> for BodyCheckInternalError {
    fn from(error: BuildCheckedBodyError) -> Self {
        Self::Construction(error)
    }
}

impl From<BuildCheckedBodyError> for BodyCheckError {
    fn from(error: BuildCheckedBodyError) -> Self {
        BodyCheckInternalError::from(error).into()
    }
}

impl From<ExpectedTypeError> for BodyCheckInternalError {
    fn from(error: ExpectedTypeError) -> Self {
        Self::ExpectedType(error)
    }
}

impl From<CopyabilityError> for BodyCheckInternalError {
    fn from(error: CopyabilityError) -> Self {
        Self::Copyability(error)
    }
}

impl From<DuplicateSourceBinding> for BodyCheckInternalError {
    fn from(error: DuplicateSourceBinding) -> Self {
        Self::DuplicateProjection(error)
    }
}
