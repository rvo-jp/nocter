use std::fmt;

use nocter_model::{BuiltinType, GenericParameterId, TypeId, TypeKind, TypeStore};

use crate::checked::{GenericArgument, GenericArguments};
use crate::type_relations::{
    SubstitutionError, TypeSubstitution, TypeUnificationError, unify_type_pairs,
};
use crate::{TypePosition, TypeValidityFailure, validate_type};

/// Type-producing classifications that participate in contextual generic inference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InferenceEvidence {
    /// An ordinary expression with a normalized type.
    Typed(TypeId),
    /// The contextual `none` tag. It never determines an unknown payload.
    Absent,
    /// An `error` value already classified as a contextual failure tag.
    Failure,
}

/// Order-independent constraint collector for one generic callable or construction selection.
///
/// The collector owns no type store and creates no semantic types. Callers may add receiver,
/// argument, closure, result-context, and equality constraints in any order, then solve them once.
#[derive(Debug)]
pub struct CallableInference {
    parameters: Box<[GenericParameterId]>,
    equations: Vec<(TypeId, TypeId)>,
    deferred: Vec<DeferredCompatibility>,
}

impl CallableInference {
    #[must_use]
    pub fn new(parameters: impl Into<Box<[GenericParameterId]>>) -> Self {
        Self {
            parameters: parameters.into(),
            equations: Vec::new(),
            deferred: Vec::new(),
        }
    }

    /// Adds an exact structural equation, such as a receiver or propagated type equality.
    pub fn constrain_exact(&mut self, left: TypeId, right: TypeId) {
        self.equations.push((left, right));
    }

    /// Adds one authoritative expected-type boundary.
    ///
    /// Known optional and fallible layers are projected before a payload equation is recorded.
    /// Matching source outcome layers remain exact. `none`, contextual failure, `never`, and
    /// `void` add no generic payload equation and are checked only after other evidence solves the
    /// substitution.
    ///
    /// # Errors
    ///
    /// Returns [`InferenceFailure::UnknownType`] when either input does not belong to `types`.
    pub fn constrain_contextual(
        &mut self,
        types: &TypeStore,
        expected: TypeId,
        evidence: InferenceEvidence,
    ) -> Result<(), InferenceFailure> {
        let mut projected = expected;
        if let InferenceEvidence::Typed(actual) = evidence {
            let actual_kind = types
                .get(actual)
                .ok_or(InferenceFailure::UnknownType(actual))?;
            if matches!(
                actual_kind,
                TypeKind::Builtin(BuiltinType::Never | BuiltinType::Void)
            ) {
                self.deferred
                    .push(DeferredCompatibility { expected, evidence });
                return Ok(());
            }
        }

        loop {
            let expected_kind = types
                .get(projected)
                .ok_or(InferenceFailure::UnknownType(projected))?;
            match (expected_kind, evidence) {
                (TypeKind::Optional(_), InferenceEvidence::Absent)
                | (TypeKind::Fallible(_), InferenceEvidence::Failure) => {
                    self.deferred
                        .push(DeferredCompatibility { expected, evidence });
                    return Ok(());
                }
                (TypeKind::Fallible(_), InferenceEvidence::Typed(actual))
                    if actual == types.builtin(BuiltinType::Error) =>
                {
                    self.deferred
                        .push(DeferredCompatibility { expected, evidence });
                    return Ok(());
                }
                (TypeKind::Optional(_), InferenceEvidence::Typed(actual))
                    if matches!(types.get(actual), Some(TypeKind::Optional(_))) =>
                {
                    self.equations.push((projected, actual));
                    return Ok(());
                }
                (TypeKind::Fallible(_), InferenceEvidence::Typed(actual))
                    if matches!(types.get(actual), Some(TypeKind::Fallible(_))) =>
                {
                    self.equations.push((projected, actual));
                    return Ok(());
                }
                (TypeKind::Optional(payload), InferenceEvidence::Typed(actual))
                    if !matches!(types.get(actual), Some(TypeKind::Optional(_))) =>
                {
                    projected = *payload;
                }
                (TypeKind::Fallible(payload), InferenceEvidence::Typed(actual))
                    if actual != types.builtin(BuiltinType::Error)
                        && !matches!(types.get(actual), Some(TypeKind::Fallible(_))) =>
                {
                    projected = *payload;
                }
                (TypeKind::Optional(payload) | TypeKind::Fallible(payload), _) => {
                    projected = *payload;
                }
                (_, InferenceEvidence::Typed(actual)) => {
                    self.equations.push((projected, actual));
                    return Ok(());
                }
                (_, InferenceEvidence::Absent | InferenceEvidence::Failure) => {
                    self.deferred
                        .push(DeferredCompatibility { expected, evidence });
                    return Ok(());
                }
            }
        }
    }

    /// Solves a unique substitution and validates every specialized generic argument.
    ///
    /// # Errors
    ///
    /// Returns a conflict for incompatible evidence, an unknown-parameter failure when evidence
    /// leaves a declared parameter undetermined, or a type-validity failure for substitutions such
    /// as `void`, `never`, and unsized data.
    pub fn finish(self, types: &mut TypeStore) -> Result<GenericArguments, InferenceFailure> {
        let bindings = unify_type_pairs(types, self.parameters.iter().copied(), self.equations)?;
        let mut substitution = TypeSubstitution::default();
        for (parameter, ty) in bindings.iter() {
            substitution.bind_generic(parameter, ty);
        }

        let mut arguments = Vec::with_capacity(self.parameters.len());
        for parameter in self.parameters.iter().copied() {
            let bound = bindings
                .get(parameter)
                .ok_or(InferenceFailure::UnknownParameter(parameter))?;
            let ty = substitution.apply_type(types, bound)?;
            validate_type(types, ty, TypePosition::Data)?;
            arguments.push(GenericArgument::new(parameter, ty));
        }
        for deferred in self.deferred {
            let expected = substitution.apply_type(types, deferred.expected)?;
            if !deferred.is_compatible(types, expected)? {
                return Err(InferenceFailure::ContextualMismatch {
                    expected,
                    evidence: deferred.evidence,
                });
            }
        }
        GenericArguments::new(arguments).map_err(|_| InferenceFailure::DuplicateParameter)
    }
}

#[derive(Clone, Copy, Debug)]
struct DeferredCompatibility {
    expected: TypeId,
    evidence: InferenceEvidence,
}

impl DeferredCompatibility {
    fn is_compatible(self, types: &TypeStore, expected: TypeId) -> Result<bool, InferenceFailure> {
        let mut current = expected;
        loop {
            let kind = types
                .get(current)
                .ok_or(InferenceFailure::UnknownType(current))?;
            match (self.evidence, kind) {
                (InferenceEvidence::Absent, TypeKind::Optional(_))
                | (InferenceEvidence::Failure, TypeKind::Fallible(_)) => return Ok(true),
                (
                    InferenceEvidence::Absent | InferenceEvidence::Failure,
                    TypeKind::Optional(payload) | TypeKind::Fallible(payload),
                ) => current = *payload,
                (InferenceEvidence::Typed(actual), _)
                    if actual == types.builtin(BuiltinType::Never) =>
                {
                    return Ok(true);
                }
                (InferenceEvidence::Typed(actual), TypeKind::Builtin(BuiltinType::Void)) => {
                    return Ok(actual == types.builtin(BuiltinType::Void));
                }
                (InferenceEvidence::Typed(actual), TypeKind::Fallible(payload)) => {
                    if actual == types.builtin(BuiltinType::Error) {
                        return Ok(true);
                    }
                    current = *payload;
                }
                _ => return Ok(false),
            }
        }
    }
}

#[derive(Debug)]
pub enum InferenceFailure {
    UnknownType(TypeId),
    Conflict {
        left: TypeId,
        right: TypeId,
    },
    RecursiveBinding {
        parameter: GenericParameterId,
        replacement: TypeId,
    },
    UnknownParameter(GenericParameterId),
    ContextualMismatch {
        expected: TypeId,
        evidence: InferenceEvidence,
    },
    InvalidSubstitution(SubstitutionError),
    InvalidArgument(TypeValidityFailure),
    DuplicateParameter,
}

impl From<TypeUnificationError> for InferenceFailure {
    fn from(error: TypeUnificationError) -> Self {
        match error {
            TypeUnificationError::UnknownType(ty) => Self::UnknownType(ty),
            TypeUnificationError::Conflict(conflict) => Self::Conflict {
                left: conflict.left(),
                right: conflict.right(),
            },
            TypeUnificationError::RecursiveBinding {
                parameter,
                replacement,
            } => Self::RecursiveBinding {
                parameter,
                replacement,
            },
        }
    }
}

impl From<SubstitutionError> for InferenceFailure {
    fn from(error: SubstitutionError) -> Self {
        Self::InvalidSubstitution(error)
    }
}

impl From<TypeValidityFailure> for InferenceFailure {
    fn from(error: TypeValidityFailure) -> Self {
        Self::InvalidArgument(error)
    }
}

impl fmt::Display for InferenceFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownType(ty) => write!(formatter, "unknown type {ty:?} during inference"),
            Self::Conflict { left, right } => {
                write!(formatter, "inference types {left:?} and {right:?} conflict")
            }
            Self::RecursiveBinding {
                parameter,
                replacement,
            } => write!(
                formatter,
                "generic parameter {parameter:?} occurs in inferred type {replacement:?}"
            ),
            Self::UnknownParameter(parameter) => {
                write!(
                    formatter,
                    "generic parameter {parameter:?} cannot be inferred"
                )
            }
            Self::ContextualMismatch { expected, evidence } => write!(
                formatter,
                "contextual evidence {evidence:?} is incompatible with {expected:?}"
            ),
            Self::InvalidSubstitution(error) => error.fmt(formatter),
            Self::InvalidArgument(error) => error.fmt(formatter),
            Self::DuplicateParameter => {
                formatter.write_str("callable inference declared one generic parameter twice")
            }
        }
    }
}

impl std::error::Error for InferenceFailure {}

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, BuiltinType, GenericParameterId, TypeKind, TypeStore};

    use super::{CallableInference, InferenceEvidence, InferenceFailure};

    fn parameter(types: &mut TypeStore) -> (GenericParameterId, nocter_model::TypeId) {
        let mut parameters = ArenaBuilder::<GenericParameterId, _>::new();
        let parameter = parameters.insert(());
        let _ = parameters.finish();
        let ty = types.intern(TypeKind::GenericParameter(parameter)).unwrap();
        (parameter, ty)
    }

    #[test]
    fn known_outcome_shape_projects_before_payload_inference() {
        let mut types = TypeStore::new();
        let (parameter, variable) = parameter(&mut types);
        let optional = types.intern(TypeKind::Optional(variable)).unwrap();
        let i32_type = types.builtin(BuiltinType::I32);
        let mut inference = CallableInference::new([parameter]);
        inference
            .constrain_contextual(&types, optional, InferenceEvidence::Typed(i32_type))
            .unwrap();

        let arguments = inference.finish(&mut types).unwrap();
        assert_eq!(arguments.get(parameter), Some(i32_type));
    }

    #[test]
    fn complete_outcome_evidence_matches_without_adding_a_layer() {
        let mut types = TypeStore::new();
        let (parameter, variable) = parameter(&mut types);
        let pattern = types.intern(TypeKind::Optional(variable)).unwrap();
        let i32_type = types.builtin(BuiltinType::I32);
        let actual = types.intern(TypeKind::Optional(i32_type)).unwrap();
        let mut inference = CallableInference::new([parameter]);
        inference
            .constrain_contextual(&types, pattern, InferenceEvidence::Typed(actual))
            .unwrap();

        assert_eq!(
            inference.finish(&mut types).unwrap().get(parameter),
            Some(i32_type)
        );
    }

    #[test]
    fn tags_and_non_values_never_determine_a_payload() {
        let mut types = TypeStore::new();
        let (parameter, variable) = parameter(&mut types);
        let optional = types.intern(TypeKind::Optional(variable)).unwrap();
        for evidence in [
            InferenceEvidence::Absent,
            InferenceEvidence::Typed(types.builtin(BuiltinType::Never)),
            InferenceEvidence::Typed(types.builtin(BuiltinType::Void)),
        ] {
            let mut inference = CallableInference::new([parameter]);
            inference
                .constrain_contextual(&types, optional, evidence)
                .unwrap();
            assert!(matches!(
                inference.finish(&mut types),
                Err(InferenceFailure::UnknownParameter(actual)) if actual == parameter
            ));
        }
    }

    #[test]
    fn another_source_can_determine_a_tag_payload() {
        let mut types = TypeStore::new();
        let (parameter, variable) = parameter(&mut types);
        let optional = types.intern(TypeKind::Optional(variable)).unwrap();
        let i32_type = types.builtin(BuiltinType::I32);
        let mut inference = CallableInference::new([parameter]);
        inference
            .constrain_contextual(&types, optional, InferenceEvidence::Absent)
            .unwrap();
        inference
            .constrain_contextual(&types, optional, InferenceEvidence::Typed(i32_type))
            .unwrap();

        assert_eq!(
            inference.finish(&mut types).unwrap().get(parameter),
            Some(i32_type)
        );
    }

    #[test]
    fn conflicting_evidence_is_input_order_independent() {
        let mut types = TypeStore::new();
        let (parameter, variable) = parameter(&mut types);
        let i32_type = types.builtin(BuiltinType::I32);
        let u32_type = types.builtin(BuiltinType::U32);
        for evidence in [[i32_type, u32_type], [u32_type, i32_type]] {
            let mut inference = CallableInference::new([parameter]);
            for actual in evidence {
                inference
                    .constrain_contextual(&types, variable, InferenceEvidence::Typed(actual))
                    .unwrap();
            }
            assert!(matches!(
                inference.finish(&mut types),
                Err(InferenceFailure::Conflict { .. })
            ));
        }
    }

    #[test]
    fn invalid_data_substitutions_are_rejected_after_solving() {
        let mut types = TypeStore::new();
        let (parameter, variable) = parameter(&mut types);
        let mut inference = CallableInference::new([parameter]);
        inference.constrain_exact(variable, types.builtin(BuiltinType::Void));

        assert!(matches!(
            inference.finish(&mut types),
            Err(InferenceFailure::InvalidArgument(_))
        ));
    }
}
