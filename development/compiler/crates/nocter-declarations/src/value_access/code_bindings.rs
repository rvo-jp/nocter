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
    ) -> Result<ConcreteWitness, CodeBindingError> {
        let selected = selected
            .substitute(|parameter| self.witnesses.get(&parameter).map(|w| w.witness()))
            .map_err(CodeBindingError::Contract)?
            .ok_or(CodeBindingError::InvalidWitness)?;
        program.concrete_witness(selected)
    }
}

/// One checked code frame with concrete or outer-parameter witness values. This is a condition
/// template, not a body-generation certificate or evidence of executable/const availability.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CodeTemplateKey {
    code: ValueCode,
    witnesses: BTreeMap<ValueParameter, FunctionWitness>,
}
impl CodeTemplateKey {
    #[must_use]
    pub const fn code(&self) -> ValueCode {
        self.code
    }
    #[must_use]
    pub const fn witnesses(&self) -> &BTreeMap<ValueParameter, FunctionWitness> {
        &self.witnesses
    }
    /// Substitute one frozen view, retaining an outer parameter as a symbolic condition.
    /// # Errors
    /// Rejects missing/nonproducer terms and incompatible invocation views.
    pub fn resolve(
        &self,
        program: &Program,
        selected: FunctionWitness,
    ) -> Result<FunctionWitness, CodeBindingError> {
        let selected = selected
            .substitute(|parameter| self.witnesses.get(&parameter).copied())
            .map_err(CodeBindingError::Contract)?
            .ok_or(CodeBindingError::InvalidWitness)?;
        program.template_witness(selected)
    }
    /// Compose free witness values through an outer symbolic frame in one substitution step.
    /// # Errors
    /// Rejects missing/nonproducer terms and incompatible substituted views or contracts.
    pub fn compose(&self, program: &Program, outer: &Self) -> Result<Self, CodeBindingError> {
        let bindings = self
            .witnesses
            .iter()
            .map(|(&parameter, &witness)| Ok((parameter, outer.resolve(program, witness)?)))
            .collect::<Result<Vec<_>, CodeBindingError>>()?;
        program.bind_template(self.code, bindings)
    }
    /// Close free witness values through the matched outer frame, without granting body capability.
    /// # Errors
    /// Rejects unbound terms and incompatible substituted views or contracts.
    pub fn close(
        &self,
        program: &Program,
        outer: &CodeInstanceKey,
    ) -> Result<CodeInstanceKey, CodeBindingError> {
        let bindings = self
            .witnesses
            .iter()
            .map(|(&parameter, &witness)| {
                Ok((parameter, outer.resolve(program, witness)?.witness()))
            })
            .collect::<Result<Vec<_>, CodeBindingError>>()?;
        program.bind_code(self.code, bindings)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodeBindingError {
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
    ) -> Result<ConcreteWitness, CodeBindingError> {
        let code = match witness.origin {
            ValueFunctionOrigin::Declaration(id) => CallableCode::Declaration(id),
            ValueFunctionOrigin::Closure(id) => CallableCode::Closure(id),
            _ => return Err(CodeBindingError::InvalidWitness),
        };
        if self.function_data(witness).is_none() {
            return Err(CodeBindingError::InvalidWitness);
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
    ) -> Result<CodeInstanceKey, CodeBindingError> {
        let witnesses = self
            .normalize_code_bindings(code, bindings, |witness| {
                self.concrete_witness(witness).map(|_| ())
            })?
            .into_iter()
            .map(|(parameter, witness)| Ok((parameter, self.concrete_witness(witness)?)))
            .collect::<Result<_, CodeBindingError>>()?;
        Ok(CodeInstanceKey { code, witnesses })
    }
    /// Build the identity condition frame for a code's own direct callable formals.
    /// # Errors
    /// Rejects code or parameter views outside this declaration authority.
    pub fn code_template(&self, code: ValueCode) -> Result<CodeTemplateKey, CodeBindingError> {
        let bindings = self
            .code_parameters(code)?
            .into_iter()
            .map(|(parameter, contract)| {
                (
                    parameter,
                    FunctionWitness {
                        origin: ValueFunctionOrigin::Parameter(parameter),
                        invocation: contract.invocation,
                    },
                )
            })
            .collect::<Vec<_>>();
        self.bind_template(code, bindings)
    }
    /// Bind exact direct formals to concrete or outer-parameter witnesses with one contract authority.
    /// # Errors
    /// Rejects missing/duplicate/foreign formals, nonproducer terms and incompatible views/signatures.
    pub fn bind_template(
        &self,
        code: ValueCode,
        bindings: impl IntoIterator<Item = (ValueParameter, FunctionWitness)>,
    ) -> Result<CodeTemplateKey, CodeBindingError> {
        let witnesses = self.normalize_code_bindings(code, bindings, |witness| {
            self.template_witness(witness).map(|_| ())
        })?;
        Ok(CodeTemplateKey { code, witnesses })
    }
    fn template_witness(
        &self,
        witness: FunctionWitness,
    ) -> Result<FunctionWitness, CodeBindingError> {
        match witness.origin {
            ValueFunctionOrigin::Declaration(_) | ValueFunctionOrigin::Closure(_) => {
                self.concrete_witness(witness)?;
            }
            ValueFunctionOrigin::Parameter(_) if self.function_data(witness).is_some() => {}
            _ => return Err(CodeBindingError::InvalidWitness),
        }
        Ok(witness)
    }
    fn normalize_code_bindings(
        &self,
        code: ValueCode,
        bindings: impl IntoIterator<Item = (ValueParameter, FunctionWitness)>,
        validate: impl Fn(FunctionWitness) -> Result<(), CodeBindingError>,
    ) -> Result<BTreeMap<ValueParameter, FunctionWitness>, CodeBindingError> {
        let mut expected = self.code_parameters(code)?;
        let mut witnesses = BTreeMap::new();
        for (parameter, actual) in bindings {
            if witnesses.contains_key(&parameter) {
                return Err(CodeBindingError::Duplicate(parameter));
            }
            let contract = expected
                .remove(&parameter)
                .ok_or(CodeBindingError::Unexpected(parameter))?;
            validate(actual)?;
            let data = self
                .function_data(actual)
                .ok_or(CodeBindingError::InvalidWitness)?;
            let selected = self
                .constrain_function(data, contract)
                .map_err(CodeBindingError::Contract)?;
            let Some(PayloadSchema::Function(selected)) = self.data_schema(selected) else {
                return Err(CodeBindingError::InvalidWitness);
            };
            witnesses.insert(parameter, selected);
        }
        if let Some((&parameter, _)) = expected.first_key_value() {
            return Err(CodeBindingError::Missing(parameter));
        }
        Ok(witnesses)
    }
    fn code_parameters(
        &self,
        code: ValueCode,
    ) -> Result<BTreeMap<ValueParameter, &FunctionConstraint>, CodeBindingError> {
        let mut expected = BTreeMap::new();
        match code {
            ValueCode::Declaration(id) => {
                for &parameter in self
                    .codes
                    .get(id)
                    .ok_or(CodeBindingError::UnknownCode)?
                    .parameters()
                {
                    let declared = self
                        .parameters
                        .get(parameter)
                        .ok_or(CodeBindingError::UnknownCode)?;
                    if let Some(contract) = declared.contract() {
                        expected.insert(parameter.into(), contract);
                    }
                }
            }
            ValueCode::Closure(owner) => {
                let closure = self.closure(owner).ok_or(CodeBindingError::UnknownCode)?;
                for (position, input) in closure.contract().inputs.iter().enumerate() {
                    if let Some(PayloadSchema::Function(witness)) = self.data_schema(input.data) {
                        let parameter = ValueParameter::Closure { owner, position };
                        if witness.origin != ValueFunctionOrigin::Parameter(parameter) {
                            return Err(CodeBindingError::UnknownCode);
                        }
                        expected.insert(
                            parameter,
                            self.parameter_contract(parameter)
                                .ok_or(CodeBindingError::UnknownCode)?,
                        );
                    }
                }
            }
            ValueCode::Expression(expression) => {
                if !self.has_type_root(nocter_model::ValueCodeRoot::Expression(expression)) {
                    return Err(CodeBindingError::UnknownCode);
                }
            }
        }
        Ok(expected)
    }
}
