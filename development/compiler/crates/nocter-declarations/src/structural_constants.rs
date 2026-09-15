use std::collections::{BTreeMap, btree_map::Entry};

use nocter_model::{ConstantId, ConstantValue};

/// Complete constant inputs available while declaration and body type shapes are constructed.
///
/// This is not the program's initializer-value authority. It contains only scalar constants
/// admitted by the structural expression language and cannot contain immutable-static values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuralConstantTable {
    constants: BTreeMap<ConstantId, ConstantValue>,
}

impl StructuralConstantTable {
    #[must_use]
    pub const fn constants(&self) -> &BTreeMap<ConstantId, ConstantValue> {
        &self.constants
    }
}

#[derive(Debug, Default)]
pub(crate) struct StructuralConstantTableBuilder {
    constants: BTreeMap<ConstantId, ConstantValue>,
}

impl StructuralConstantTableBuilder {
    pub(crate) fn define(
        &mut self,
        id: ConstantId,
        value: ConstantValue,
    ) -> Result<(), crate::DefinitionError> {
        match self.constants.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert(value);
                Ok(())
            }
            Entry::Occupied(_) => Err(crate::DefinitionError::AlreadyDefined),
        }
    }

    pub(crate) fn finish(self) -> StructuralConstantTable {
        StructuralConstantTable {
            constants: self.constants,
        }
    }
}
