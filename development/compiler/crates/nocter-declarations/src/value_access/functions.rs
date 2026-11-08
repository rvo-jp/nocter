//! Named code identity and structural constraints are distinct, declaration-owned facts.
use super::{PayloadSchema, Program};
use nocter_model::{
    AccessCallableId, TypeAuthority, TypeId, TypeKind, ValueInputOperation, ValueInvocation,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunctionWitness {
    pub declaration: AccessCallableId,
    pub invocation: ValueInvocation,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunctionInput {
    pub data: TypeId,
    pub operation: ValueInputOperation,
}
/// An annotation constrains a concrete witness; it is not a runtime representation or data type.
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
impl Program {
    /// Validate one selected invocation adapter without reopening signature or witness selection.
    #[must_use]
    pub fn function_adapter(&self, from: TypeId, to: TypeId) -> bool {
        match (self.data_schema(from), self.data_schema(to)) {
            (Some(PayloadSchema::Function(from)), Some(PayloadSchema::Function(to))) => {
                from.declaration == to.declaration && from.invocation.can_supply(to.invocation)
            }
            _ => false,
        }
    }

    #[must_use]
    pub fn function_data(&self, witness: FunctionWitness) -> Option<TypeId> {
        self.callables.get(witness.declaration)?;
        self.types.store().identity(&TypeKind::ValueFunction {
            declaration: witness.declaration,
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
            .callables
            .get(witness.declaration)
            .ok_or(FunctionMismatch::Integrity)?;
        if !witness.invocation.can_supply(expected.invocation) {
            return Err(FunctionMismatch::Invocation {
                actual: witness.invocation,
                expected: expected.invocation,
            });
        }
        if callable.parameters().len() != expected.inputs.len() {
            return Err(FunctionMismatch::InputCount {
                actual: callable.parameters().len(),
                expected: expected.inputs.len(),
            });
        }
        for (position, (parameter, input)) in callable
            .parameters()
            .iter()
            .zip(expected.inputs.iter())
            .enumerate()
        {
            let parameter = self
                .parameters
                .get(*parameter)
                .ok_or(FunctionMismatch::Integrity)?;
            let actual = FunctionInput {
                data: parameter.data(),
                operation: parameter.operation(),
            };
            if actual != *input {
                return Err(FunctionMismatch::Input {
                    position,
                    actual,
                    expected: *input,
                });
            }
        }
        if callable.result() != expected.result {
            return Err(FunctionMismatch::Result {
                actual: callable.result(),
                expected: expected.result,
            });
        }
        self.function_data(FunctionWitness {
            declaration: witness.declaration,
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
                    declaration,
                    invocation,
                })
                .expect("a named witness has no structural type edges");
        }
    }
    types.commit(base).expect("exact declaration authority")
}
