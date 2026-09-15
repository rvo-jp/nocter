use super::{DeclarationDomain, ProgramIntegrityError};
use crate::{DeclarationProgram, StructuralConstantTable};

/// Validates the sparse constants admitted to structural type construction.
pub(super) fn validate_structural(
    program: &DeclarationProgram,
    values: &StructuralConstantTable,
) -> Result<(), ProgramIntegrityError> {
    for (&id, value) in values.constants() {
        let declaration = program.declarations().constants().get(id).ok_or(
            ProgramIntegrityError::InvalidDeclarationShape(DeclarationDomain::Constant),
        )?;
        if !crate::value_shape::constant_matches(program.types(), declaration.ty(), value) {
            return Err(ProgramIntegrityError::InvalidDeclarationShape(
                DeclarationDomain::Constant,
            ));
        }
    }
    Ok(())
}
