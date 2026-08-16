use std::fmt;

use nocter_model::{BuiltinType, TypeId, TypeKind, TypeStore};

/// Source classification at one authoritative expected-type boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedEvidence {
    Typed(TypeId),
    Absent,
    Failure,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OutcomeLayer {
    Optional,
    Fallible,
}

/// Innermost operation selected by recursive outcome injection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedBase {
    Exact(TypeId),
    Absent(TypeId),
    Failure(TypeId),
    Diverges(TypeId),
}

/// Complete recursive-outcome plan in construction order.
///
/// `injections` are ordered from the selected base outward. `Optional` means presence injection;
/// `Fallible` means success injection. Absence and failure themselves are represented by `base`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedTypePlan {
    base: ExpectedBase,
    injections: Box<[OutcomeLayer]>,
}

impl ExpectedTypePlan {
    #[must_use]
    pub const fn base(&self) -> ExpectedBase {
        self.base
    }

    #[must_use]
    pub const fn injections(&self) -> &[OutcomeLayer] {
        &self.injections
    }
}

/// Applies the normative outer-to-inner expected-type rule and returns an inner-to-outer plan.
///
/// Exact type identity is tested before opening each outcome layer. This preserves an existing
/// complete outcome value instead of adding another layer. This function selects no coercion or
/// integer-literal specialization; callers perform those operations before or at the mismatched
/// leaf and invoke this planner with the selected exact source type.
///
/// # Errors
///
/// Returns [`ExpectedTypeError::Mismatch`] when no exact leaf or contextual tag exists, or
/// [`ExpectedTypeError::UnknownType`] for an inconsistent type store.
pub fn plan_expected_type(
    types: &TypeStore,
    expected: TypeId,
    evidence: ExpectedEvidence,
) -> Result<ExpectedTypePlan, ExpectedTypeError> {
    if let ExpectedEvidence::Typed(actual) = evidence
        && actual == types.builtin(BuiltinType::Never)
    {
        return Ok(ExpectedTypePlan {
            base: ExpectedBase::Diverges(actual),
            injections: Box::new([]),
        });
    }

    let mut current = expected;
    let mut outer = Vec::new();
    loop {
        if evidence == ExpectedEvidence::Typed(current) {
            return Ok(finish(ExpectedBase::Exact(current), outer));
        }
        let kind = types
            .get(current)
            .ok_or(ExpectedTypeError::UnknownType(current))?;
        match (kind, evidence) {
            (TypeKind::Optional(_), ExpectedEvidence::Absent) => {
                return Ok(finish(ExpectedBase::Absent(current), outer));
            }
            (TypeKind::Fallible(_), ExpectedEvidence::Failure) => {
                return Ok(finish(ExpectedBase::Failure(current), outer));
            }
            (TypeKind::Fallible(_), ExpectedEvidence::Typed(actual))
                if actual == types.builtin(BuiltinType::Error) =>
            {
                return Ok(finish(ExpectedBase::Failure(current), outer));
            }
            (TypeKind::Optional(payload), _) => {
                outer.push(OutcomeLayer::Optional);
                current = *payload;
            }
            (TypeKind::Fallible(payload), _) => {
                outer.push(OutcomeLayer::Fallible);
                current = *payload;
            }
            _ => {
                return Err(ExpectedTypeError::Mismatch {
                    expected: current,
                    evidence,
                });
            }
        }
    }
}

fn finish(base: ExpectedBase, mut outer: Vec<OutcomeLayer>) -> ExpectedTypePlan {
    outer.reverse();
    ExpectedTypePlan {
        base,
        injections: outer.into_boxed_slice(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedTypeError {
    UnknownType(TypeId),
    Mismatch {
        expected: TypeId,
        evidence: ExpectedEvidence,
    },
}

impl fmt::Display for ExpectedTypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownType(ty) => write!(formatter, "unknown expected type {ty:?}"),
            Self::Mismatch { expected, evidence } => write!(
                formatter,
                "expected {expected:?} cannot consume contextual evidence {evidence:?}"
            ),
        }
    }
}

impl std::error::Error for ExpectedTypeError {}

#[cfg(test)]
mod tests {
    use nocter_model::{BuiltinType, TypeKind, TypeStore};

    use super::{ExpectedBase, ExpectedEvidence, OutcomeLayer, plan_expected_type};

    #[test]
    fn exact_complete_outcome_precedes_recursive_injection() {
        let mut types = TypeStore::new();
        let i32_type = types.builtin(BuiltinType::I32);
        let optional = types.intern(TypeKind::Optional(i32_type)).unwrap();

        let plan = plan_expected_type(&types, optional, ExpectedEvidence::Typed(optional)).unwrap();
        assert_eq!(plan.base(), ExpectedBase::Exact(optional));
        assert!(plan.injections().is_empty());
    }

    #[test]
    fn nested_payload_injections_are_returned_inside_out() {
        let mut types = TypeStore::new();
        let i32_type = types.builtin(BuiltinType::I32);
        let fallible = types.intern(TypeKind::Fallible(i32_type)).unwrap();
        let optional_fallible = types.intern(TypeKind::Optional(fallible)).unwrap();

        let plan = plan_expected_type(&types, optional_fallible, ExpectedEvidence::Typed(i32_type))
            .unwrap();
        assert_eq!(plan.base(), ExpectedBase::Exact(i32_type));
        assert_eq!(
            plan.injections(),
            &[OutcomeLayer::Fallible, OutcomeLayer::Optional]
        );
    }

    #[test]
    fn tag_selection_preserves_only_required_outer_success_layers() {
        let mut types = TypeStore::new();
        let i32_type = types.builtin(BuiltinType::I32);
        let optional = types.intern(TypeKind::Optional(i32_type)).unwrap();
        let fallible_optional = types.intern(TypeKind::Fallible(optional)).unwrap();

        let absent =
            plan_expected_type(&types, fallible_optional, ExpectedEvidence::Absent).unwrap();
        assert_eq!(absent.base(), ExpectedBase::Absent(optional));
        assert_eq!(absent.injections(), &[OutcomeLayer::Fallible]);

        let failure =
            plan_expected_type(&types, fallible_optional, ExpectedEvidence::Failure).unwrap();
        assert_eq!(failure.base(), ExpectedBase::Failure(fallible_optional));
        assert!(failure.injections().is_empty());
    }

    #[test]
    fn never_is_compatible_without_constructing_any_layer() {
        let mut types = TypeStore::new();
        let never = types.builtin(BuiltinType::Never);
        let optional = types
            .intern(TypeKind::Optional(types.builtin(BuiltinType::I32)))
            .unwrap();
        let plan = plan_expected_type(&types, optional, ExpectedEvidence::Typed(never)).unwrap();

        assert_eq!(plan.base(), ExpectedBase::Diverges(never));
        assert!(plan.injections().is_empty());
    }
}
