//! One-way construction of the syntax-independent checked program.
//!
//! This crate is the only Phase 3 boundary allowed to inspect body syntax. It consumes the
//! declaration-lowering result and extends the separate source projection while constructing
//! checked semantic identities. Later stages may consume its immutable semantic values, but they
//! cannot construct them, inspect body syntax, or reopen a checked selection.

mod body_check;
mod body_sources;
mod checked;
mod concrete_destruction;
mod concrete_dispatch;
mod conformance;
mod construction_completion;
mod construction_surfaces;
mod copyability;
mod enum_pattern_completion;
mod expected;
mod field_selection;
mod inference;
mod instance_operations;
mod loans;
mod member_completion;
mod name_recovery;
mod names;
mod ownership;
mod pattern_requirements;
mod preparation;
mod provenance;
mod recovery;
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

pub use body_check::{
    BodyCheckError, BodyCheckFailure, BodyCheckInternalError, BodyRule, TypedBodyInterruption,
    TypedBodyInterruptionKind, check_prepared_program, check_prepared_program_recovering,
};
pub use body_sources::{BodySource, BodySourceCatalog, BodySourceError, catalog_body_sources};
pub use checked::{
    AggregateConstruction, AllocationSelection, AmbientStorageDependence,
    BorrowConversionImplementation, BorrowConversionPreparation, BuildCheckedBodyError, CallTarget,
    CallableProvenanceTable, CheckedBody, CheckedBodyLoans, CheckedBodyProvenance,
    CheckedBorrowConversion, CheckedCall, CheckedCallableProvenance, CheckedCapture,
    CheckedClosure, CheckedClosureCapture, CheckedClosureProvenance, CheckedComparison,
    CheckedControl, CheckedInterpolation, CheckedIteratorAcquisition, CheckedLoan, CheckedLocal,
    CheckedLoop, CheckedNode, CheckedOpaqueWitness, CheckedOperation, CheckedOutcome,
    CheckedPattern, CheckedPatternArm, CheckedPatternFallback, CheckedPatternSlot,
    CheckedPatternSubject, CheckedPlace, CheckedProgram, CheckedProgramOutput,
    CheckedReadonlyOperand, CheckedReceiver, CheckedReceiverCoercion, CheckedSequence,
    CleanupAction, CleanupCondition, CleanupPath, CleanupSchedule, CleanupTable, CleanupTarget,
    CleanupTiming, ClosureDefinition, ClosureEnvironmentField, ClosureProvenanceTable,
    ClosureSignature, ClosureTable, ClosureTableBuildError, CoercedReceiverPreparation,
    ComparisonImplementation, ComparisonOperation, ConstantValue, DropSelection,
    DuplicateGenericArgument, GenericArgument, GenericArguments, InterpolationPart,
    IterationAcquisition, LoanId, LoanPlace, LoanProjection, LoanRoot, LoanTable, LogicalOperation,
    LoopKind, OpaqueWitnessTable, OpaqueWitnessTableBuildError, PatternBindingMode,
    PatternRemainder, PatternSubjectPreparation, PlaceAccess, PlaceProjection, PlaceRoot,
    PrimitiveBinary, PrimitiveOperation, PrimitiveUnary, ProvenanceProjection, ProvenanceSource,
    ProvenanceTable, ReadonlyOperandPreparation, ReceiverPreparation, SequenceElement, SpreadMode,
    StaticDispatch, StaticSelection, TypedIteration, ValueProvenance,
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
pub use conformance::{
    CheckedConformance, CheckedPredicate, CheckedRequirement, ConformanceBuildError,
    ConformanceInternalError, ConformanceMethod, ConformanceRule, ConformanceTable,
    MethodSelection, SubstitutionError, build_conformance_table,
};
pub use construction_completion::{
    ConstructionCompletionCandidate, ConstructionCompletionError, ConstructionCompletionOwner,
    ConstructionCompletionTarget,
};
pub use construction_surfaces::{
    ConstructionSurfaceBuildError, ConstructionSurfaceSelectionError, ConstructionSurfaceTable,
    SelectedConstructionEntry, SelectedConstructionSurface,
};
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
    CheckedInstanceOperations, InstanceOperationBuildError, InstanceOperationInternalError,
    InstanceOperationRule, InstanceOperationTable, InstanceSelectionError,
    build_instance_operation_table,
};
pub use member_completion::{
    MemberCompletionCandidate, MemberCompletionContext, MemberCompletionError,
    MemberCompletionTarget,
};
pub use name_recovery::NameAnalysisRecovery;
pub use names::{
    BodyScope, Capture, CaptureMode, LocalBinding, LocalBindingKind, NameResolution,
    NameResolutionError, NameRule, NameTarget, ResolvedBodyNames, ResolvedNameUse, ScopeBinding,
    resolve_body_names,
};
pub use ownership::{DropTable, DropTableError};
pub use preparation::{
    PreparationError, PreparationFailure, PreparedChecking, PreparedSemanticProgram,
    prepare_program_checking, prepare_program_checking_recovering,
};
pub use recovery::{BodyAnalysisRecovery, SemanticAnalysisRecovery};
pub use standard_semantics::{StandardSemanticError, StandardSemanticTable};
pub use structural_field_completion::{
    StructuralFieldCompletionCandidate, StructuralFieldCompletionError,
};
pub use type_relations::{TypeSubstitution, is_concrete_type};
pub use type_validity::{
    DeclarationTypeValidityError, TypePosition, TypeValidityFailure, TypeValidityInternalError,
    TypeValidityRule, TypeValidityViolation, validate_declaration_types, validate_type,
};
