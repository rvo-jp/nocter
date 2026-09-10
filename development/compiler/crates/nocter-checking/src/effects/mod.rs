mod analysis;
mod table;

use crate::{BodyRelationError, ClosureTable, body_relations::BodyRelationCatalog};

pub use table::{AllocationEffect, BlockingEffect, CallableEffects, EffectTable};

pub(crate) fn analyze_program_effects(
    environment: &crate::program_environment::ProgramEnvironment,
    closures: &ClosureTable,
    inputs: &BodyRelationCatalog<'_>,
) -> Result<EffectTable, BodyRelationError> {
    analysis::analyze_program(environment, closures, inputs)
}
