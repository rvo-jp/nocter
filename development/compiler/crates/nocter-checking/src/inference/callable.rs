use std::fmt;

use nocter_model::{
    BorrowCapability, BuiltinType, GenericParameterId, TypeId, TypeKind, TypeStore,
};

use crate::checked::{GenericArgument, GenericArguments};
use crate::expected::{ExpectedTypeError, plan_expected_type};
use crate::type_relations::{
    SubstitutionError, TypeSubstitution, TypeUnificationError, unify_type_pairs,
};
use crate::{TypePosition, TypeValidityFailure, validate_type};

pub use crate::expected::ExpectedEvidence as InferenceEvidence;

/// Order-independent constraint collector for one generic callable or construction selection.
///
/// The collector owns no type store and creates no semantic types. Callers may add receiver,
/// argument, closure, result-context, and equality constraints in any order, then solve them once.
#[derive(Debug)]
pub struct CallableInference {
    parameters: Box<[GenericParameterId]>,
    equations: Vec<(TypeId, TypeId)>,
    deferred: Vec<DeferredCompatibility>,
    result_context: Option<ResultContext>,
}

impl CallableInference {
    #[must_use]
    pub fn new(parameters: impl Into<Box<[GenericParameterId]>>) -> Self {
        Self {
            parameters: parameters.into(),
            equations: Vec::new(),
            deferred: Vec::new(),
            result_context: None,
        }
    }

    /// Adds an exact structural equation, such as a receiver or propagated type equality.
    pub fn constrain_exact(&mut self, left: TypeId, right: TypeId) {
        self.equations.push((left, right));
    }

    /// Returns every binding determined by the evidence collected so far.
    ///
    /// Unlike [`Self::finish`], this operation deliberately permits unbound inference parameters.
    /// It exists for bidirectional boundaries such as closures: ordinary arguments and the call
    /// result first determine as much of a callable contract as possible, the closure is checked
    /// under that partial contract, and its concrete signature then contributes the remaining
    /// equations. Result candidates use the same ranking as final inference.
    pub(crate) fn partial_substitution(
        &self,
        types: &mut TypeStore,
    ) -> Result<TypeSubstitution, InferenceFailure> {
        let candidates = self
            .result_context
            .map_or(Ok(vec![ResultCandidate::None]), |context| {
                context.candidates(types)
            })?;
        let mut first_failure = None;
        for candidate in candidates {
            let mut equations = self.equations.clone();
            self.append_result_equation(types, candidate, &mut equations);
            match unify_type_pairs(types, self.parameters.iter().copied(), equations) {
                Ok(bindings) => {
                    let mut substitution = TypeSubstitution::default();
                    for (parameter, ty) in bindings.iter() {
                        substitution.bind_generic(parameter, ty);
                    }
                    return Ok(substitution);
                }
                Err(error) => {
                    first_failure.get_or_insert(InferenceFailure::from(error));
                }
            }
        }
        Err(first_failure.unwrap_or(InferenceFailure::DuplicateResultContext))
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
                (
                    TypeKind::Borrow {
                        capability: BorrowCapability::Readonly,
                        referent: expected_referent,
                    },
                    InferenceEvidence::Typed(actual),
                ) if matches!(
                    types.get(actual),
                    Some(TypeKind::Borrow {
                        capability: BorrowCapability::ReadWrite,
                        ..
                    })
                ) =>
                {
                    let Some(TypeKind::Borrow {
                        referent: actual_referent,
                        ..
                    }) = types.get(actual)
                    else {
                        unreachable!("the guard established a borrowed actual type")
                    };
                    self.equations.push((*expected_referent, *actual_referent));
                    return Ok(());
                }
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

    /// Adds the expected type of the complete call result.
    ///
    /// Result inference ranks exact complete-type identity before recursively projecting optional
    /// or fallible destination payloads. The selected specialization is then revalidated by the
    /// ordinary expected-type planner, so inference and outcome injection cannot disagree.
    ///
    /// # Errors
    ///
    /// Returns an unknown-type failure or a duplicate-result-context failure.
    pub fn constrain_result_contextual(
        &mut self,
        types: &TypeStore,
        result: TypeId,
        expected: TypeId,
    ) -> Result<(), InferenceFailure> {
        if types.get(result).is_none() {
            return Err(InferenceFailure::UnknownType(result));
        }
        if types.get(expected).is_none() {
            return Err(InferenceFailure::UnknownType(expected));
        }
        if self
            .result_context
            .replace(ResultContext::Complete { result, expected })
            .is_some()
        {
            return Err(InferenceFailure::DuplicateResultContext);
        }
        Ok(())
    }

    /// Constrains the immediate payload produced after one statically known outcome layer.
    ///
    /// This is the contextual boundary used by postfix propagation. It never guesses whether an
    /// unconstrained result parameter is optional or fallible; the declared result shape must own
    /// that layer.
    ///
    /// # Errors
    ///
    /// Returns an unknown-type failure or a duplicate-result-context failure.
    pub fn constrain_outcome_payload(
        &mut self,
        types: &TypeStore,
        result: TypeId,
        expected_payload: TypeId,
    ) -> Result<(), InferenceFailure> {
        if types.get(result).is_none() {
            return Err(InferenceFailure::UnknownType(result));
        }
        if types.get(expected_payload).is_none() {
            return Err(InferenceFailure::UnknownType(expected_payload));
        }
        if self
            .result_context
            .replace(ResultContext::OutcomePayload {
                result,
                expected: expected_payload,
            })
            .is_some()
        {
            return Err(InferenceFailure::DuplicateResultContext);
        }
        Ok(())
    }

    /// Solves a unique substitution and validates every specialized generic argument.
    ///
    /// # Errors
    ///
    /// Returns a conflict for incompatible evidence, an unknown-parameter failure when evidence
    /// leaves a declared parameter undetermined, or a type-validity failure for substitutions such
    /// as `void`, `never`, and unsized data.
    pub fn finish(self, types: &mut TypeStore) -> Result<GenericArguments, InferenceFailure> {
        let candidates = self
            .result_context
            .map_or(Ok(vec![ResultCandidate::None]), |context| {
                context.candidates(types)
            })?;
        let mut first_failure = None;
        for candidate in candidates {
            match self.finish_candidate(types, candidate) {
                Ok(arguments) => return Ok(arguments),
                Err(error) => {
                    first_failure.get_or_insert(error);
                }
            }
        }
        Err(first_failure.unwrap_or(InferenceFailure::DuplicateResultContext))
    }

    fn finish_candidate(
        &self,
        types: &mut TypeStore,
        result_candidate: ResultCandidate,
    ) -> Result<GenericArguments, InferenceFailure> {
        let mut equations = self.equations.clone();
        self.append_result_equation(types, result_candidate, &mut equations);
        let bindings = unify_type_pairs(types, self.parameters.iter().copied(), equations)?;
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
        for deferred in self.deferred.iter().copied() {
            let expected = substitution.apply_type(types, deferred.expected)?;
            if !deferred.is_compatible(types, expected)? {
                return Err(InferenceFailure::ContextualMismatch {
                    expected,
                    evidence: deferred.evidence,
                });
            }
        }
        if let Some(context) = self.result_context
            && !matches!(result_candidate, ResultCandidate::None)
        {
            let (expected, result) = match context {
                ResultContext::Complete { result, expected } => {
                    (expected, substitution.apply_type(types, result)?)
                }
                ResultContext::OutcomePayload { result, expected } => {
                    let result = substitution.apply_type(types, result)?;
                    let payload = immediate_outcome_payload(types, result)?;
                    (expected, payload)
                }
            };
            if !result_context_compatible(types, expected, result)? {
                return Err(InferenceFailure::ContextualMismatch {
                    expected,
                    evidence: InferenceEvidence::Typed(result),
                });
            }
        }
        GenericArguments::new(arguments).map_err(|_| InferenceFailure::DuplicateParameter)
    }

    fn append_result_equation(
        &self,
        types: &TypeStore,
        candidate: ResultCandidate,
        equations: &mut Vec<(TypeId, TypeId)>,
    ) {
        let Some(context) = self.result_context else {
            return;
        };
        if matches!(
            types.get(context.result()),
            Some(TypeKind::Builtin(BuiltinType::Never))
        ) {
            return;
        }
        match candidate {
            ResultCandidate::None => {}
            ResultCandidate::Exact(candidate) => equations.push((context.result(), candidate)),
            ResultCandidate::BorrowWeakening {
                result_referent,
                expected_referent,
            } => equations.push((result_referent, expected_referent)),
            ResultCandidate::OutcomePayload {
                result_payload,
                expected_payload,
            } => equations.push((result_payload, expected_payload)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ResultContext {
    Complete { result: TypeId, expected: TypeId },
    OutcomePayload { result: TypeId, expected: TypeId },
}

impl ResultContext {
    const fn result(self) -> TypeId {
        match self {
            Self::Complete { result, .. } | Self::OutcomePayload { result, .. } => result,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ResultCandidate {
    None,
    Exact(TypeId),
    BorrowWeakening {
        result_referent: TypeId,
        expected_referent: TypeId,
    },
    OutcomePayload {
        result_payload: TypeId,
        expected_payload: TypeId,
    },
}

impl ResultContext {
    fn candidates(self, types: &TypeStore) -> Result<Vec<ResultCandidate>, InferenceFailure> {
        if matches!(
            types.get(self.result()),
            Some(TypeKind::Builtin(BuiltinType::Never))
        ) {
            return Ok(vec![ResultCandidate::None]);
        }
        if let Self::OutcomePayload { result, expected } = self {
            return Ok(vec![ResultCandidate::OutcomePayload {
                result_payload: immediate_outcome_payload(types, result)?,
                expected_payload: expected,
            }]);
        }
        let Self::Complete { result, expected } = self else {
            unreachable!("outcome payload context returned above")
        };
        let mut candidates = Vec::new();
        let mut current = expected;
        loop {
            candidates.push(result_candidate(types, result, current)?);
            match types
                .get(current)
                .ok_or(InferenceFailure::UnknownType(current))?
            {
                TypeKind::Optional(payload) | TypeKind::Fallible(payload) => current = *payload,
                _ => {
                    candidates.push(ResultCandidate::None);
                    return Ok(candidates);
                }
            }
        }
    }
}

fn immediate_outcome_payload(
    types: &TypeStore,
    result: TypeId,
) -> Result<TypeId, InferenceFailure> {
    match types
        .get(result)
        .ok_or(InferenceFailure::UnknownType(result))?
    {
        TypeKind::Optional(payload) | TypeKind::Fallible(payload) => Ok(*payload),
        _ => Err(InferenceFailure::ContextualMismatch {
            expected: result,
            evidence: InferenceEvidence::Typed(result),
        }),
    }
}

fn result_candidate(
    types: &TypeStore,
    result: TypeId,
    expected: TypeId,
) -> Result<ResultCandidate, InferenceFailure> {
    let result_kind = types
        .get(result)
        .ok_or(InferenceFailure::UnknownType(result))?;
    let expected_kind = types
        .get(expected)
        .ok_or(InferenceFailure::UnknownType(expected))?;
    match (result_kind, expected_kind) {
        (
            TypeKind::Borrow {
                capability: BorrowCapability::ReadWrite,
                referent: result_referent,
            },
            TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent: expected_referent,
            },
        ) => Ok(ResultCandidate::BorrowWeakening {
            result_referent: *result_referent,
            expected_referent: *expected_referent,
        }),
        _ => Ok(ResultCandidate::Exact(expected)),
    }
}

fn result_context_compatible(
    types: &TypeStore,
    expected: TypeId,
    result: TypeId,
) -> Result<bool, InferenceFailure> {
    match plan_expected_type(types, expected, InferenceEvidence::Typed(result)) {
        Ok(_) => return Ok(true),
        Err(ExpectedTypeError::UnknownType(ty)) => return Err(InferenceFailure::UnknownType(ty)),
        Err(ExpectedTypeError::Mismatch { .. }) => {}
    }
    let mut target = expected;
    loop {
        match (types.get(result), types.get(target)) {
            (
                Some(TypeKind::Borrow {
                    capability: BorrowCapability::ReadWrite,
                    referent: result_referent,
                }),
                Some(TypeKind::Borrow {
                    capability: BorrowCapability::Readonly,
                    referent: expected_referent,
                }),
            ) => return Ok(result_referent == expected_referent),
            (_, Some(TypeKind::Optional(payload) | TypeKind::Fallible(payload))) => {
                target = *payload;
            }
            (None, _) => return Err(InferenceFailure::UnknownType(result)),
            (_, None) => return Err(InferenceFailure::UnknownType(target)),
            _ => return Ok(false),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct DeferredCompatibility {
    expected: TypeId,
    evidence: InferenceEvidence,
}

impl DeferredCompatibility {
    fn is_compatible(self, types: &TypeStore, expected: TypeId) -> Result<bool, InferenceFailure> {
        match plan_expected_type(types, expected, self.evidence) {
            Ok(_) => Ok(true),
            Err(ExpectedTypeError::Mismatch { .. }) => Ok(false),
            Err(ExpectedTypeError::UnknownType(ty)) => Err(InferenceFailure::UnknownType(ty)),
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
    DuplicateResultContext,
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
            Self::DuplicateResultContext => {
                formatter.write_str("callable inference received two result contexts")
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

    #[test]
    fn result_context_prefers_complete_type_identity_before_injection() {
        let mut types = TypeStore::new();
        let (parameter, variable) = parameter(&mut types);
        let i32_type = types.builtin(BuiltinType::I32);
        let expected = types.intern(TypeKind::Optional(i32_type)).unwrap();
        let mut inference = CallableInference::new([parameter]);
        inference
            .constrain_result_contextual(&types, variable, expected)
            .unwrap();

        assert_eq!(
            inference.finish(&mut types).unwrap().get(parameter),
            Some(expected)
        );
    }

    #[test]
    fn outcome_payload_context_uses_the_declared_immediate_layer() {
        let mut types = TypeStore::new();
        let (parameter, variable) = parameter(&mut types);
        let optional = types.intern(TypeKind::Optional(variable)).unwrap();
        let i32_type = types.builtin(BuiltinType::I32);
        let mut inference = CallableInference::new([parameter]);
        inference
            .constrain_outcome_payload(&types, optional, i32_type)
            .unwrap();

        assert_eq!(
            inference.finish(&mut types).unwrap().get(parameter),
            Some(i32_type)
        );
    }

    #[test]
    fn fixed_result_uses_the_nearest_compatible_outcome_payload() {
        let mut types = TypeStore::new();
        let i32_type = types.builtin(BuiltinType::I32);
        let optional = types.intern(TypeKind::Optional(i32_type)).unwrap();
        let expected = types.intern(TypeKind::Fallible(optional)).unwrap();
        let mut inference = CallableInference::new([]);
        inference
            .constrain_result_contextual(&types, i32_type, expected)
            .unwrap();

        assert!(inference.finish(&mut types).unwrap().as_slice().is_empty());
    }

    #[test]
    fn shaped_generic_result_infers_from_the_exact_expected_shape() {
        let mut types = TypeStore::new();
        let (parameter, variable) = parameter(&mut types);
        let result = types.intern(TypeKind::Optional(variable)).unwrap();
        let i32_type = types.builtin(BuiltinType::I32);
        let expected = types.intern(TypeKind::Optional(i32_type)).unwrap();
        let mut inference = CallableInference::new([parameter]);
        inference
            .constrain_result_contextual(&types, result, expected)
            .unwrap();

        assert_eq!(
            inference.finish(&mut types).unwrap().get(parameter),
            Some(i32_type)
        );
    }

    #[test]
    fn never_result_accepts_context_without_inventing_generic_evidence() {
        let mut types = TypeStore::new();
        let expected = types
            .intern(TypeKind::Optional(types.builtin(BuiltinType::I32)))
            .unwrap();
        let mut inference = CallableInference::new([]);
        inference
            .constrain_result_contextual(&types, types.builtin(BuiltinType::Never), expected)
            .unwrap();

        assert!(inference.finish(&mut types).unwrap().as_slice().is_empty());
    }

    #[test]
    fn result_context_is_a_single_authoritative_boundary() {
        let types = TypeStore::new();
        let i32_type = types.builtin(BuiltinType::I32);
        let mut inference = CallableInference::new([]);
        inference
            .constrain_result_contextual(&types, i32_type, i32_type)
            .unwrap();

        assert!(matches!(
            inference.constrain_result_contextual(&types, i32_type, i32_type),
            Err(InferenceFailure::DuplicateResultContext)
        ));
    }
}
