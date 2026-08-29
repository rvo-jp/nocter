use std::fmt;

use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{
    BodyId, BodyNodeId, CaptureId, ClosureId, LocalBindingId, LoopId, PlaceId, TypeId,
};
use nocter_source_index::SemanticEntity;
use nocter_syntax::{NodeId, NodeKind, SyntaxOrigin};

use crate::checked::{BuildCheckedBodyError, ClosureTableBuildError};
use crate::instance_operations::InstanceSelectionError;
use crate::{
    BodyRule, ConstantExpressionRule, CopyabilityError, ExpectedTypeError, NameTarget,
    TypeValidityRule,
};

/// Internal result of constructing one body before program-level recovery is assembled.
pub(super) struct BodyConstructionFailure {
    error: Box<BodyCheckError>,
    interruption: Option<super::TypedBodyInterruption>,
}

impl BodyConstructionFailure {
    pub(super) fn new(
        error: BodyCheckError,
        interruption: Option<super::TypedBodyInterruption>,
    ) -> Self {
        Self {
            error: Box::new(error),
            interruption,
        }
    }

    pub(super) fn into_parts(self) -> (BodyCheckError, Option<super::TypedBodyInterruption>) {
        (*self.error, self.interruption)
    }

    pub(super) const fn interruption(&self) -> Option<&super::TypedBodyInterruption> {
        self.interruption.as_ref()
    }
}

/// A typed-body failure with the deepest current-generation semantic state that remains valid.
#[derive(Clone, Debug)]
pub struct BodyCheckFailure {
    error: BodyCheckError,
    recovery: Option<Box<crate::BodyAnalysisRecovery>>,
}

impl BodyCheckFailure {
    pub(crate) fn new(
        error: BodyCheckError,
        recovery: Option<crate::BodyAnalysisRecovery>,
    ) -> Self {
        Self {
            error,
            recovery: recovery.map(Box::new),
        }
    }

    /// Constructs a failure from an explicit recovery assembly outcome.
    ///
    /// Recovery is optional by policy, but a requested recovery that fails to assemble is a
    /// compiler consistency failure. It must replace the authored error rather than disappear
    /// behind an empty recovery value.
    pub(crate) fn from_recovery_result(
        error: BodyCheckError,
        recovery: Result<Option<crate::BodyAnalysisRecovery>, BodyCheckInternalError>,
    ) -> Self {
        match recovery {
            Ok(recovery) => Self::new(error, recovery),
            Err(internal) => Self::new(internal.into(), None),
        }
    }

    #[must_use]
    pub const fn error(&self) -> &BodyCheckError {
        &self.error
    }

    #[must_use]
    pub fn recovery(&self) -> Option<&crate::BodyAnalysisRecovery> {
        self.recovery.as_deref()
    }

    #[must_use]
    pub fn into_parts(self) -> (BodyCheckError, Option<crate::BodyAnalysisRecovery>) {
        (self.error, self.recovery.map(|recovery| *recovery))
    }

    /// Opens an owned consumer branch from one immutable exact-current query failure.
    #[must_use]
    pub fn current_branch(&self) -> Self {
        self.clone()
    }
}

#[derive(Clone, Debug)]
pub enum BodyCheckError {
    Rule {
        rule: BodyRule,
        diagnostic: SourceDiagnostic,
    },
    TypeValidity {
        rule: TypeValidityRule,
        diagnostic: SourceDiagnostic,
    },
    ConstantExpression {
        rule: ConstantExpressionRule,
        diagnostic: SourceDiagnostic,
    },
    Internal(BodyCheckInternalError),
}

impl BodyCheckError {
    #[must_use]
    pub const fn source_diagnostic(&self) -> Option<&SourceDiagnostic> {
        match self {
            Self::Rule { diagnostic, .. }
            | Self::TypeValidity { diagnostic, .. }
            | Self::ConstantExpression { diagnostic, .. } => Some(diagnostic),
            Self::Internal(_) => None,
        }
    }

    #[must_use]
    pub const fn rule(&self) -> Option<BodyRule> {
        match self {
            Self::Rule { rule, .. } => Some(*rule),
            Self::TypeValidity { .. } | Self::ConstantExpression { .. } | Self::Internal(_) => None,
        }
    }

    pub(crate) fn clone_authored(&self) -> Option<Self> {
        match self {
            Self::Rule { rule, diagnostic } => Some(Self::Rule {
                rule: *rule,
                diagnostic: diagnostic.clone(),
            }),
            Self::TypeValidity { rule, diagnostic } => Some(Self::TypeValidity {
                rule: *rule,
                diagnostic: diagnostic.clone(),
            }),
            Self::ConstantExpression { rule, diagnostic } => Some(Self::ConstantExpression {
                rule: *rule,
                diagnostic: diagnostic.clone(),
            }),
            Self::Internal(_) => None,
        }
    }

    #[must_use]
    pub const fn type_validity_rule(&self) -> Option<TypeValidityRule> {
        match self {
            Self::TypeValidity { rule, .. } => Some(*rule),
            Self::Rule { .. } | Self::ConstantExpression { .. } | Self::Internal(_) => None,
        }
    }

    #[must_use]
    pub const fn constant_expression_rule(&self) -> Option<ConstantExpressionRule> {
        match self {
            Self::ConstantExpression { rule, .. } => Some(*rule),
            Self::Rule { .. } | Self::TypeValidity { .. } | Self::Internal(_) => None,
        }
    }

    pub(crate) const fn from_rule(rule: BodyRule, diagnostic: SourceDiagnostic) -> Self {
        Self::Rule { rule, diagnostic }
    }

    pub(crate) const fn from_type_validity(
        rule: TypeValidityRule,
        diagnostic: SourceDiagnostic,
    ) -> Self {
        Self::TypeValidity { rule, diagnostic }
    }

    pub(crate) const fn from_constant_expression(
        rule: ConstantExpressionRule,
        diagnostic: SourceDiagnostic,
    ) -> Self {
        Self::ConstantExpression { rule, diagnostic }
    }
}

impl fmt::Display for BodyCheckError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule { diagnostic, .. }
            | Self::TypeValidity { diagnostic, .. }
            | Self::ConstantExpression { diagnostic, .. } => {
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

#[derive(Clone, Debug)]
pub enum BodyCheckInternalError {
    MissingBodySource(BodyId),
    MissingBodyNames(BodyId),
    BodyIdentityMismatch(BodyId),
    SourceAccess(nocter_frontend_bindings::SourceAccessError),
    SourceModuleMismatch(BodyId),
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
    Copyability(CopyabilityError),
    ExpectedType(ExpectedTypeError),
    Construction(BuildCheckedBodyError),
    ClosureConstruction(ClosureTableBuildError),
    MissingSource(SemanticEntity),
    UnconsumedNameUses(BodyId),
    MissingBodyEvidence(BodyId),
    NonCanonicalBody(BodyId),
    OwnershipState,
    FieldSelection,
    InstanceSelection(InstanceSelectionError),
    IndexSelection,
    BodyAssumptions(crate::SubstitutionError),
    CallSubstitution(crate::SubstitutionError),
    AssociatedTypeResolution(crate::AssociatedTypeResolutionError),
    CallInference(crate::InferenceFailure),
    CallGenericArguments(crate::DuplicateGenericArgument),
    CallContractSelection,
    ConstructionSurfaceSelection(crate::ConstructionSurfaceSelectionError),
    MissingAllocationSemanticRoles,
    MissingInterpolationSemanticRoles,
    MissingIterationSemanticRoles,
    ExpectedConversion,
    CleanupPlanning,
    ProvenanceAnalysis,
    LoanAnalysis,
    OpaqueWitnessPlanning,
    TypeProjection(nocter_model::TypeProjectionError),
    BodyClosureRecipe(crate::BodyClosureRecipeError),
    BodyTypeRecipe(crate::BodyTypeRecipeError),
    CheckedSemanticRebind(crate::CheckedSemanticRebindError),
    BodySourceRecipe(super::source_recipe::BodySourceRecipeError),
    MissingReusableBody(BodyId),
    DuplicateReusableBody(BodyId),
    UnknownReusableBody(BodyId),
    BodySemanticCommit,
    InvalidQueriedBodyRejection(BodyId),
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

impl From<crate::AssociatedTypeResolutionError> for BodyCheckInternalError {
    fn from(error: crate::AssociatedTypeResolutionError) -> Self {
        Self::AssociatedTypeResolution(error)
    }
}

impl From<crate::ConstructionSurfaceSelectionError> for BodyCheckInternalError {
    fn from(error: crate::ConstructionSurfaceSelectionError) -> Self {
        Self::ConstructionSurfaceSelection(error)
    }
}

impl From<nocter_model::TypeProjectionError> for BodyCheckInternalError {
    fn from(error: nocter_model::TypeProjectionError) -> Self {
        Self::TypeProjection(error)
    }
}

impl From<crate::BodyClosureRecipeError> for BodyCheckInternalError {
    fn from(error: crate::BodyClosureRecipeError) -> Self {
        Self::BodyClosureRecipe(error)
    }
}

impl From<crate::BodyTypeRecipeError> for BodyCheckInternalError {
    fn from(error: crate::BodyTypeRecipeError) -> Self {
        Self::BodyTypeRecipe(error)
    }
}

impl From<crate::CheckedSemanticRebindError> for BodyCheckInternalError {
    fn from(error: crate::CheckedSemanticRebindError) -> Self {
        Self::CheckedSemanticRebind(error)
    }
}

impl From<super::source_recipe::BodySourceRecipeError> for BodyCheckInternalError {
    fn from(error: super::source_recipe::BodySourceRecipeError) -> Self {
        Self::BodySourceRecipe(error)
    }
}
