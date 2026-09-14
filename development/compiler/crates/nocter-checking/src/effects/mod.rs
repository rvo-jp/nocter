mod analysis;
mod table;

use crate::{BodyRelationError, ClosureTable, body_relations::BodyRelationCatalog};

pub use table::{AllocationFact, ExecutionFactTable, ExecutionFacts, SynchronousWaitFact};

pub(crate) fn analyze_program_execution(
    environment: &crate::program_environment::ProgramEnvironment,
    closures: &ClosureTable,
    inputs: &BodyRelationCatalog<'_>,
) -> Result<ExecutionFactTable, BodyRelationError> {
    analysis::analyze_program(environment, closures, inputs)
}
