mod analysis;
mod table;

use crate::{BodyCheckError, ClosureTable, body_relations::BodyRelationCatalog};

pub use table::{AllocationEffect, EffectTable};

pub(crate) fn analyze_program_effects(
    environment: &crate::program_environment::ProgramEnvironment,
    closures: &ClosureTable,
    inputs: &BodyRelationCatalog<'_, '_>,
) -> Result<EffectTable, BodyCheckError> {
    analysis::analyze_program(environment, closures, inputs)
}
