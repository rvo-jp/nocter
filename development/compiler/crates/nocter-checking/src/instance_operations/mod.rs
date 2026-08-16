mod build;
mod diagnostic;
mod model;
mod selection;

#[cfg(test)]
mod tests;

pub use build::{
    InstanceOperationBuildError, InstanceOperationInternalError, build_instance_operation_table,
};
pub use diagnostic::InstanceOperationRule;
pub use model::{CheckedInstanceOperations, InstanceOperationTable};
pub use selection::InstanceSelectionError;
pub(crate) use selection::{
    IndexOperationCandidate, select_coerced_index_operations, select_index_operations,
};
