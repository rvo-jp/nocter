//! Received compound contexts belong to the canonical frame, not to a raw-type substitution map.
use super::{
    CodeBindingError, CodeInstanceKey, ConcreteWitness, FunctionWitness, PayloadSchema, Program,
};
use crate::value_access::DataInstanceKey;
use nocter_model::{TypeId, ValueParameter};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputBindingError {
    Code(CodeBindingError),
    Missing(usize),
    Unexpected(usize),
    Duplicate(usize),
    Schema(usize),
}

impl Program {
    /// The sole classification of compound inputs requiring contextual metadata, by original
    /// explicit ordinal and source schema. This creates neither an actual call nor an input value.
    /// # Errors
    /// Rejects unavailable callable signatures or unknown input schemas.
    pub fn compound_input_contracts(
        &self,
        selected: FunctionWitness,
    ) -> Result<BTreeMap<usize, TypeId>, InputBindingError> {
        let contract = self
            .function_contract(selected)
            .ok_or(InputBindingError::Code(CodeBindingError::UnknownCode))?;
        let mut expected = BTreeMap::new();
        for (index, input) in contract.inputs.iter().enumerate() {
            let schema = self
                .data_schema(input.data)
                .ok_or(InputBindingError::Schema(index))?;
            if !matches!(schema, PayloadSchema::Function(_))
                && self.instantiate_closed_data(input.data).is_err()
            {
                expected.insert(index, input.data);
            }
        }
        Ok(expected)
    }
    /// Bind direct callable inputs and independently received compound contexts in one frame.
    /// Compound keys use frozen explicit-input ordinals, excluding the environment prefix.
    /// Supply exactly the compound inputs whose metadata is not independently closed. Direct
    /// callable inputs retain their existing witness bindings; closed inputs need no extra key.
    /// This selects specialization metadata, not a body, initialized storage or source permission.
    /// # Errors
    /// Rejects invalid code/witnesses and missing, duplicate, foreign-schema or surplus inputs.
    pub fn bind_instance_with_inputs(
        &self,
        selected: &ConcreteWitness,
        bindings: impl IntoIterator<Item = (ValueParameter, ConcreteWitness)>,
        inputs: impl IntoIterator<Item = (usize, DataInstanceKey)>,
    ) -> Result<CodeInstanceKey, InputBindingError> {
        let mut frame = self
            .bind_instance(selected, bindings)
            .map_err(InputBindingError::Code)?;
        let expected = self.compound_input_contracts(selected.witness())?;
        let mut values = BTreeMap::new();
        for (index, data) in inputs {
            let target = expected
                .get(&index)
                .ok_or(InputBindingError::Unexpected(index))?;
            if data.data() != *target || self.validate_data_instance(&data).is_err() {
                return Err(InputBindingError::Schema(index));
            }
            if values.insert(index, data).is_some() {
                return Err(InputBindingError::Duplicate(index));
            }
        }
        if let Some((&index, _)) = expected
            .iter()
            .find(|(index, _)| !values.contains_key(index))
        {
            return Err(InputBindingError::Missing(index));
        }
        frame.inputs = values;
        Ok(frame)
    }
}
