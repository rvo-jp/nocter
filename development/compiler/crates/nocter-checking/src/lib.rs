//! One-way construction of the syntax-independent checked program.
//!
//! This crate is the only Phase 3 boundary allowed to inspect body syntax. It consumes the
//! declaration-lowering result and extends the separate source projection while constructing
//! checked semantic identities. Target validation and later lowering cannot depend on this crate.

mod body_check;
mod body_sources;
mod checked;
mod conformance;
mod copyability;
mod expected;
mod inference;
mod names;
mod preparation;
mod syntax;
mod type_relations;
mod type_validity;

#[cfg(test)]
mod test_support;

pub use body_check::{BodyCheckError, BodyCheckInternalError, BodyRule, check_prepared_program};
pub use body_sources::{BodySource, BodySourceCatalog, BodySourceError, catalog_body_sources};
pub use checked::{
    AggregateConstruction, AllocationSelection, BuildCheckedBodyError, CallTarget, CheckedBody,
    CheckedCall, CheckedCapture, CheckedClosure, CheckedControl, CheckedInterpolation,
    CheckedLocal, CheckedLoop, CheckedMatchArm, CheckedNode, CheckedOperation, CheckedOutcome,
    CheckedPattern, CheckedPlace, CheckedProgram, CheckedProgramOutput, CheckedSequence,
    ConstantValue, DuplicateGenericArgument, GenericArgument, GenericArguments, InterpolationPart,
    IterationAcquisition, LoopKind, PlaceAccess, PlaceProjection, PlaceRoot, PrimitiveBinary,
    PrimitiveOperation, PrimitiveUnary, SequenceElement, SpreadMode, StaticDispatch,
    TypedIteration,
};
pub use conformance::{
    CheckedConformance, CheckedPredicate, CheckedRequirement, ConformanceBuildError,
    ConformanceInternalError, ConformanceMethod, ConformanceRule, ConformanceTable,
    MethodSelection, SubstitutionError, build_conformance_table,
};
pub use copyability::{Copyability, CopyabilityError, CopyabilityTable};
pub use expected::{
    ExpectedBase, ExpectedEvidence, ExpectedTypeError, ExpectedTypePlan, OutcomeLayer,
    plan_expected_type,
};
pub use inference::{CallableInference, InferenceEvidence, InferenceFailure};
pub use names::{
    BodyScope, Capture, CaptureMode, LocalBinding, LocalBindingKind, NameResolution,
    NameResolutionError, NameRule, NameTarget, ResolvedBodyNames, ResolvedNameUse,
    resolve_body_names,
};
pub use preparation::{PreparationError, PreparedChecking, prepare_program_checking};
pub use type_validity::{
    DeclarationTypeValidityError, TypePosition, TypeValidityFailure, TypeValidityInternalError,
    TypeValidityRule, TypeValidityViolation, validate_declaration_types, validate_type,
};
