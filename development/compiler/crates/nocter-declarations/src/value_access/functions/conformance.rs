//! Abstract contract binding compares signatures without merging original witness identities.
use super::{
    FunctionConstraint, FunctionMismatch, FunctionPatternMismatch, FunctionPosition,
    FunctionWitness, Program,
};
use crate::value_access::{AggregateForm, PayloadSchema};
use nocter_model::{TypeId, ValueFunctionOrigin as Origin, ValueProjection};
use std::collections::BTreeSet;

type Path = Option<usize>;
#[derive(Clone, Copy)]
enum ContractMode {
    Abstract,
    Authored,
}
struct Comparison<'a> {
    program: &'a Program,
    mode: ContractMode,
    seen: BTreeSet<(TypeId, TypeId)>,
    pending: Vec<(TypeId, TypeId, Path)>,
    paths: Vec<(Path, FunctionPosition)>,
}
impl Program {
    /// Bind to an abstract type contract while retaining the actual code and environment owner.
    /// Nested abstract signatures are invariant; concrete witness identities remain exact.
    /// # Errors
    /// Rejects an unknown contract or reports the original nested signature mismatch path.
    pub fn constrain_callable_contract(
        &self,
        data: TypeId,
        wanted: FunctionWitness,
    ) -> Result<TypeId, FunctionPatternMismatch> {
        let failure = |mismatch| FunctionPatternMismatch {
            path: Box::new([]),
            mismatch,
        };
        if !matches!(wanted.origin, Origin::Contract(_)) {
            return Err(failure(FunctionMismatch::Integrity));
        }
        let expected = self
            .function_contract(wanted)
            .ok_or_else(|| failure(FunctionMismatch::Integrity))?;
        self.constrain_signature(data, &expected, ContractMode::Abstract)
    }
    /// Match written type-contract nodes in a declaration while retaining supplied identities.
    /// Normalized formal/producer/concrete leaves stay exact; only `Contract` leaves describe
    /// signatures. This does not replace exact contextual matching or prove executable payload.
    /// # Errors
    /// Reports the original nested input/result path without constructing a witness or type.
    pub fn constrain_declared_function(
        &self,
        data: TypeId,
        expected: &FunctionConstraint,
    ) -> Result<TypeId, FunctionPatternMismatch> {
        self.constrain_signature(data, expected, ContractMode::Authored)
            .map_err(|failure| {
                // A plain outer leaf retains the established input/result diagnostic. Nested
                // authored-contract mismatches keep their full structural path instead.
                if failure.path.len() == 1
                    && matches!(failure.mismatch, FunctionMismatch::Data { .. })
                    && let Err(mismatch) = self.constrain_function(data, expected)
                {
                    return FunctionPatternMismatch {
                        path: Box::new([]),
                        mismatch,
                    };
                }
                failure
            })
    }
    fn constrain_signature(
        &self,
        data: TypeId,
        expected: &FunctionConstraint,
        mode: ContractMode,
    ) -> Result<TypeId, FunctionPatternMismatch> {
        let failure = |mismatch| FunctionPatternMismatch {
            path: Box::new([]),
            mismatch,
        };
        if !self.signature_data_known(expected.result)
            || expected
                .inputs
                .iter()
                .any(|input| !self.signature_data_known(input.data))
        {
            return Err(failure(FunctionMismatch::Integrity));
        }
        let Some(PayloadSchema::Function(actual)) = self.data_schema(data) else {
            return Err(failure(FunctionMismatch::NotFunction));
        };
        let contract = self
            .function_contract(actual)
            .ok_or_else(|| failure(FunctionMismatch::Integrity))?;
        let mut comparison = Comparison {
            program: self,
            mode,
            seen: BTreeSet::new(),
            pending: Vec::new(),
            paths: Vec::new(),
        };
        comparison.signature(&contract, expected, None, true)?;
        while let Some((actual, expected, path)) = comparison.pending.pop() {
            if comparison.seen.insert((actual, expected)) {
                comparison.data(actual, expected, path)?;
            }
        }
        self.function_data(FunctionWitness {
            origin: actual.origin,
            invocation: expected.invocation,
        })
        .ok_or_else(|| failure(FunctionMismatch::Integrity))
    }
}
impl Comparison<'_> {
    fn error(&self, mut path: Path, mismatch: FunctionMismatch) -> FunctionPatternMismatch {
        let mut positions = Vec::new();
        while let Some(index) = path {
            let (parent, position) = self.paths[index];
            positions.push(position);
            path = parent;
        }
        positions.reverse();
        FunctionPatternMismatch {
            path: positions.into(),
            mismatch,
        }
    }
    fn push(&mut self, actual: TypeId, expected: TypeId, path: Path, position: FunctionPosition) {
        let index = self.paths.len();
        self.paths.push((path, position));
        self.pending.push((actual, expected, Some(index)));
    }
    fn signature(
        &mut self,
        actual: &FunctionConstraint,
        expected: &FunctionConstraint,
        path: Path,
        outer: bool,
    ) -> Result<(), FunctionPatternMismatch> {
        if outer && !actual.invocation.can_supply(expected.invocation) {
            return Err(self.error(
                path,
                FunctionMismatch::Invocation {
                    actual: actual.invocation,
                    expected: expected.invocation,
                },
            ));
        }
        if !outer && actual.invocation != expected.invocation {
            return Err(self.error(
                path,
                FunctionMismatch::ContractInvocation {
                    actual: actual.invocation,
                    expected: expected.invocation,
                },
            ));
        }
        if actual.inputs.len() != expected.inputs.len() {
            return Err(self.error(
                path,
                FunctionMismatch::InputCount {
                    actual: actual.inputs.len(),
                    expected: expected.inputs.len(),
                },
            ));
        }
        for (position, (&actual, &expected)) in
            actual.inputs.iter().zip(expected.inputs.iter()).enumerate()
        {
            if actual.operation != expected.operation {
                return Err(self.error(
                    path,
                    FunctionMismatch::Input {
                        position,
                        actual,
                        expected,
                    },
                ));
            }
        }
        self.push(
            actual.result,
            expected.result,
            path,
            FunctionPosition::Result,
        );
        for (position, (actual, expected)) in actual
            .inputs
            .iter()
            .zip(expected.inputs.iter())
            .enumerate()
            .rev()
        {
            self.push(
                actual.data,
                expected.data,
                path,
                FunctionPosition::Input(position),
            );
        }
        Ok(())
    }
    fn data(
        &mut self,
        actual: TypeId,
        expected: TypeId,
        path: Path,
    ) -> Result<(), FunctionPatternMismatch> {
        let mismatch = || self.error(path, FunctionMismatch::Data { actual, expected });
        if self.program.nominal_identity(actual).is_some()
            || self.program.nominal_identity(expected).is_some()
        {
            return if actual == expected {
                Ok(())
            } else {
                Err(mismatch())
            };
        }
        let left = self
            .program
            .data_schema(actual)
            .ok_or_else(|| self.error(path, FunctionMismatch::Integrity))?;
        let right = self
            .program
            .data_schema(expected)
            .ok_or_else(|| self.error(path, FunctionMismatch::Integrity))?;
        match (left, right) {
            (PayloadSchema::Builtin(left), PayloadSchema::Builtin(right)) if left == right => {}
            (PayloadSchema::Function(left), PayloadSchema::Function(right)) => {
                let structural = match self.mode {
                    ContractMode::Abstract => {
                        abstract_origin(left.origin) && abstract_origin(right.origin)
                    }
                    ContractMode::Authored => matches!(right.origin, Origin::Contract(_)),
                };
                if !structural && left != right {
                    return Err(mismatch());
                }
                let actual = self
                    .program
                    .function_contract(left)
                    .ok_or_else(|| self.error(path, FunctionMismatch::Integrity))?;
                let expected = self
                    .program
                    .function_contract(right)
                    .ok_or_else(|| self.error(path, FunctionMismatch::Integrity))?;
                self.signature(&actual, &expected, path, false)?;
            }
            (PayloadSchema::Optional(left), PayloadSchema::Optional(right))
            | (PayloadSchema::Fallible(left), PayloadSchema::Fallible(right)) => self.push(
                left,
                right,
                path,
                FunctionPosition::Component(ValueProjection::Variant(true)),
            ),
            (PayloadSchema::Array(left), PayloadSchema::Array(right))
                if left.length() == right.length() =>
            {
                self.push(
                    left.element(),
                    right.element(),
                    path,
                    FunctionPosition::Component(ValueProjection::Array(())),
                );
            }
            (PayloadSchema::Aggregate(left), PayloadSchema::Aggregate(right)) => {
                let (AggregateForm::Tuple(left), AggregateForm::Tuple(right)) =
                    (left.form(), right.form())
                else {
                    return Err(mismatch());
                };
                if left.len() != right.len() {
                    return Err(mismatch());
                }
                for (index, (&actual, &expected)) in left.iter().zip(right).enumerate().rev() {
                    self.push(
                        actual,
                        expected,
                        path,
                        FunctionPosition::Component(ValueProjection::Tuple(index)),
                    );
                }
            }
            _ => return Err(mismatch()),
        }
        Ok(())
    }
}
fn abstract_origin(origin: Origin) -> bool {
    matches!(
        origin,
        Origin::Parameter(_) | Origin::Contract(_) | Origin::Result(_) | Origin::Application(_)
    )
}
