//! Named/formal witness identity and structural contracts are distinct declaration-owned facts.
use super::{DataReference, PayloadSchema, Program};
use nocter_model::{
    AccessCodeId, TypeAuthority, TypeId, TypeKind, ValueFunctionOrigin, ValueInputOperation,
    ValueInvocation,
};
mod compound_inputs;
mod patterns;
mod result_components;
mod substitution;
pub use patterns::{
    FunctionDataPattern, FunctionPattern, FunctionPatternMismatch, FunctionPosition,
    FunctionStructure,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionWitness {
    pub origin: ValueFunctionOrigin,
    pub invocation: ValueInvocation,
}
impl From<AccessCodeId> for FunctionWitness {
    fn from(code: AccessCodeId) -> Self {
        Self {
            origin: ValueFunctionOrigin::Declaration(code),
            invocation: ValueInvocation::Read,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunctionInput<Data = TypeId> {
    pub data: Data,
    pub operation: ValueInputOperation,
}
/// A structural contract constrains a witness; it is not a runtime representation or data type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionConstraint<Data = TypeId> {
    pub invocation: ValueInvocation,
    pub inputs: Box<[FunctionInput<Data>]>,
    pub result: Data,
}
impl<Data: Copy> FunctionConstraint<Data> {
    pub(super) fn try_map_data<Target, Error>(
        &self,
        mut project: impl FnMut(Data) -> Result<Target, Error>,
    ) -> Result<FunctionConstraint<Target>, Error> {
        Ok(FunctionConstraint {
            invocation: self.invocation,
            inputs: self
                .inputs
                .iter()
                .map(|input| {
                    Ok(FunctionInput {
                        data: project(input.data)?,
                        operation: input.operation,
                    })
                })
                .collect::<Result<_, Error>>()?,
            result: project(self.result)?,
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionMismatch {
    Data {
        actual: TypeId,
        expected: TypeId,
    },
    Structure {
        actual: TypeId,
        expected: FunctionStructure,
    },
    NotFunction,
    Invocation {
        actual: ValueInvocation,
        expected: ValueInvocation,
    },
    ContractInvocation {
        actual: ValueInvocation,
        expected: ValueInvocation,
    },
    InputCount {
        actual: usize,
        expected: usize,
    },
    Input {
        position: usize,
        actual: FunctionInput,
        expected: FunctionInput,
    },
    Result {
        actual: TypeId,
        expected: TypeId,
    },
    Integrity,
}
/// Construction-time structural input contract; leaves resolve in the same data namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionDefinition {
    pub invocation: ValueInvocation,
    pub inputs: Box<[(ValueInputOperation, DataReference)]>,
    pub result: DataReference,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueDefinition {
    Data(DataReference),
    Function(FunctionDefinition),
}
impl From<DataReference> for ValueDefinition {
    fn from(data: DataReference) -> Self {
        Self::Data(data)
    }
}
impl From<nocter_model::BuiltinType> for ValueDefinition {
    fn from(data: nocter_model::BuiltinType) -> Self {
        Self::Data(data.into())
    }
}
impl ValueDefinition {
    pub(super) fn has_repeated_failure(&self) -> bool {
        let mut repeated = false;
        self.visit_references(|data| repeated |= data.has_repeated_failure());
        repeated
    }
    pub(super) fn visit_data(&self, mut visit: impl FnMut(&DataReference)) {
        self.visit_references(|data| data.visit_leaves(&mut visit));
    }
    pub(super) fn visit_payload_data(&self, mut visit: impl FnMut(&DataReference)) {
        self.visit_references(|data| data.visit_payload_leaves(&mut visit));
    }
    fn visit_references(&self, mut visit: impl FnMut(&DataReference)) {
        match self {
            Self::Data(data) => visit(data),
            Self::Function(contract) => {
                for (_, data) in &contract.inputs {
                    visit(data);
                }
                visit(&contract.result);
            }
        }
    }
}
/// Intern the declared view and exactly its permitted adapters. Witness keys are type leaves;
/// signature, identity-domain and representation validation remain with the publishing owner.
pub(super) fn intern_views(
    types: &mut nocter_model::TypeTransaction,
    origin: ValueFunctionOrigin,
    declared: ValueInvocation,
) -> TypeId {
    let mut selected = None;
    for invocation in [
        ValueInvocation::Read,
        ValueInvocation::Edit,
        ValueInvocation::Take,
    ] {
        if declared.can_supply(invocation) {
            let data = types
                .intern(TypeKind::ValueFunction { origin, invocation })
                .expect("a witness key is a type leaf");
            if invocation == declared {
                selected = Some(data);
            }
        }
    }
    selected.expect("the declared invocation supplies itself")
}

impl Program {
    /// Signature identity does not require a reserved nominal payload to be complete.
    /// This proves neither an input value nor its ABI layout or invocation availability.
    pub(super) fn signature_data_known(&self, data: TypeId) -> bool {
        self.data_schema(data).is_some() || self.record_identity(data).is_some()
    }

    /// The result template selected at this call. It constrains a result, never proves a producer.
    #[must_use]
    pub fn application_template(
        &self,
        application: nocter_model::ValueAppliedResult,
    ) -> Option<FunctionWitness> {
        self.applications.get(&application).copied()
    }
    /// Project an anonymous result through this call's frozen bindings. Named result families keep
    /// their separate application relation. No callee body or concrete environment is inspected.
    #[must_use]
    pub fn function_result(
        &self,
        witness: FunctionWitness,
        binding: impl FnOnce(nocter_model::ValueParameter) -> Option<FunctionWitness>,
    ) -> Option<TypeId> {
        let result = self.function_contract(witness)?.result;
        if let (ValueFunctionOrigin::Closure(owner), Some(PayloadSchema::Function(selected))) =
            (witness.origin, self.data_schema(result))
            && let ValueFunctionOrigin::Parameter(parameter) = selected.origin
        {
            if self.parameter_owner(parameter) != Some(nocter_model::ValueCode::Closure(owner)) {
                return None;
            }
            return self.function_data(selected.substitute(binding).ok()??);
        }
        Some(result)
    }
    #[must_use]
    pub fn parameter_owner(
        &self,
        parameter: nocter_model::ValueParameter,
    ) -> Option<nocter_model::ValueCode> {
        self.parameter_contract(parameter)?;
        Some(match parameter {
            nocter_model::ValueParameter::Declaration(id)
            | nocter_model::ValueParameter::Component {
                input: nocter_model::ValueInput::Declaration(id),
                ..
            } => nocter_model::ValueCode::Declaration(self.parameters.get(id)?.owner()),
            nocter_model::ValueParameter::Closure { owner, .. }
            | nocter_model::ValueParameter::Component {
                input: nocter_model::ValueInput::Closure { owner, .. },
                ..
            } => nocter_model::ValueCode::Closure(owner),
        })
    }
    #[must_use]
    pub fn parameter_label(&self, parameter: nocter_model::ValueParameter) -> Option<String> {
        self.parameter_contract(parameter)?;
        match parameter {
            nocter_model::ValueParameter::Declaration(id) => self
                .symbols
                .spelling(self.parameters.get(id)?.name())
                .map(str::to_owned),
            nocter_model::ValueParameter::Component {
                input: nocter_model::ValueInput::Declaration(input),
                ordinal,
            } => Some(format!(
                "callable component {} of {}",
                ordinal.checked_add(1)?,
                self.symbols.spelling(self.parameters.get(input)?.name())?
            )),
            nocter_model::ValueParameter::Component {
                input: nocter_model::ValueInput::Closure { owner, position },
                ordinal,
            } => Some(format!(
                "callable component {} of input {} of closure {} in {}",
                ordinal.checked_add(1)?,
                position.checked_add(1)?,
                owner.slot,
                self.root_label(owner.owner)?
            )),
            nocter_model::ValueParameter::Closure { owner, position } => Some(format!(
                "input {} of closure {} in {}",
                position.checked_add(1)?,
                owner.slot,
                self.root_label(owner.owner)?
            )),
        }
    }
    /// One structural signature for an already selected formal input; no payload evidence follows.
    #[must_use]
    pub fn parameter_contract(
        &self,
        parameter: nocter_model::ValueParameter,
    ) -> Option<&FunctionConstraint> {
        match parameter {
            nocter_model::ValueParameter::Declaration(id) => self.parameters.get(id)?.contract(),
            nocter_model::ValueParameter::Component {
                input: nocter_model::ValueInput::Declaration(input),
                ordinal,
            } => self.parameters.get(input)?.components.get(ordinal),
            nocter_model::ValueParameter::Closure { .. }
            | nocter_model::ValueParameter::Component {
                input: nocter_model::ValueInput::Closure { .. },
                ..
            } => self.closure_parameters.get(&parameter),
        }
    }
    /// Resolve applied signature templates only; no result producer or occurrence is selected.
    pub(super) fn contract_template(
        &self,
        mut witness: FunctionWitness,
    ) -> Option<FunctionWitness> {
        let mut seen = std::collections::BTreeSet::new();
        while let ValueFunctionOrigin::Application(application) = witness.origin {
            if !seen.insert(application) {
                return None;
            }
            let template = self.application_template(application)?;
            if !template.invocation.can_supply(witness.invocation) {
                return None;
            }
            witness.origin = template.origin;
        }
        Some(witness)
    }
    /// The contract of an already selected concrete or formal witness. No candidate search occurs.
    #[must_use]
    pub fn function_contract(&self, witness: FunctionWitness) -> Option<FunctionConstraint> {
        let witness = self.contract_template(witness)?;
        let mut contract = match witness.origin {
            ValueFunctionOrigin::Closure(id) => self.closure(id)?.contract().clone(),
            ValueFunctionOrigin::Declaration(id) => {
                let callable = self.codes.get(id)?;
                if callable.kind().is_initializer() {
                    return None;
                }
                let inputs = callable
                    .parameters()
                    .iter()
                    .map(|id| {
                        let parameter = self.parameters.get(*id)?;
                        Some(FunctionInput {
                            data: parameter.data(),
                            operation: parameter.operation(),
                        })
                    })
                    .collect::<Option<_>>()?;
                FunctionConstraint {
                    invocation: witness.invocation,
                    inputs,
                    result: callable.result(),
                }
            }
            ValueFunctionOrigin::Parameter(id) => self.parameter_contract(id)?.clone(),
            ValueFunctionOrigin::Contract(id) => self.callable_contracts.get(&id)?.clone(),
            ValueFunctionOrigin::Result(result) => self
                .codes
                .get(result.owner)?
                .result_contracts()
                .get(result.ordinal)?
                .clone(),
            ValueFunctionOrigin::Application(_) => return None,
        };
        if !contract.invocation.can_supply(witness.invocation) {
            return None;
        }
        contract.invocation = witness.invocation;
        Some(contract)
    }

    /// Validate one selected invocation adapter without reopening signature or witness selection.
    #[must_use]
    pub fn function_adapter(&self, from: TypeId, to: TypeId) -> bool {
        match (self.data_schema(from), self.data_schema(to)) {
            (Some(PayloadSchema::Function(from)), Some(PayloadSchema::Function(to))) => {
                from.origin == to.origin && from.invocation.can_supply(to.invocation)
            }
            _ => false,
        }
    }

    #[must_use]
    pub fn function_data(&self, witness: FunctionWitness) -> Option<TypeId> {
        match witness.origin {
            ValueFunctionOrigin::Closure(id) => {
                self.closure(id)?;
            }
            ValueFunctionOrigin::Declaration(id) => {
                if self.codes.get(id)?.kind().is_initializer() {
                    return None;
                }
            }
            ValueFunctionOrigin::Parameter(id) => {
                self.parameter_contract(id)?;
            }
            ValueFunctionOrigin::Contract(id) => {
                self.callable_contracts.get(&id)?;
            }
            ValueFunctionOrigin::Result(result) => {
                self.codes
                    .get(result.owner)?
                    .result_contracts()
                    .get(result.ordinal)?;
            }
            ValueFunctionOrigin::Application(application) => {
                if !self.has_type_root(application.application.owner.root()) {
                    return None;
                }
                self.function_contract(witness)?;
            }
        }
        self.types.store().identity(&TypeKind::ValueFunction {
            origin: witness.origin,
            invocation: witness.invocation,
        })
    }
    /// Freeze a permitted view after exact input/result contract matching. No body lookup occurs.
    /// # Errors
    /// Reports the exact incompatible contract component without choosing a different witness.
    pub fn constrain_function(
        &self,
        data: TypeId,
        expected: &FunctionConstraint,
    ) -> Result<TypeId, FunctionMismatch> {
        let schema = self.data_schema(data).ok_or(FunctionMismatch::Integrity)?;
        let PayloadSchema::Function(witness) = schema else {
            return Err(FunctionMismatch::NotFunction);
        };
        if !self.signature_data_known(expected.result)
            || expected
                .inputs
                .iter()
                .any(|input| !self.signature_data_known(input.data))
        {
            return Err(FunctionMismatch::Integrity);
        }
        let callable = self
            .function_contract(witness)
            .ok_or(FunctionMismatch::Integrity)?;
        if !witness.invocation.can_supply(expected.invocation) {
            return Err(FunctionMismatch::Invocation {
                actual: witness.invocation,
                expected: expected.invocation,
            });
        }
        if callable.inputs.len() != expected.inputs.len() {
            return Err(FunctionMismatch::InputCount {
                actual: callable.inputs.len(),
                expected: expected.inputs.len(),
            });
        }
        for (position, (&actual, input)) in callable
            .inputs
            .iter()
            .zip(expected.inputs.iter())
            .enumerate()
        {
            if actual != *input {
                return Err(FunctionMismatch::Input {
                    position,
                    actual,
                    expected: *input,
                });
            }
        }
        if callable.result != expected.result {
            return Err(FunctionMismatch::Result {
                actual: callable.result,
                expected: expected.result,
            });
        }
        self.function_data(FunctionWitness {
            origin: witness.origin,
            invocation: expected.invocation,
        })
        .ok_or(FunctionMismatch::Integrity)
    }
}

/// All named views belong to the header authority, independent of body annotations.
pub(super) fn publish_witnesses(
    base: &TypeAuthority,
    declarations: impl Iterator<Item = AccessCodeId>,
) -> TypeAuthority {
    let mut types = base.transaction();
    for declaration in declarations {
        for invocation in [
            ValueInvocation::Read,
            ValueInvocation::Edit,
            ValueInvocation::Take,
        ] {
            types
                .intern(TypeKind::ValueFunction {
                    origin: ValueFunctionOrigin::Declaration(declaration),
                    invocation,
                })
                .expect("a named witness has no structural type edges");
        }
    }
    types.commit(base).expect("exact declaration authority")
}
