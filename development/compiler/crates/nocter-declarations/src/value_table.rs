use nocter_model::{Arena, ConstantId, ConstantValue, FrozenValue, StaticId, TypeStore};

use crate::{DeclarationGraph, StructuralConstantTable};

/// Evaluated values paired with declaration identities without becoming declaration metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationValueTable {
    constants: Arena<ConstantId, ConstantValue>,
    statics: Arena<StaticId, FrozenValue>,
}

impl DeclarationValueTable {
    /// Validates and joins complete dense constant and static authorities.
    ///
    /// This boundary rejects a missing, extra, or type-incompatible value before it can become an
    /// observable semantic product.
    ///
    /// # Errors
    ///
    /// Returns the exact declaration-value domain or identity that disagrees with `graph`.
    pub fn for_program(
        graph: &DeclarationGraph,
        types: &TypeStore,
        structural: &StructuralConstantTable,
        constants: Arena<ConstantId, ConstantValue>,
        statics: Arena<StaticId, FrozenValue>,
    ) -> Result<Self, DeclarationValueTableError> {
        if constants.len() != graph.declarations().constants().len() {
            return Err(DeclarationValueTableError::ConstantDomain);
        }
        if statics.len() != graph.declarations().statics().len() {
            return Err(DeclarationValueTableError::StaticDomain);
        }
        for (id, declaration) in graph.declarations().constants().iter() {
            let value = constants
                .get(id)
                .ok_or(DeclarationValueTableError::MissingConstant(id))?;
            if !crate::value_shape::constant_matches(types, declaration.ty(), value) {
                return Err(DeclarationValueTableError::InvalidConstant(id));
            }
        }
        for (&id, structural_value) in structural.constants() {
            let final_value = constants
                .get(id)
                .ok_or(DeclarationValueTableError::MissingConstant(id))?;
            if final_value != structural_value {
                return Err(DeclarationValueTableError::StructuralDisagreement(id));
            }
        }
        for (id, declaration) in graph.declarations().statics().iter() {
            let value = statics
                .get(id)
                .ok_or(DeclarationValueTableError::MissingStatic(id))?;
            if !crate::value_shape::frozen_matches(types, declaration.ty(), value) {
                return Err(DeclarationValueTableError::InvalidStatic(id));
            }
        }
        Ok(Self { constants, statics })
    }

    #[must_use]
    pub const fn constants(&self) -> &Arena<ConstantId, ConstantValue> {
        &self.constants
    }

    #[must_use]
    pub const fn statics(&self) -> &Arena<StaticId, FrozenValue> {
        &self.statics
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationValueTableError {
    ConstantDomain,
    StaticDomain,
    MissingConstant(ConstantId),
    MissingStatic(StaticId),
    InvalidConstant(ConstantId),
    InvalidStatic(StaticId),
    StructuralDisagreement(ConstantId),
}

impl std::fmt::Display for DeclarationValueTableError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid declaration value table: {self:?}")
    }
}

impl std::error::Error for DeclarationValueTableError {}
