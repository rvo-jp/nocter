#![allow(clippy::disallowed_types)]

//! One-way construction of the syntax-independent checked program.
//!
//! This crate is the only Phase 3 boundary allowed to inspect body syntax. It consumes the
//! declaration-lowering result and extends the separate source projection while constructing
//! checked semantic identities. Later stages may consume its immutable semantic values, but they
//! cannot construct them, inspect body syntax, or reopen a checked selection.

mod admitted_operations;
mod associated_type_completion;
mod associated_type_resolution;
mod body_check;
mod body_evidence;
mod body_name_query;
mod body_sources;
mod body_type_recipe;
mod checked;
mod concrete_destruction;
mod concrete_dispatch;
mod concrete_type;
mod construction_completion;
mod construction_surfaces;
mod copyability;
mod declaration_patterns;
mod enum_pattern_completion;
mod expected;
mod field_selection;
mod inference;
mod instance_operations;
mod interface_implementation;
mod loans;
mod member_completion;
mod name_evidence;
mod name_recovery;
mod names;
mod ownership;
mod pattern_requirements;
mod preparation;
mod program_environment;
mod provenance;
mod recovery;
pub(crate) use preparation::semantic_authority;
mod source_visibility;
mod standard_semantics;
#[cfg(test)]
mod standard_semantics_tests;
mod structural_field_completion;
mod syntax;
#[cfg(test)]
mod target_tests;
mod type_relations;
mod type_validity;

#[cfg(test)]
mod test_support;

pub use associated_type_completion::{
    AssociatedTypeCompletionCandidate, AssociatedTypeCompletionContext,
    AssociatedTypeCompletionError,
};
pub use associated_type_resolution::AssociatedTypeResolutionError;
pub use body_check::{
    BodyCheckError, BodyCheckFailure, BodyCheckInternalError, BodyRule, CapabilityEvidence,
    TypedBodyInterruption, TypedBodyInterruptionKind, analyze_prepared_program_bodies,
    check_prepared_program, check_prepared_program_recovering,
};
pub use body_evidence::{BodyEvidence, BodyRejection, BodyRejectionReason};
pub use body_name_query::{
    ProgramBodyNameContext, ReusableProgramBodyNameError, resolve_reusable_program_body_names,
};
pub use body_sources::{
    BodySource, BodySourceCatalog, BodySourceError, catalog_body_source, catalog_body_sources,
};
pub use body_type_recipe::{
    BodyClosureRef, BodyTypeRecipe, BodyTypeRecipeError, BodyTypeRef, ReplayedBodyTypes,
};
pub use checked::{
    AggregateConstruction, AllocationSelection, AmbientStorageDependence, ArgumentPackSegment,
    BorrowConversionImplementation, BorrowConversionPreparation, BuildCheckedBodyError, CallTarget,
    CallableProvenanceTable, CheckedArgumentPack, CheckedBody, CheckedBodyLoans,
    CheckedBodyProvenance, CheckedBorrowConversion, CheckedCall, CheckedCallableProvenance,
    CheckedCapture, CheckedClosure, CheckedClosureCapture, CheckedClosureProvenance,
    CheckedComparison, CheckedControl, CheckedInterpolation, CheckedIteratorAcquisition,
    CheckedLoan, CheckedLocal, CheckedLoop, CheckedNode, CheckedOpaqueWitness, CheckedOperation,
    CheckedOutcome, CheckedPattern, CheckedPatternArm, CheckedPatternFallback, CheckedPatternSlot,
    CheckedPatternSubject, CheckedPlace, CheckedProgram, CheckedProgramOutput,
    CheckedReadonlyOperand, CheckedReceiver, CheckedReceiverCoercion, CheckedSequence,
    CleanupAction, CleanupCondition, CleanupFieldProjection, CleanupPath, CleanupSchedule,
    CleanupTable, CleanupTarget, CleanupTiming, ClosureDefinition, ClosureEnvironmentField,
    ClosureParameter, ClosureProvenanceTable, ClosureSignature, ClosureTable,
    ClosureTableBuildError, CoercedReceiverPreparation, ComparisonImplementation,
    ComparisonOperation, ConstantValue, DropSelection, DuplicateGenericArgument, GenericArgument,
    GenericArguments, InterpolationPart, IterationAcquisition, LoanId, LoanPlace, LoanProjection,
    LoanRoot, LoanTable, LogicalOperation, LoopKind, OpaqueWitnessTable,
    OpaqueWitnessTableBuildError, PatternBindingMode, PatternRemainder, PatternSubjectPreparation,
    PlaceAccess, PlaceProjection, PlaceRoot, PrimitiveBinary, PrimitiveOperation, PrimitiveUnary,
    ProvenanceProjection, ProvenanceSource, ProvenanceTable, ReadonlyOperandPreparation,
    ReceiverPreparation, SpreadMode, StaticDispatch, StaticSelection, TypedIteration,
    ValueProvenance,
};
pub use concrete_destruction::{
    ConcreteCaptureDestruction, ConcreteDestructionError, ConcreteDestructionKind,
    ConcreteDestructionPlan, ConcreteFieldDestruction, ConcretePayloadDestruction,
    ConcreteVariantDestruction,
};
pub use concrete_dispatch::{
    ConcreteDispatchError, ConcreteDispatchResolver, ResolvedCallableDispatch,
    ResolvedDispatchPlan, ResolvedDispatchStep, ResolvedOpaqueReceiver, ResolvedPrimitiveDispatch,
};
pub use construction_completion::{
    ConstructionCompletionCandidate, ConstructionCompletionError, ConstructionCompletionOwner,
    ConstructionCompletionTarget,
};
pub use construction_surfaces::{ConstructionSurfaceBuildError, ConstructionSurfaceSelectionError};
pub(crate) use construction_surfaces::{ConstructionSurfaceTable, SelectedConstructionEntry};
pub use copyability::{
    CopyCondition, Copyability, CopyabilityBuildError, CopyabilityError, CopyabilityRule,
    CopyabilityTable,
};
pub use enum_pattern_completion::{EnumPatternCompletionCandidate, EnumPatternCompletionError};
pub use expected::{
    ExpectedBase, ExpectedEvidence, ExpectedTypeError, ExpectedTypePlan, OutcomeLayer,
    plan_expected_type,
};
pub use inference::{CallableInference, InferenceEvidence, InferenceFailure};
pub use instance_operations::{
    CheckedInstanceCoercion, CheckedInstanceComparison, CheckedInstanceExpansion,
    CheckedInstanceIndex, CheckedInstanceMember, CheckedInstanceMethod, CheckedInstanceOperations,
    InstanceOperationBuildError, InstanceOperationInternalError, InstanceOperationRule,
    InstanceOperationTable, InstanceSelectionError,
};
pub use interface_implementation::{
    CheckedInterfaceImplementation, CheckedPredicate, CheckedRequirement,
    InterfaceImplementationBuildError, InterfaceImplementationInternalError,
    InterfaceImplementationMethod, InterfaceImplementationRule, InterfaceImplementationTable,
    MethodSelection, MissingInterfaceImplementationMethods, RequiredInterfaceImplementationMethod,
    RequiredInterfaceImplementationParameter, RequirementDerivation, SubstitutionError,
};
pub use member_completion::{
    MemberCompletionCandidate, MemberCompletionError, MemberCompletionQuerySession,
    MemberCompletionTarget,
};
pub use name_evidence::{BodyNameEvidence, NameRejection};
pub use name_recovery::{BodyNameEvidenceTable, NameAnalysisRecovery};
pub use names::{
    BodyScope, Capture, CaptureMode, LocalBinding, LocalBindingKind, NameResolution,
    NameResolutionError, NameRule, NameTarget, ResolvedBodyNames, ResolvedNameUse,
    ReusableBodyNameCatalogError, ReusableBodyNames, ReusableBodyNamesError,
    ReusableBodyResolutionError, ScopeBinding, materialize_reusable_body_name_catalog,
    materialize_reusable_body_names, resolve_body_names, resolve_reusable_body_names,
};
pub use nocter_constant_evaluation::ConstantExpressionRule;
pub use nocter_frontend_bindings::{SourceOwnershipError, SourceOwnershipTable};
pub use ownership::{DropTable, DropTableError};
pub use preparation::{
    PreparationError, PreparationFailure, PreparationFailureEvidence, PreparationRepairEvidence,
    PreparedBodyAnalysis, PreparedChecking, PreparedSemanticProgram, ReusablePreparedProgram,
    prepare_analysis_program_checking_recovering, prepare_program_checking,
    prepare_program_checking_from_reusable_names,
    prepare_program_checking_from_reusable_recovering, prepare_program_checking_recovering,
    prepare_reusable_program,
};
pub use recovery::{BodyAnalysisRecovery, DeclarationAnalysisRecovery, InterruptionEvidenceError};
pub use source_visibility::{SourceAccessContext, SourceVisibilityError};
pub use standard_semantics::{StandardSemanticError, StandardSemanticTable};
pub use structural_field_completion::{
    StructuralFieldCompletionCandidate, StructuralFieldCompletionError,
};
pub use type_relations::{TypeSubstitution, is_concrete_type};
pub use type_validity::{
    DeclarationTypeValidityError, TypePosition, TypeValidityFailure, TypeValidityInternalError,
    TypeValidityRule, TypeValidityViolation, validate_declaration_types, validate_type,
};
