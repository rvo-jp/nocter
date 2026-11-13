//! Canonical metadata bindings for one code frame; no body, const, layout or ownership admission.
use super::{FunctionConstraint, FunctionMismatch, FunctionWitness, PayloadSchema, Program};
use nocter_model::{ValueCode, ValueFunctionOrigin, ValueInvocation, ValueParameter};
use std::collections::BTreeMap;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CallableCode {
    Declaration(nocter_model::AccessCodeId),
    Closure(nocter_model::ValueClosure),
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ConcreteWitness {
    code: CallableCode,
    invocation: ValueInvocation,
}
impl ConcreteWitness {
    #[must_use]
    pub const fn code(self) -> ValueCode {
        match self.code {
            CallableCode::Declaration(id) => ValueCode::Declaration(id),
            CallableCode::Closure(id) => ValueCode::Closure(id),
        }
    }
    #[must_use]
    pub const fn invocation(self) -> ValueInvocation {
        self.invocation
    }
    #[must_use]
    pub const fn witness(self) -> FunctionWitness {
        FunctionWitness {
            origin: match self.code {
                CallableCode::Declaration(id) => ValueFunctionOrigin::Declaration(id),
                CallableCode::Closure(id) => ValueFunctionOrigin::Closure(id),
            },
            invocation: self.invocation,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CodeInstanceKey {
    code: ValueCode,
    witnesses: BTreeMap<ValueParameter, ConcreteWitness>,
}
impl CodeInstanceKey {
    #[must_use]
    pub const fn code(&self) -> ValueCode {
        self.code
    }
    #[must_use]
    pub const fn witnesses(&self) -> &BTreeMap<ValueParameter, ConcreteWitness> {
        &self.witnesses
    }
    /// Substitute a selected view through this frame, without finding a different code candidate.
    /// # Errors
    /// Rejects an unbound/symbolic/foreign witness or a weaker supplied invocation view.
    pub fn resolve(
        &self,
        program: &Program,
        selected: FunctionWitness,
    ) -> Result<ConcreteWitness, InstanceBindingError> {
        let selected = selected
            .substitute(|parameter| self.witnesses.get(&parameter).map(|w| w.witness()))
            .map_err(InstanceBindingError::Contract)?
            .ok_or(InstanceBindingError::InvalidWitness)?;
        program.concrete_witness(selected)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstanceBindingError {
    UnknownCode,
    InvalidWitness,
    Missing(ValueParameter),
    Unexpected(ValueParameter),
    Duplicate(ValueParameter),
    Contract(FunctionMismatch),
}
impl Program {
    /// Validate a concrete static witness in this authority. This grants no code-body capability.
    /// # Errors
    /// Rejects symbolic terms, initializers and code/views outside this declaration program.
    pub fn concrete_witness(
        &self,
        witness: FunctionWitness,
    ) -> Result<ConcreteWitness, InstanceBindingError> {
        let code = match witness.origin {
            ValueFunctionOrigin::Declaration(id) => CallableCode::Declaration(id),
            ValueFunctionOrigin::Closure(id) => CallableCode::Closure(id),
            _ => return Err(InstanceBindingError::InvalidWitness),
        };
        if self.function_data(witness).is_none() {
            return Err(InstanceBindingError::InvalidWitness);
        }
        Ok(ConcreteWitness {
            code,
            invocation: witness.invocation,
        })
    }
    /// Close exact direct callable-formal coverage and canonical invocation views for one frame.
    /// Root expressions/initializers have no callable role granted by this metadata constructor.
    /// # Errors
    /// Rejects unknown code, duplicate/missing/foreign formals and incompatible concrete witnesses.
    pub fn bind_code(
        &self,
        code: ValueCode,
        bindings: impl IntoIterator<Item = (ValueParameter, FunctionWitness)>,
    ) -> Result<CodeInstanceKey, InstanceBindingError> {
        let mut expected = self.instance_parameters(code)?;
        let mut witnesses = BTreeMap::new();
        for (parameter, actual) in bindings {
            if witnesses.contains_key(&parameter) {
                return Err(InstanceBindingError::Duplicate(parameter));
            }
            let contract = expected
                .remove(&parameter)
                .ok_or(InstanceBindingError::Unexpected(parameter))?;
            self.concrete_witness(actual)?;
            let data = self
                .function_data(actual)
                .ok_or(InstanceBindingError::InvalidWitness)?;
            let selected = self
                .constrain_function(data, contract)
                .map_err(InstanceBindingError::Contract)?;
            let Some(PayloadSchema::Function(selected)) = self.data_schema(selected) else {
                return Err(InstanceBindingError::InvalidWitness);
            };
            witnesses.insert(parameter, self.concrete_witness(selected)?);
        }
        if let Some((&parameter, _)) = expected.first_key_value() {
            return Err(InstanceBindingError::Missing(parameter));
        }
        Ok(CodeInstanceKey { code, witnesses })
    }
    fn instance_parameters(
        &self,
        code: ValueCode,
    ) -> Result<BTreeMap<ValueParameter, &FunctionConstraint>, InstanceBindingError> {
        let mut expected = BTreeMap::new();
        match code {
            ValueCode::Declaration(id) => {
                for &parameter in self
                    .codes
                    .get(id)
                    .ok_or(InstanceBindingError::UnknownCode)?
                    .parameters()
                {
                    let declared = self
                        .parameters
                        .get(parameter)
                        .ok_or(InstanceBindingError::UnknownCode)?;
                    if let Some(contract) = declared.contract() {
                        expected.insert(parameter.into(), contract);
                    }
                }
            }
            ValueCode::Closure(owner) => {
                let closure = self
                    .closure(owner)
                    .ok_or(InstanceBindingError::UnknownCode)?;
                for (position, input) in closure.contract().inputs.iter().enumerate() {
                    if let Some(PayloadSchema::Function(witness)) = self.data_schema(input.data) {
                        let parameter = ValueParameter::Closure { owner, position };
                        if witness.origin != ValueFunctionOrigin::Parameter(parameter) {
                            return Err(InstanceBindingError::UnknownCode);
                        }
                        expected.insert(
                            parameter,
                            self.parameter_contract(parameter)
                                .ok_or(InstanceBindingError::UnknownCode)?,
                        );
                    }
                }
            }
            ValueCode::Expression(expression) => {
                if !self.has_type_root(nocter_model::ValueCodeRoot::Expression(expression)) {
                    return Err(InstanceBindingError::UnknownCode);
                }
            }
        }
        Ok(expected)
    }
}
