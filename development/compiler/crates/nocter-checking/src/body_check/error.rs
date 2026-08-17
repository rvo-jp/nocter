use std::fmt;

use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{
    BodyId, BodyNodeId, CaptureId, ClosureId, LocalBindingId, LoopId, PlaceId, TypeId,
};
use nocter_source_index::{DuplicateSourceBinding, SemanticEntity, SyntaxOrigin};
use nocter_syntax::{NodeId, NodeKind};

use crate::checked::{BuildCheckedBodyError, ClosureTableBuildError};
use crate::instance_operations::InstanceSelectionError;
use crate::{BodyRule, CopyabilityError, ExpectedTypeError, NameTarget};

#[derive(Debug)]
pub enum BodyCheckError {
    Rule {
        rule: BodyRule,
        diagnostic: SourceDiagnostic,
    },
    Internal(BodyCheckInternalError),
}

impl BodyCheckError {
    #[must_use]
    pub const fn source_diagnostic(&self) -> Option<&SourceDiagnostic> {
        match self {
            Self::Rule { diagnostic, .. } => Some(diagnostic),
            Self::Internal(_) => None,
        }
    }

    #[must_use]
    pub const fn rule(&self) -> Option<BodyRule> {
        match self {
            Self::Rule { rule, .. } => Some(*rule),
            Self::Internal(_) => None,
        }
    }

    pub(crate) const fn from_rule(rule: BodyRule, diagnostic: SourceDiagnostic) -> Self {
        Self::Rule { rule, diagnostic }
    }
}

impl fmt::Display for BodyCheckError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule { diagnostic, .. } => {
                write!(formatter, "{}: {}", diagnostic.code(), diagnostic.message())
            }
            Self::Internal(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BodyCheckError {}

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
    MissingCallable(nocter_model::CallableId),
    MissingLocalDeclaration(NodeId),
    MissingBlockScope(NodeId),
    DuplicateLocalDeclaration(SyntaxOrigin),
    DuplicateCaptureDeclaration(SyntaxOrigin),
    MissingLocalType(LocalBindingId),
    MissingCaptureType(CaptureId),
    MissingCaptureDeclaration(NodeId),
    MissingClosure(ClosureId),
    InvalidLiteral(NodeId),
    UnknownType(TypeId),
    MissingNode(BodyNodeId),
    MissingNodeOrigin(BodyNodeId),
    DuplicateNodeOrigin(BodyNodeId),
    InvalidMovePlace(PlaceId),
    UnsupportedOwnershipOperation(BodyNodeId),
    LoopStack,
    UnknownLoop(LoopId),
    UnsupportedLoop(LoopId),
    Copyability(CopyabilityError),
    ExpectedType(ExpectedTypeError),
    Construction(BuildCheckedBodyError),
    ClosureConstruction(ClosureTableBuildError),
    DuplicateProjection(DuplicateSourceBinding),
    MissingSource(SemanticEntity),
    UnconsumedNameUses(BodyId),
    NonCanonicalBody(BodyId),
    OwnershipState,
    FieldSelection,
    InstanceSelection(InstanceSelectionError),
    IndexSelection,
    BodyAssumptions(crate::SubstitutionError),
    CallSubstitution(crate::SubstitutionError),
    CallInference(crate::InferenceFailure),
    CallGenericArguments(crate::DuplicateGenericArgument),
    CallContractSelection,
    ConstructionSurfaceSelection(crate::ConstructionSurfaceSelectionError),
    MissingAllocationSemanticRoles,
    MissingInterpolationSemanticRoles,
    ExpectedConversion,
    CleanupPlanning,
    ProvenanceAnalysis,
    LoanAnalysis,
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

impl From<ClosureTableBuildError> for BodyCheckInternalError {
    fn from(error: ClosureTableBuildError) -> Self {
        Self::ClosureConstruction(error)
    }
}

impl From<ClosureTableBuildError> for BodyCheckError {
    fn from(error: ClosureTableBuildError) -> Self {
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

impl From<InstanceSelectionError> for BodyCheckInternalError {
    fn from(error: InstanceSelectionError) -> Self {
        Self::InstanceSelection(error)
    }
}

impl From<crate::ConstructionSurfaceSelectionError> for BodyCheckInternalError {
    fn from(error: crate::ConstructionSurfaceSelectionError) -> Self {
        Self::ConstructionSurfaceSelection(error)
    }
}

impl From<DuplicateSourceBinding> for BodyCheckInternalError {
    fn from(error: DuplicateSourceBinding) -> Self {
        Self::DuplicateProjection(error)
    }
}
