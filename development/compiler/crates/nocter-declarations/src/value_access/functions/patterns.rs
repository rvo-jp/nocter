//! Structural annotation constraints retain selected witnesses at every callable node.
use super::{FunctionConstraint, FunctionInput, FunctionMismatch, Program};
use crate::value_access::PayloadSchema;
use nocter_model::TypeId;
#[cfg(test)]
mod tests;

/// A data leaf requires identity; a callable node constrains an existing witness without erasure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FunctionDataPattern {
    Data(TypeId),
    Function(Box<FunctionPattern>),
}
pub type FunctionPattern = FunctionConstraint<FunctionDataPattern>;

/// A path through the written contract, independent of source spans and concrete witness identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionPosition {
    Input(usize),
    Result,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionPatternMismatch {
    pub path: Box<[FunctionPosition]>,
    pub mismatch: FunctionMismatch,
}

impl FunctionPattern {
    /// Keep each normalized data identity exact when providing ordinary contextual information.
    #[must_use]
    pub fn exact(contract: FunctionConstraint) -> Self {
        Self {
            invocation: contract.invocation,
            inputs: contract
                .inputs
                .into_iter()
                .map(|input| FunctionInput {
                    operation: input.operation,
                    data: FunctionDataPattern::Data(input.data),
                })
                .collect(),
            result: FunctionDataPattern::Data(contract.result),
        }
    }
}
impl Program {
    /// Match a recursive annotation against one selected witness. Only its outer invocation can
    /// adapt; nested input/result contracts are invariant and retain their original identities.
    /// # Errors
    /// Reports the exact nested contract path. No witness, type or environment is constructed.
    pub fn constrain_function_pattern(
        &self,
        data: TypeId,
        expected: &FunctionPattern,
    ) -> Result<TypeId, FunctionPatternMismatch> {
        self.match_function_pattern(data, expected, false, &mut Vec::new())
    }
    fn match_function_pattern(
        &self,
        data: TypeId,
        expected: &FunctionPattern,
        nested: bool,
        path: &mut Vec<FunctionPosition>,
    ) -> Result<TypeId, FunctionPatternMismatch> {
        let failure = |mismatch| FunctionPatternMismatch {
            path: path.clone().into(),
            mismatch,
        };
        let witness = match self.data_schema(data) {
            Some(PayloadSchema::Function(witness)) => witness,
            Some(_) => return Err(failure(FunctionMismatch::NotFunction)),
            None => return Err(failure(FunctionMismatch::Integrity)),
        };
        let contract = self
            .function_contract(witness)
            .ok_or_else(|| failure(FunctionMismatch::Integrity))?;
        if nested && contract.invocation != expected.invocation {
            return Err(failure(FunctionMismatch::ContractInvocation {
                actual: contract.invocation,
                expected: expected.invocation,
            }));
        }
        if !contract.invocation.can_supply(expected.invocation) {
            return Err(failure(FunctionMismatch::Invocation {
                actual: contract.invocation,
                expected: expected.invocation,
            }));
        }
        if contract.inputs.len() != expected.inputs.len() {
            return Err(failure(FunctionMismatch::InputCount {
                actual: contract.inputs.len(),
                expected: expected.inputs.len(),
            }));
        }
        let mut inputs = Vec::with_capacity(expected.inputs.len());
        for (index, (actual, expected)) in contract
            .inputs
            .iter()
            .zip(expected.inputs.iter())
            .enumerate()
        {
            let data = self.match_pattern_data(
                actual.data,
                &expected.data,
                FunctionPosition::Input(index),
                path,
            )?;
            inputs.push(FunctionInput {
                data,
                operation: expected.operation,
            });
        }
        let result = self.match_pattern_data(
            contract.result,
            &expected.result,
            FunctionPosition::Result,
            path,
        )?;
        self.constrain_function(
            data,
            &FunctionConstraint {
                invocation: expected.invocation,
                inputs: inputs.into(),
                result,
            },
        )
        .map_err(|mismatch| FunctionPatternMismatch {
            path: path.clone().into(),
            mismatch,
        })
    }
    fn match_pattern_data(
        &self,
        actual: TypeId,
        expected: &FunctionDataPattern,
        position: FunctionPosition,
        path: &mut Vec<FunctionPosition>,
    ) -> Result<TypeId, FunctionPatternMismatch> {
        match expected {
            FunctionDataPattern::Data(data) => Ok(*data),
            FunctionDataPattern::Function(contract) => {
                path.push(position);
                let matched = self.match_function_pattern(actual, contract, true, path);
                path.pop();
                matched
            }
        }
    }
}
