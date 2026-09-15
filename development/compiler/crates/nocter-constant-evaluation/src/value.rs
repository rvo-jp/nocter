use nocter_model::{ConstantValue, FrozenValue};

use crate::{CompileTimeExecutionRule, CompileTimeValueType, ConstantScalarType};

/// One typed, storage-independent value carried by the compile-time executor.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CompileTimeValue {
    ty: CompileTimeValueType,
    representation: CompileTimeValueRepresentation,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum CompileTimeValueRepresentation {
    Void,
    Scalar(ConstantValue),
    Tuple(Box<[CompileTimeValue]>),
    FixedArray(Box<[CompileTimeValue]>),
}

impl CompileTimeValue {
    /// Constructs a typed scalar after proving that its representation matches the declared
    /// scalar shape.
    ///
    /// # Errors
    ///
    /// Returns `TypeMismatch` instead of admitting an ill-typed evaluator input.
    pub fn scalar(
        ty: ConstantScalarType,
        value: ConstantValue,
    ) -> Result<Self, CompileTimeExecutionRule> {
        let value = Self {
            ty: CompileTimeValueType::Scalar(ty),
            representation: CompileTimeValueRepresentation::Scalar(value),
        };
        value
            .matches_type(&value.ty)
            .then_some(value)
            .ok_or(CompileTimeExecutionRule::TypeMismatch)
    }

    #[must_use]
    pub fn ty(&self) -> &CompileTimeValueType {
        &self.ty
    }

    #[must_use]
    pub fn scalar_value(&self) -> Option<&ConstantValue> {
        match &self.representation {
            CompileTimeValueRepresentation::Scalar(value) => Some(value),
            CompileTimeValueRepresentation::Void
            | CompileTimeValueRepresentation::Tuple(_)
            | CompileTimeValueRepresentation::FixedArray(_) => None,
        }
    }

    pub(crate) fn void() -> Self {
        Self {
            ty: CompileTimeValueType::Void,
            representation: CompileTimeValueRepresentation::Void,
        }
    }

    /// Constructs a typed tuple after validating every element against its positional shape.
    ///
    /// # Errors
    ///
    /// Returns `TypeMismatch` for a non-tuple type or a mismatched element.
    pub fn tuple(
        ty: CompileTimeValueType,
        values: Vec<Self>,
    ) -> Result<Self, CompileTimeExecutionRule> {
        Self::aggregate(
            ty,
            CompileTimeValueRepresentation::Tuple(values.into_boxed_slice()),
        )
    }

    /// Constructs a typed fixed array after validating its length and element shapes.
    ///
    /// # Errors
    ///
    /// Returns `TypeMismatch` for a non-array type or a mismatched element.
    pub fn fixed_array(
        ty: CompileTimeValueType,
        values: Vec<Self>,
    ) -> Result<Self, CompileTimeExecutionRule> {
        Self::aggregate(
            ty,
            CompileTimeValueRepresentation::FixedArray(values.into_boxed_slice()),
        )
    }

    fn aggregate(
        ty: CompileTimeValueType,
        representation: CompileTimeValueRepresentation,
    ) -> Result<Self, CompileTimeExecutionRule> {
        let value = Self { ty, representation };
        value
            .matches_type(&value.ty)
            .then_some(value)
            .ok_or(CompileTimeExecutionRule::TypeMismatch)
    }

    pub(crate) fn matches_type(&self, expected: &CompileTimeValueType) -> bool {
        match (expected, &self.representation) {
            (CompileTimeValueType::Void, CompileTimeValueRepresentation::Void) => true,
            (CompileTimeValueType::Scalar(ty), CompileTimeValueRepresentation::Scalar(value)) => {
                scalar_matches(*ty, value)
            }
            (CompileTimeValueType::ReadonlyBorrow(referent), _) => self.matches_type(referent),
            (CompileTimeValueType::Tuple(types), CompileTimeValueRepresentation::Tuple(values)) => {
                types.len() == values.len()
                    && types
                        .iter()
                        .zip(values)
                        .all(|(ty, value)| value.matches_type(ty))
            }
            (
                CompileTimeValueType::FixedArray { element, length },
                CompileTimeValueRepresentation::FixedArray(values),
            ) => {
                usize::try_from(*length).ok() == Some(values.len())
                    && values.iter().all(|value| value.matches_type(element))
            }
            (
                CompileTimeValueType::Never
                | CompileTimeValueType::Void
                | CompileTimeValueType::Scalar(_)
                | CompileTimeValueType::Tuple(_)
                | CompileTimeValueType::FixedArray { .. },
                _,
            ) => false,
        }
    }

    pub(crate) fn into_type(
        mut self,
        expected: &CompileTimeValueType,
    ) -> Result<Self, CompileTimeExecutionRule> {
        if !self.matches_type(expected) {
            return Err(CompileTimeExecutionRule::TypeMismatch);
        }
        self.ty = expected.clone();
        Ok(self)
    }

    /// Converts a scalar callable result into the declaration constant domain.
    ///
    /// # Errors
    ///
    /// Returns `TypeMismatch` when the value is aggregate or void.
    pub fn into_constant(self) -> Result<ConstantValue, CompileTimeExecutionRule> {
        match self.representation {
            CompileTimeValueRepresentation::Scalar(value) => Ok(value),
            CompileTimeValueRepresentation::Void
            | CompileTimeValueRepresentation::Tuple(_)
            | CompileTimeValueRepresentation::FixedArray(_) => {
                Err(CompileTimeExecutionRule::TypeMismatch)
            }
        }
    }

    /// Converts a callable result into the recursively frozen static-value domain.
    ///
    /// # Errors
    ///
    /// Returns `TypeMismatch` for `void`; every value-producing plan shape is preserved.
    pub fn into_frozen(self) -> Result<FrozenValue, CompileTimeExecutionRule> {
        match self.representation {
            CompileTimeValueRepresentation::Scalar(value) => Ok(FrozenValue::Scalar(value)),
            CompileTimeValueRepresentation::Tuple(values) => values
                .into_vec()
                .into_iter()
                .map(Self::into_frozen)
                .collect::<Result<Vec<_>, _>>()
                .map(|values| FrozenValue::Tuple(values.into_boxed_slice())),
            CompileTimeValueRepresentation::FixedArray(values) => values
                .into_vec()
                .into_iter()
                .map(Self::into_frozen)
                .collect::<Result<Vec<_>, _>>()
                .map(|values| FrozenValue::FixedArray(values.into_boxed_slice())),
            CompileTimeValueRepresentation::Void => Err(CompileTimeExecutionRule::TypeMismatch),
        }
    }
}

fn scalar_matches(ty: ConstantScalarType, value: &ConstantValue) -> bool {
    match (ty, value) {
        (ConstantScalarType::Bool, ConstantValue::Bool(_))
        | (ConstantScalarType::Character, ConstantValue::Character(_))
        | (ConstantScalarType::Float(crate::FloatFormat::Binary32), ConstantValue::Float32(_))
        | (ConstantScalarType::Float(crate::FloatFormat::Binary64), ConstantValue::Float64(_))
        | (ConstantScalarType::Text, ConstantValue::Text(_)) => true,
        (ConstantScalarType::Integer(builtin), ConstantValue::Integer(value)) => {
            crate::support::integer_spec(builtin).is_some_and(|spec| spec.contains(*value))
        }
        _ => false,
    }
}
