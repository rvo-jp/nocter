//! Complete environment contexts are canonical; flattened free bindings are derived on demand.
use super::{
    CallableCode, CodeBindingError, CodeInstanceKey, ConcreteWitness, FunctionWitness, Program,
};
use crate::value_access::{DataInstanceKey, PayloadSchema, data_instances::Context};
use nocter_model::{TypeId, ValueFunctionOrigin as Origin, ValueParameter};
use std::collections::{BTreeMap, BTreeSet};

impl Program {
    /// Retain an exactly typed environment without merging different component occurrences.
    /// The environment key is metadata, not a source-generation or ownership certificate.
    /// # Errors
    /// Rejects non-closures, mismatched environment schemas and invalid callable dependencies.
    pub fn bind_callable_data(
        &self,
        selected: FunctionWitness,
        environment: &DataInstanceKey,
    ) -> Result<ConcreteWitness, CodeBindingError> {
        let Origin::Closure(code) = selected.origin else {
            return Err(CodeBindingError::InvalidWitness);
        };
        if self.function_data(selected).is_none()
            || self
                .closure(code)
                .ok_or(CodeBindingError::UnknownCode)?
                .environment()
                != environment.data()
        {
            return Err(CodeBindingError::InvalidWitness);
        }
        self.environment_contracts(code)?;
        let witness = ConcreteWitness {
            code: CallableCode::Closure(code),
            invocation: selected.invocation,
            environment: (!environment.is_context_free()).then(|| environment.clone()),
        };
        self.validate_concrete(&witness)?;
        Ok(witness)
    }
    pub(super) fn validate_concrete(&self, root: &ConcreteWitness) -> Result<(), CodeBindingError> {
        let mut pending = vec![root];
        let mut validation = Validation {
            program: self,
            seen: BTreeSet::new(),
            closed: BTreeMap::new(),
        };
        while let Some(witness) = pending.pop() {
            if self.function_data(witness.witness()).is_none() {
                return Err(CodeBindingError::InvalidWitness);
            }
            match (witness.code, witness.environment.as_ref()) {
                (CallableCode::Declaration(_), None) => {}
                (CallableCode::Declaration(_), Some(_)) => {
                    return Err(CodeBindingError::InvalidWitness);
                }
                (CallableCode::Closure(code), environment) => {
                    let contracts = self.environment_contracts(code)?;
                    let Some(environment) = environment else {
                        if !contracts.is_empty() {
                            return Err(CodeBindingError::InvalidWitness);
                        }
                        continue;
                    };
                    if environment.data()
                        != self
                            .closure(code)
                            .ok_or(CodeBindingError::UnknownCode)?
                            .environment()
                        || environment.is_context_free()
                    {
                        return Err(CodeBindingError::InvalidWitness);
                    }
                    validation.environment(environment, &mut pending)?;
                }
            }
        }
        Ok(())
    }
    pub(super) fn resolve_frame(
        &self,
        frame: &CodeInstanceKey,
        selected: FunctionWitness,
    ) -> Result<ConcreteWitness, CodeBindingError> {
        if let Origin::Parameter(parameter) = selected.origin
            && let Some(actual) = frame.witnesses.get(&parameter)
        {
            return self.adapt_witness(actual, selected.invocation);
        }
        let needed = match selected.origin {
            Origin::Parameter(parameter) => BTreeSet::from([parameter]),
            Origin::Closure(code) => self.environment_contracts(code)?.into_keys().collect(),
            _ => return self.concrete_witness(selected),
        };
        let captures = self.environment_bindings(frame.environment.as_ref(), &needed)?;
        self.resolve_bindings(selected, |parameter| {
            frame
                .witnesses
                .get(&parameter)
                .or_else(|| captures.get(&parameter))
        })
    }
    /// Derive only the requested free parameters. Conflicting occurrences remain unresolved;
    /// callers with an exact value/component must use that context rather than guessing a name.
    fn environment_bindings(
        &self,
        environment: Option<&DataInstanceKey>,
        needed: &BTreeSet<ValueParameter>,
    ) -> Result<BTreeMap<ValueParameter, ConcreteWitness>, CodeBindingError> {
        if needed.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut pending: Vec<_> = environment.into_iter().collect();
        let mut seen = BTreeSet::new();
        let mut bindings = BTreeMap::new();
        while let Some(key) = pending.pop() {
            if !seen.insert((key.data(), std::sync::Arc::as_ptr(&key.context))) {
                continue;
            }
            match &*key.context {
                Context::Closed => {}
                Context::Components(children) => pending.extend(children.values()),
                Context::Function(actual) => {
                    let Some(PayloadSchema::Function(wanted)) = self.data_schema(key.data()) else {
                        return Err(CodeBindingError::InvalidWitness);
                    };
                    if let Origin::Parameter(parameter) = wanted.origin {
                        if needed.contains(&parameter) {
                            self.merge_environment_binding(&mut bindings, parameter, actual)?;
                        }
                    } else {
                        pending.extend(actual.environment());
                    }
                }
            }
        }
        Ok(bindings)
    }
    fn merge_environment_binding(
        &self,
        bindings: &mut BTreeMap<ValueParameter, ConcreteWitness>,
        parameter: ValueParameter,
        actual: &ConcreteWitness,
    ) -> Result<(), CodeBindingError> {
        let Some(previous) = bindings.get(&parameter) else {
            bindings.insert(parameter, actual.clone());
            return Ok(());
        };
        if previous.invocation.can_supply(actual.invocation) {
            if self.adapt_witness(previous, actual.invocation)? != *actual {
                return Err(CodeBindingError::InvalidWitness);
            }
        } else {
            if self.adapt_witness(actual, previous.invocation)? != *previous {
                return Err(CodeBindingError::InvalidWitness);
            }
            bindings.insert(parameter, actual.clone());
        }
        Ok(())
    }
}

// Allocation identity only bounds this validation walk; it is not an instance/reuse identity.
struct Validation<'a> {
    program: &'a Program,
    seen: BTreeSet<(TypeId, *const Context)>,
    closed: BTreeMap<TypeId, bool>,
}
impl Validation<'_> {
    fn environment<'a>(
        &mut self,
        root: &'a DataInstanceKey,
        witnesses: &mut Vec<&'a ConcreteWitness>,
    ) -> Result<(), CodeBindingError> {
        let mut pending = vec![root];
        while let Some(key) = pending.pop() {
            if !self
                .seen
                .insert((key.data(), std::sync::Arc::as_ptr(&key.context)))
            {
                continue;
            }
            match &*key.context {
                Context::Closed => return Err(CodeBindingError::InvalidWitness),
                Context::Function(actual) => {
                    self.program
                        .constrain_instance_function(key.data(), actual.witness())?;
                    let Some(PayloadSchema::Function(wanted)) =
                        self.program.data_schema(key.data())
                    else {
                        return Err(CodeBindingError::InvalidWitness);
                    };
                    if self.program.concrete_witness(wanted).as_ref() == Ok(actual) {
                        return Err(CodeBindingError::InvalidWitness);
                    }
                    witnesses.push(actual);
                }
                Context::Components(children) => {
                    if children.is_empty() {
                        return Err(CodeBindingError::InvalidWitness);
                    }
                    let mut fields: BTreeMap<_, _> = self
                        .program
                        .instance_fields(key.data())?
                        .into_iter()
                        .collect();
                    for (field, child) in children {
                        if child.is_context_free() || fields.remove(field) != Some(child.data()) {
                            return Err(CodeBindingError::InvalidWitness);
                        }
                        pending.push(child);
                    }
                    for data in fields.into_values() {
                        let closed = if let Some(&closed) = self.closed.get(&data) {
                            closed
                        } else {
                            let closed = self.program.payload_contracts(data)?.is_empty();
                            self.closed.insert(data, closed);
                            closed
                        };
                        if !closed {
                            return Err(CodeBindingError::InvalidWitness);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
