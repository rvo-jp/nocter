mod checker;
mod closure_capability;
mod context;
mod diagnostic;
mod error;
mod interruption;
mod literal;
mod ownership;
mod pipeline;
mod query;
mod reusable_body;
mod semantic_transaction;
mod source_recipe;

#[cfg(test)]
mod aggregate_tests;
#[cfg(test)]
mod arithmetic_tests;
#[cfg(test)]
mod assignment_tests;
#[cfg(test)]
mod binding_tests;
#[cfg(test)]
mod call_tests;
#[cfg(test)]
mod callable_value_tests;
#[cfg(test)]
mod cleanup_tests;
#[cfg(test)]
mod closure_tests;
#[cfg(test)]
mod construction_tests;
#[cfg(test)]
mod control_tests;
#[cfg(test)]
mod conversion_tests;
#[cfg(test)]
mod copy_tests;
#[cfg(test)]
mod drop_tests;
#[cfg(test)]
mod flow_tests;
#[cfg(test)]
mod interpolation_tests;
#[cfg(test)]
mod iteration_tests;
#[cfg(test)]
mod loan_tests;
#[cfg(test)]
mod loop_tests;
#[cfg(test)]
mod method_tests;
#[cfg(test)]
mod move_tests;
#[cfg(test)]
mod opaque_tests;
#[cfg(test)]
mod operator_tests;
#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod outcome_tests;
#[cfg(test)]
mod pattern_tests;
#[cfg(test)]
mod place_tests;
#[cfg(test)]
mod provenance_tests;
#[cfg(test)]
mod region_tests;
#[cfg(test)]
mod typed_literal_tests;

pub use diagnostic::BodyRule;
pub use error::{BodyCheckError, BodyCheckFailure, BodyCheckInternalError};
pub use interruption::{TypedBodyInterruption, TypedBodyInterruptionKind};

pub use pipeline::{
    analyze_prepared_program_bodies, check_prepared_program,
    check_prepared_program_from_queried_bodies, check_prepared_program_recovering,
};
pub use query::{
    ProgramBodyCheckingContext, QueriedBodyRejection, ReusableBodyQueryOutcome,
    ReusableProgramBodyCheckError, ReusableProgramBodyNameError,
};
pub use reusable_body::ReusableCheckedBody;
mod assumptions;
pub use assumptions::CapabilityEvidence;
pub(crate) use assumptions::{BodyAssumptionTable, BodyRequirement, CapabilityEvidenceTable};
pub(crate) use semantic_transaction::CheckedSemanticAuthority;
