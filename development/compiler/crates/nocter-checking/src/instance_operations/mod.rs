mod build;
mod comparison;
mod contracts;
mod diagnostic;
mod evidence;
mod expansion;
mod methods;
mod model;
mod requirements;
mod selection;

#[cfg(test)]
mod tests;

#[cfg(test)]
use build::build_instance_operation_table;
pub(crate) use build::build_instance_operation_table_from_ids;
pub use build::{InstanceOperationBuildError, InstanceOperationInternalError};
pub(crate) use comparison::ComparisonCandidateImplementation;
pub use contracts::{
    CheckedInstanceCoercion, CheckedInstanceComparison, CheckedInstanceExpansion,
    CheckedInstanceIndex, CheckedInstanceMember, CheckedInstanceMethod,
};
pub use diagnostic::InstanceOperationRule;
pub(crate) use evidence::ConcreteEvidenceAuthority;
pub(crate) use methods::{MethodCandidate, MethodCompletionCandidate, receiver_supports};
pub use model::{CheckedInstanceOperations, InstanceOperationTable};
pub use selection::InstanceSelectionError;
pub(crate) use selection::{
    IndexOperationCandidate, InstanceOperationSelector, InstanceSelectionContext,
    retain_direct_candidates, selected_generic_arguments,
};
