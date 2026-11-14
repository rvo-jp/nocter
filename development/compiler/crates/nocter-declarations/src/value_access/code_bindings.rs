//! Canonical metadata bindings for one code frame; no body, const, layout or ownership admission.
use super::{FunctionConstraint, FunctionMismatch, FunctionWitness, PayloadSchema, Program};
use nocter_model::{ValueCode, ValueFunctionOrigin, ValueInvocation, ValueParameter};
use std::collections::BTreeMap;
mod context_debug;
mod context_order;
mod environments;
mod inputs;
pub use inputs::InputBindingError;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CallableCode {
    Declaration(nocter_model::AccessCodeId),
    Closure(nocter_model::ValueClosure),
}
#[derive(Clone)]
pub struct ConcreteWitness {
    code: CallableCode,
    invocation: ValueInvocation,
    environment: Option<super::DataInstanceKey>,
}
impl ConcreteWitness {
    #[must_use]
    pub const fn code(&self) -> ValueCode {
        match self.code {
            CallableCode::Declaration(id) => ValueCode::Declaration(id),
            CallableCode::Closure(id) => ValueCode::Closure(id),
        }
    }
    #[must_use]
    pub const fn invocation(&self) -> ValueInvocation {
        self.invocation
    }
    #[must_use]
    pub const fn witness(&self) -> FunctionWitness {
        FunctionWitness {
            origin: match self.code {
                CallableCode::Declaration(id) => ValueFunctionOrigin::Declaration(id),
                CallableCode::Closure(id) => ValueFunctionOrigin::Closure(id),
            },
            invocation: self.invocation,
        }
    }
    #[must_use]
    pub const fn environment(&self) -> Option<&super::DataInstanceKey> {
        self.environment.as_ref()
    }
    fn with_invocation(mut self, invocation: ValueInvocation) -> Self {
        self.invocation = invocation;
        self
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CodeInstanceKey {
    code: ValueCode,
    witnesses: BTreeMap<ValueParameter, ConcreteWitness>,
    environment: Option<super::DataInstanceKey>,
    inputs: BTreeMap<usize, super::DataInstanceKey>,
}
impl CodeInstanceKey {
    /// Context of a received compound value at its frozen explicit-input ordinal. This is not
    /// an input value or an ownership receipt; closed/direct callable inputs use other authorities.
    #[must_use]
    pub fn input_data(&self, index: usize) -> Option<&super::DataInstanceKey> {
        self.inputs.get(&index)
    }
    /// Whether received compound contexts participate in this frame's canonical identity.
    #[must_use]
    pub fn has_compound_input_contexts(&self) -> bool {
        !self.inputs.is_empty()
    }
    #[must_use]
    pub const fn code(&self) -> ValueCode {
        self.code
    }
    #[must_use]
    pub const fn witnesses(&self) -> &BTreeMap<ValueParameter, ConcreteWitness> {
        &self.witnesses
    }
    #[must_use]
    pub const fn environment(&self) -> Option<&super::DataInstanceKey> {
        self.environment.as_ref()
    }
    /// Substitute a selected view through this frame, without finding a different code candidate.
    /// # Errors
    /// Rejects an unbound/symbolic/foreign witness or a weaker supplied invocation view.
    pub fn resolve(
        &self,
        program: &Program,
        selected: FunctionWitness,
    ) -> Result<ConcreteWitness, CodeBindingError> {
        program.resolve_frame(self, selected)
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
        if let ValueFunctionOrigin::Parameter(parameter @ ValueParameter::Component { .. }) =
            selected.origin
            && program.parameter_owner(parameter) == Some(self.code)
        {
            // This frame has no actual compound-input receipts. Its own component remains a
            // symbolic condition, not a missing direct input binding or a concrete witness.
            return program.template_witness(selected);
        }
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
    /// This closes callable variables only; compound-input receipts require the original call
    /// bundle and are not inherited even when the selected code equals the outer code.
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
            .map(|(&parameter, &witness)| Ok((parameter, outer.resolve(program, witness)?)))
            .collect::<Result<Vec<_>, CodeBindingError>>()?;
        let environment = if self.code == outer.code {
            outer.environment.clone()
        } else if let ValueCode::Closure(code) = self.code {
            let invocation = program
                .closure(code)
                .ok_or(CodeBindingError::UnknownCode)?
                .contract()
                .invocation;
            outer
                .resolve(
                    program,
                    FunctionWitness {
                        origin: ValueFunctionOrigin::Closure(code),
                        invocation,
                    },
                )?
                .environment
        } else {
            None
        };
        program.bind_frame(self.code, bindings, environment)
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
    pub(super) fn resolve_bindings<'a>(
        &self,
        selected: FunctionWitness,
        lookup: impl Fn(ValueParameter) -> Option<&'a ConcreteWitness>,
    ) -> Result<ConcreteWitness, CodeBindingError> {
        if let ValueFunctionOrigin::Parameter(parameter) = selected.origin {
            let actual = lookup(parameter).ok_or(CodeBindingError::InvalidWitness)?;
            return self.adapt_witness(actual, selected.invocation);
        }
        if let ValueFunctionOrigin::Closure(code) = selected.origin {
            let contracts = self.environment_contracts(code)?;
            let captures = contracts
                .keys()
                .map(|&parameter| {
                    let actual = lookup(parameter).ok_or(CodeBindingError::Missing(parameter))?;
                    Ok((parameter, actual.clone()))
                })
                .collect::<Result<Vec<_>, CodeBindingError>>()?;
            return self.bind_callable_environment(selected, captures);
        }
        self.concrete_witness(selected)
    }

    /// Select a permitted invocation view without discarding captured callable context.
    /// This grants neither body capability nor ownership/representation evidence.
    /// # Errors
    /// Rejects a foreign/invalid witness or an upgrade from a weaker supplied invocation.
    pub fn adapt_witness(
        &self,
        witness: &ConcreteWitness,
        invocation: ValueInvocation,
    ) -> Result<ConcreteWitness, CodeBindingError> {
        self.validate_concrete(witness)?;
        if !witness.invocation().can_supply(invocation) {
            return Err(CodeBindingError::Contract(FunctionMismatch::Invocation {
                actual: witness.invocation(),
                expected: invocation,
            }));
        }
        Ok(witness.clone().with_invocation(invocation))
    }

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
        if let CallableCode::Closure(id) = code {
            let contracts = self.environment_contracts(id)?;
            if let Some((&parameter, _)) = contracts.first_key_value() {
                return Err(CodeBindingError::Missing(parameter));
            }
        }
        Ok(ConcreteWitness {
            code,
            invocation: witness.invocation,
            environment: None,
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
        if let ValueCode::Closure(id) = code
            && let Some((&parameter, _)) = self.environment_contracts(id)?.first_key_value()
        {
            return Err(CodeBindingError::Missing(parameter));
        }
        Ok(CodeInstanceKey {
            code,
            witnesses,
            environment: None,
            inputs: BTreeMap::new(),
        })
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
                if self.function_data(witness).is_none() {
                    return Err(CodeBindingError::InvalidWitness);
                }
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
        self.normalize_bindings(
            self.code_parameters(code)?,
            bindings,
            |witness| *witness,
            |witness, invocation| FunctionWitness {
                invocation,
                ..witness
            },
            |witness| validate(*witness),
        )
    }
    fn normalize_bindings<W>(
        &self,
        mut expected: BTreeMap<ValueParameter, &FunctionConstraint>,
        bindings: impl IntoIterator<Item = (ValueParameter, W)>,
        view: impl Fn(&W) -> FunctionWitness,
        select: impl Fn(W, ValueInvocation) -> W,
        validate: impl Fn(&W) -> Result<(), CodeBindingError>,
    ) -> Result<BTreeMap<ValueParameter, W>, CodeBindingError> {
        let mut witnesses = BTreeMap::new();
        for (parameter, actual) in bindings {
            if witnesses.contains_key(&parameter) {
                return Err(CodeBindingError::Duplicate(parameter));
            }
            let contract = expected
                .remove(&parameter)
                .ok_or(CodeBindingError::Unexpected(parameter))?;
            validate(&actual)?;
            let data = self
                .function_data(view(&actual))
                .ok_or(CodeBindingError::InvalidWitness)?;
            let selected = self
                .constrain_function(data, contract)
                .map_err(CodeBindingError::Contract)?;
            let Some(PayloadSchema::Function(selected)) = self.data_schema(selected) else {
                return Err(CodeBindingError::InvalidWitness);
            };
            witnesses.insert(parameter, select(actual, selected.invocation));
        }
        if let Some((&parameter, _)) = expected.first_key_value() {
            return Err(CodeBindingError::Missing(parameter));
        }
        Ok(witnesses)
    }
    fn environment_contracts(
        &self,
        code: nocter_model::ValueClosure,
    ) -> Result<BTreeMap<ValueParameter, FunctionConstraint>, CodeBindingError> {
        let schema = self.closure(code).ok_or(CodeBindingError::UnknownCode)?;
        let contracts = self.payload_contracts(schema.environment())?;
        for &parameter in contracts.keys() {
            if self.parameter_owner(parameter) == Some(ValueCode::Closure(code)) {
                return Err(CodeBindingError::Unexpected(parameter));
            }
        }
        Ok(contracts)
    }
    fn payload_contracts(
        &self,
        data: nocter_model::TypeId,
    ) -> Result<BTreeMap<ValueParameter, FunctionConstraint>, CodeBindingError> {
        let dependencies = self
            .payload_dependencies(data)
            .ok_or(CodeBindingError::InvalidWitness)?;
        if dependencies.has_pending_producers() {
            return Err(CodeBindingError::InvalidWitness);
        }
        let mut contracts = BTreeMap::<ValueParameter, FunctionConstraint>::new();
        for witness in dependencies.parameters() {
            let ValueFunctionOrigin::Parameter(parameter) = witness.origin else {
                return Err(CodeBindingError::InvalidWitness);
            };
            let mut contract = self
                .parameter_contract(parameter)
                .ok_or(CodeBindingError::InvalidWitness)?
                .clone();
            contract.invocation = witness.invocation;
            if let Some(previous) = contracts.get(&parameter)
                && previous.invocation.can_supply(contract.invocation)
            {
                continue;
            }
            contracts.insert(parameter, contract);
        }
        Ok(contracts)
    }
    pub(super) fn bind_payload_with(
        &self,
        data: nocter_model::TypeId,
        resolve: impl Fn(FunctionWitness) -> Result<ConcreteWitness, CodeBindingError>,
    ) -> Result<super::DataInstanceKey, CodeBindingError> {
        let contracts = self.payload_contracts(data)?;
        let bindings = contracts
            .iter()
            .map(|(&parameter, contract)| {
                let selected = resolve(FunctionWitness {
                    origin: ValueFunctionOrigin::Parameter(parameter),
                    invocation: contract.invocation,
                })?;
                Ok((parameter, selected))
            })
            .collect::<Result<Vec<_>, CodeBindingError>>()?;
        let witnesses = self.normalize_bindings(
            contracts
                .iter()
                .map(|(&parameter, contract)| (parameter, contract))
                .collect(),
            bindings,
            ConcreteWitness::witness,
            ConcreteWitness::with_invocation,
            |witness| self.validate_concrete(witness),
        )?;
        self.build_data_instance(data, |selected| {
            self.resolve_bindings(selected, |parameter| witnesses.get(&parameter))
        })
    }
    /// Bind exactly the free callable dependencies of one frozen owned environment.
    /// This metadata retains nested contexts and grants no payload, body or const admission.
    /// # Errors
    /// Rejects pending producers, missing/foreign dependencies and incompatible selected views.
    pub fn bind_callable_environment(
        &self,
        selected: FunctionWitness,
        bindings: impl IntoIterator<Item = (ValueParameter, ConcreteWitness)>,
    ) -> Result<ConcreteWitness, CodeBindingError> {
        let ValueFunctionOrigin::Closure(code) = selected.origin else {
            return Err(CodeBindingError::InvalidWitness);
        };
        if self.function_data(selected).is_none() {
            return Err(CodeBindingError::InvalidWitness);
        }
        let contracts = self.environment_contracts(code)?;
        let captures = self.normalize_bindings(
            contracts
                .iter()
                .map(|(&parameter, contract)| (parameter, contract))
                .collect(),
            bindings,
            ConcreteWitness::witness,
            ConcreteWitness::with_invocation,
            |witness| self.validate_concrete(witness),
        )?;
        let environment = self.bind_payload_with(
            self.closure(code)
                .ok_or(CodeBindingError::UnknownCode)?
                .environment(),
            |witness| self.resolve_bindings(witness, |parameter| captures.get(&parameter)),
        )?;
        self.bind_callable_data(selected, &environment)
    }
    /// Instantiate an already selected callable without dropping its captured substitutions.
    /// # Errors
    /// Rejects incomplete formals, incompatible views or an invalid captured context.
    pub fn bind_instance(
        &self,
        selected: &ConcreteWitness,
        bindings: impl IntoIterator<Item = (ValueParameter, ConcreteWitness)>,
    ) -> Result<CodeInstanceKey, CodeBindingError> {
        self.validate_concrete(selected)?;
        self.bind_frame(selected.code(), bindings, selected.environment.clone())
    }
    fn bind_frame(
        &self,
        code: ValueCode,
        bindings: impl IntoIterator<Item = (ValueParameter, ConcreteWitness)>,
        environment: Option<super::DataInstanceKey>,
    ) -> Result<CodeInstanceKey, CodeBindingError> {
        let witnesses = self.normalize_bindings(
            self.code_parameters(code)?,
            bindings,
            ConcreteWitness::witness,
            ConcreteWitness::with_invocation,
            |witness| self.validate_concrete(witness),
        )?;
        Ok(CodeInstanceKey {
            code,
            witnesses,
            environment,
            inputs: BTreeMap::new(),
        })
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
