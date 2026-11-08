//! Named/formal witness identity and structural contracts are distinct declaration-owned facts.
use super::{DataReference, PayloadSchema, Program};
use nocter_model::{
    AccessCallableId, TypeAuthority, TypeId, TypeKind, ValueFunctionOrigin, ValueInputOperation,
    ValueInvocation,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunctionWitness {
    pub origin: ValueFunctionOrigin,
    pub invocation: ValueInvocation,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunctionInput {
    pub data: TypeId,
    pub operation: ValueInputOperation,
}
/// A structural contract constrains a witness; it is not a runtime representation or data type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionConstraint {
    pub invocation: ValueInvocation,
    pub inputs: Box<[FunctionInput]>,
    pub result: TypeId,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionMismatch {
    NotFunction,
    Invocation {
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
pub enum ParameterData {
    Data(DataReference),
    Function(FunctionDefinition),
}
impl From<DataReference> for ParameterData {
    fn from(data: DataReference) -> Self {
        Self::Data(data)
    }
}
impl From<nocter_model::BuiltinType> for ParameterData {
    fn from(data: nocter_model::BuiltinType) -> Self {
        Self::Data(data.into())
    }
}
impl ParameterData {
    pub(super) fn visit_data(&self, mut visit: impl FnMut(DataReference)) {
        match self {
            Self::Data(data) => visit(*data),
            Self::Function(contract) => {
                for (_, data) in &contract.inputs {
                    visit(*data);
                }
                visit(contract.result);
            }
        }
    }
}
impl Program {
    /// The contract of an already selected concrete or formal witness. No candidate search occurs.
    #[must_use]
    pub fn function_contract(&self, witness: FunctionWitness) -> Option<FunctionConstraint> {
        let mut contract = match witness.origin {
            ValueFunctionOrigin::Declaration(id) => {
                let callable = self.callables.get(id)?;
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
            ValueFunctionOrigin::Parameter(id) => self.parameters.get(id)?.contract()?.clone(),
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
            ValueFunctionOrigin::Declaration(id) => {
                self.callables.get(id)?;
            }
            ValueFunctionOrigin::Parameter(id) => {
                self.parameters.get(id)?.contract()?;
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
        if self.data_schema(expected.result).is_none()
            || expected
                .inputs
                .iter()
                .any(|input| self.data_schema(input.data).is_none())
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
    declarations: impl Iterator<Item = AccessCallableId>,
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
