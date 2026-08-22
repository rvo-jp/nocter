mod build;
mod comparison;
mod diagnostic;
mod expansion;
mod methods;
mod model;
mod requirements;
mod selection;

#[cfg(test)]
mod tests;

pub use build::{
    InstanceOperationBuildError, InstanceOperationInternalError, build_instance_operation_table,
};
pub(crate) use comparison::ComparisonCandidateImplementation;
pub use diagnostic::InstanceOperationRule;
pub(crate) use methods::{MethodCandidate, MethodCompletionCandidate, receiver_supports};
pub use model::{CheckedInstanceOperations, InstanceOperationTable};
pub use selection::InstanceSelectionError;
pub(crate) use selection::{
    IndexOperationCandidate, InstanceOperationSelector, InstanceSelectionContext,
    retain_direct_candidates, selected_generic_arguments,
};
