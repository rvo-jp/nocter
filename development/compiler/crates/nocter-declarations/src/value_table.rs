use nocter_model::{Arena, ArenaBuilder, ConstantId, ConstantValue, FrozenValue, StaticId};

use crate::DefinitionError;

/// Evaluated values paired with declaration identities without becoming declaration metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationValueTable {
    constants: Arena<ConstantId, ConstantValue>,
    statics: Arena<StaticId, FrozenValue>,
}

impl DeclarationValueTable {
    /// Joins already complete dense constant and static authorities.
    ///
    /// This constructor cannot represent an unfilled declaration slot. Pairing identities and
    /// validating declared types remains the declaration program's responsibility.
    #[must_use]
    pub const fn new(
        constants: Arena<ConstantId, ConstantValue>,
        statics: Arena<StaticId, FrozenValue>,
    ) -> Self {
        Self { constants, statics }
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

#[derive(Debug, Default)]
pub(crate) struct DeclarationValueTableBuilder {
    constants: ArenaBuilder<ConstantId, Option<ConstantValue>>,
    statics: ArenaBuilder<StaticId, Option<FrozenValue>>,
}

impl DeclarationValueTableBuilder {
    pub(crate) fn reserve_constant(&mut self) -> ConstantId {
        self.constants.insert(None)
    }

    pub(crate) fn define_constant(
        &mut self,
        id: ConstantId,
        value: ConstantValue,
    ) -> Result<(), DefinitionError> {
        let slot = self
            .constants
            .get_mut(id)
            .ok_or(DefinitionError::UnknownId)?;
        if slot.is_some() {
            return Err(DefinitionError::AlreadyDefined);
        }
        *slot = Some(value);
        Ok(())
    }

    pub(crate) fn reserve_static(&mut self) -> StaticId {
        self.statics.insert(None)
    }

    pub(crate) fn define_static(
        &mut self,
        id: StaticId,
        value: FrozenValue,
    ) -> Result<(), DefinitionError> {
        let slot = self.statics.get_mut(id).ok_or(DefinitionError::UnknownId)?;
        if slot.is_some() {
            return Err(DefinitionError::AlreadyDefined);
        }
        *slot = Some(value);
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<DeclarationValueTable, DeclarationValueTableError> {
        Ok(DeclarationValueTable {
            constants: self.constants.try_finish_with(|id, value| {
                value.ok_or(DeclarationValueTableError::MissingConstant(id))
            })?,
            statics: self.statics.try_finish_with(|id, value| {
                value.ok_or(DeclarationValueTableError::MissingStatic(id))
            })?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationValueTableError {
    MissingConstant(ConstantId),
    MissingStatic(StaticId),
}

impl std::fmt::Display for DeclarationValueTableError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid declaration value table: {self:?}")
    }
}

impl std::error::Error for DeclarationValueTableError {}
