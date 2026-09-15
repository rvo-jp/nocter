use std::collections::HashMap;

use nocter_declarations::{DeclarationProgramBuilder, PreparedDeclarationProgram};
use nocter_model::{ConstantId, StaticId};

use crate::types::{PreparedConstantValue, PreparedStaticValue};

use super::HeaderDefinitionError;

/// Completes the value authority only after declaration metadata has frozen successfully.
///
/// The header evaluator and declaration definition retain separate products until this consuming
/// join. Sorting by semantic identity makes the transition deterministic without giving source
/// order semantic meaning.
pub(super) fn complete(
    program: DeclarationProgramBuilder,
    constants: HashMap<ConstantId, PreparedConstantValue>,
    statics: HashMap<StaticId, PreparedStaticValue>,
) -> Result<PreparedDeclarationProgram, HeaderDefinitionError> {
    let mut program = program.prepare()?;
    let mut constants = constants
        .into_iter()
        .map(|(id, prepared)| (id, prepared.value))
        .collect::<Vec<_>>();
    constants.sort_unstable_by_key(|(id, _)| *id);
    for (id, value) in constants {
        program.define_constant_value(id, value)?;
    }
    let mut statics = statics
        .into_iter()
        .map(|(id, prepared)| (id, prepared.value))
        .collect::<Vec<_>>();
    statics.sort_unstable_by_key(|(id, _)| *id);
    for (id, value) in statics {
        program.define_static_value(id, value)?;
    }
    Ok(program)
}
