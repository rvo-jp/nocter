//! Structural annotation constraints retain selected witnesses at every callable node.
use super::{FunctionConstraint, FunctionInput, FunctionMismatch, Program};
use crate::value_access::{AggregateForm, ParameterBinding, PayloadSchema};
use nocter_model::TypeId;
#[cfg(test)]
mod tests;

/// A data leaf requires identity; a callable node constrains an existing witness without erasure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FunctionDataPattern {
    Data(TypeId),
    Function(Box<FunctionPattern>),
    Optional(Box<Self>),
    Fallible(Box<Self>),
    Tuple(Box<[Self]>),
    Array { element: Box<Self>, length: u64 },
}

impl FunctionDataPattern {
    /// Check a callable input binding against an annotation whose callable leaves may be open.
    #[must_use]
    pub fn matches_input_binding<Name>(
        &self,
        program: &Program,
        binding: &ParameterBinding<Name>,
    ) -> bool {
        enum Data<'a> {
            Pattern(&'a FunctionDataPattern),
            Exact(TypeId),
        }
        binding.matches_tuple_shape(Data::Pattern(self), &mut |data| {
            let exact = match data {
                Data::Pattern(Self::Tuple(fields)) => {
                    return Some(
                        fields
                            .iter()
                            .map(Data::Pattern)
                            .collect::<Vec<_>>()
                            .into_iter(),
                    );
                }
                Data::Pattern(Self::Data(data)) => *data,
                Data::Exact(data) => data,
                Data::Pattern(_) => return None,
            };
            let PayloadSchema::Aggregate(tuple) = program.data_schema(exact)? else {
                return None;
            };
            let AggregateForm::Tuple(fields) = tuple.form() else {
                return None;
            };
            Some(
                fields
                    .iter()
                    .copied()
                    .map(Data::Exact)
                    .collect::<Vec<_>>()
                    .into_iter(),
            )
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionStructure {
    Optional,
    Fallible,
    Tuple(usize),
    Array(u64),
}
pub type FunctionPattern = FunctionConstraint<FunctionDataPattern>;

/// A path through the written contract, independent of source spans and concrete witness identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionPosition {
    Input(usize),
    Result,
    Component(nocter_model::ValueProjection<()>),
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
            compound => {
                path.push(position);
                let matched = self.match_compound_pattern(actual, compound, path);
                path.pop();
                matched
            }
        }
    }

    /// Match data constraints without allocating schemas or substituting their callable leaves.
    /// # Errors
    /// Reports the original structural/contract path of an incompatible or unavailable schema.
    pub fn constrain_data_pattern(
        &self,
        actual: TypeId,
        expected: &FunctionDataPattern,
    ) -> Result<TypeId, FunctionPatternMismatch> {
        let mut path = Vec::new();
        self.match_compound_pattern(actual, expected, &mut path)
    }

    fn match_compound_pattern(
        &self,
        actual: TypeId,
        expected: &FunctionDataPattern,
        path: &mut Vec<FunctionPosition>,
    ) -> Result<TypeId, FunctionPatternMismatch> {
        use nocter_model::ValueProjection;
        let failure = |mismatch| FunctionPatternMismatch {
            path: path.clone().into(),
            mismatch,
        };
        if !self.signature_data_known(actual) {
            return Err(failure(FunctionMismatch::Integrity));
        }
        let (structure, children) =
            match expected {
                FunctionDataPattern::Data(data) => {
                    if !self.signature_data_known(*data) {
                        return Err(failure(FunctionMismatch::Integrity));
                    }
                    return if actual == *data {
                        Ok(actual)
                    } else {
                        Err(failure(FunctionMismatch::Data {
                            actual,
                            expected: *data,
                        }))
                    };
                }
                FunctionDataPattern::Function(contract) => {
                    return self.match_function_pattern(actual, contract, true, path);
                }
                FunctionDataPattern::Optional(payload) => (
                    FunctionStructure::Optional,
                    match self.data_schema(actual) {
                        Some(PayloadSchema::Optional(data)) => {
                            Some(vec![(ValueProjection::Variant(true), data, &**payload)])
                        }
                        _ => None,
                    },
                ),
                FunctionDataPattern::Fallible(payload) => (
                    FunctionStructure::Fallible,
                    match self.data_schema(actual) {
                        Some(PayloadSchema::Fallible(data)) => {
                            Some(vec![(ValueProjection::Variant(true), data, &**payload)])
                        }
                        _ => None,
                    },
                ),
                FunctionDataPattern::Tuple(fields) => (
                    FunctionStructure::Tuple(fields.len()),
                    match self.data_schema(actual) {
                        Some(PayloadSchema::Aggregate(schema)) => match schema.form() {
                            crate::value_access::AggregateForm::Tuple(data)
                                if data.len() == fields.len() =>
                            {
                                Some(
                                    data.iter()
                                        .copied()
                                        .zip(fields.iter())
                                        .enumerate()
                                        .map(|(index, (data, expected))| {
                                            (ValueProjection::Tuple(index), data, expected)
                                        })
                                        .collect(),
                                )
                            }
                            _ => None,
                        },
                        _ => None,
                    },
                ),
                FunctionDataPattern::Array { element, length } => (
                    FunctionStructure::Array(*length),
                    match self.data_schema(actual) {
                        Some(PayloadSchema::Array(array)) if array.length() == *length => Some(
                            vec![(ValueProjection::Array(()), array.element(), &**element)],
                        ),
                        _ => None,
                    },
                ),
            };
        let children = children.ok_or_else(|| {
            failure(FunctionMismatch::Structure {
                actual,
                expected: structure,
            })
        })?;
        for (component, data, expected) in children {
            path.push(FunctionPosition::Component(component));
            let matched = self.match_compound_pattern(data, expected, path);
            path.pop();
            matched?;
        }
        Ok(actual)
    }
}
