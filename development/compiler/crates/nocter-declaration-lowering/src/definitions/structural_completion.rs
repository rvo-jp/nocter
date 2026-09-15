use std::collections::HashMap;

use nocter_declarations::{DeclarationProgramBuilder, PreparedDeclarationProgram};
use nocter_model::ConstantId;

use crate::types::PreparedStructuralConstant;

use super::HeaderDefinitionError;

/// Attaches the scalar constants admitted for later type-shape construction.
///
/// Ordinary constant and immutable-static initializer values are deliberately absent: checked
/// initializer plans own that later semantic product.
pub(super) fn complete(
    program: DeclarationProgramBuilder,
    constants: HashMap<ConstantId, PreparedStructuralConstant>,
) -> Result<PreparedDeclarationProgram, HeaderDefinitionError> {
    let mut program = program.prepare()?;
    let mut constants = constants
        .into_iter()
        .map(|(id, prepared)| (id, prepared.value))
        .collect::<Vec<_>>();
    constants.sort_unstable_by_key(|(id, _)| *id);
    for (id, value) in constants {
        program.define_structural_constant(id, value)?;
    }
    Ok(program)
}
