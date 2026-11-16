//! Finite positive callable domains live in the canonical data-instance graph.
use super::{Context, DataInstanceKey, Program};
use crate::value_access::{CodeBindingError, ConcreteWitness, InstanceSchema};
use nocter_model::TypeId;
use std::{collections::BTreeSet, sync::Arc};

impl Program {
    /// Retain complete positive callable choices under one validated source descriptor.
    /// This supplies no selected alternative, value, call frame, layout or transport permission.
    /// Empty input is missing evidence; exclusion uses `instantiate_uninhabited_data` instead.
    /// # Errors
    /// Rejects empty input and every witness/descriptor mismatch rejected by single instantiation.
    pub fn instantiate_function_alternatives(
        &self,
        data: TypeId,
        alternatives: impl IntoIterator<Item = ConcreteWitness>,
    ) -> Result<DataInstanceKey, CodeBindingError> {
        let mut choices = BTreeSet::new();
        for actual in alternatives {
            let key = self.instantiate_function(data, &actual)?;
            let InstanceSchema::Function(actual) = key.schema(self)? else {
                return Err(CodeBindingError::InvalidWitness);
            };
            choices.insert(actual);
        }
        match choices.len() {
            0 => Err(CodeBindingError::InvalidWitness),
            1 => self.instantiate_function(
                data,
                choices.first().ok_or(CodeBindingError::InvalidWitness)?,
            ),
            _ => Ok(DataInstanceKey {
                data,
                context: Arc::new(Context::FunctionAlternatives(
                    choices.into_iter().collect::<Arc<[_]>>(),
                )),
            }),
        }
    }
}
