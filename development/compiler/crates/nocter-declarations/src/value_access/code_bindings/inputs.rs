//! Received compound contexts belong to the canonical frame, not to a raw-type substitution map.
use super::{
    CodeBindingError, CodeInstanceKey, ConcreteWitness, FunctionWitness, PayloadSchema, Program,
};
use crate::value_access::DataInstanceKey;
use nocter_model::{TypeId, ValueCode, ValueFunctionOrigin, ValueParameter};
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
    /// Select the anonymous destructor frame for one complete receiver contract. Its contextual
    /// representation occupies the ordinary explicit input slot and grants no callable witness.
    /// # Errors
    /// Rejects an absent destructor header, invalid receiver context or uninhabited receiver.
    pub fn bind_destructor(
        &self,
        receiver: &DataInstanceKey,
    ) -> Result<CodeInstanceKey, CodeBindingError> {
        let owner = self
            .nominal_identity(receiver.data())
            .ok_or(CodeBindingError::InvalidWitness)?;
        let destructor = self
            .destructor(owner)
            .ok_or(CodeBindingError::UnknownCode)?;
        if receiver.is_uninhabited() {
            return Err(CodeBindingError::InvalidWitness);
        }
        self.validate_data_instance(receiver)?;
        let mut frame = self.bind_code(destructor.into(), [])?;
        if !receiver.is_context_free() {
            frame.inputs.insert(0, receiver.clone());
        }
        Ok(frame)
    }

    /// Original explicit callback-input ordinals and physical formal IDs of one selected code.
    /// A nested type contract supplies neither a code owner nor an additional physical parameter.
    /// # Errors
    /// Rejects unknown code or an inconsistent lexical input contract.
    pub fn callable_input_parameters(
        &self,
        code: ValueCode,
    ) -> Result<BTreeMap<usize, ValueParameter>, CodeBindingError> {
        let mut inputs = BTreeMap::new();
        match code {
            ValueCode::Declaration(id) => {
                for (index, &parameter) in self
                    .codes
                    .get(id)
                    .ok_or(CodeBindingError::UnknownCode)?
                    .parameters()
                    .iter()
                    .enumerate()
                {
                    let declared = self
                        .parameters
                        .get(parameter)
                        .ok_or(CodeBindingError::UnknownCode)?;
                    if declared.contract().is_some() {
                        inputs.insert(index, parameter.into());
                    }
                }
            }
            ValueCode::Closure(owner) => {
                let closure = self.closure(owner).ok_or(CodeBindingError::UnknownCode)?;
                for (position, input) in closure.contract().inputs.iter().enumerate() {
                    if let Some(PayloadSchema::Function(witness)) = self.data_schema(input.data) {
                        let parameter = ValueParameter::Closure { owner, position };
                        if witness.origin != ValueFunctionOrigin::Parameter(parameter)
                            || self.parameter_contract(parameter).is_none()
                        {
                            return Err(CodeBindingError::UnknownCode);
                        }
                        inputs.insert(position, parameter);
                    }
                }
            }
            ValueCode::Expression(expression) => {
                if !self.has_type_root(nocter_model::ValueCodeRoot::Expression(expression)) {
                    return Err(CodeBindingError::UnknownCode);
                }
            }
        }
        Ok(inputs)
    }
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
            if data.is_uninhabited()
                || data.data() != *target
                || self.validate_data_instance(&data).is_err()
            {
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
