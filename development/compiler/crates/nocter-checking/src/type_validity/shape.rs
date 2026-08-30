use std::collections::HashSet;
use std::fmt;

use nocter_model::{BuiltinType, TypeId, TypeKind, TypeStore};

use super::rule::TypeValidityRule;

/// Semantic position of a normalized type root.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TypePosition {
    Data,
    CallableResult,
    TypeOperand,
    BorrowPointee,
    PointerPointee,
}

/// One source-language type-position violation, independent of source projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeValidityViolation {
    rule: TypeValidityRule,
    offending: TypeId,
}

impl TypeValidityViolation {
    const fn new(rule: TypeValidityRule, offending: TypeId) -> Self {
        Self { rule, offending }
    }

    #[must_use]
    pub const fn rule(self) -> TypeValidityRule {
        self.rule
    }

    #[must_use]
    pub const fn offending(self) -> TypeId {
        self.offending
    }
}

/// Invalid authored/specialized type or a type-store integrity failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeValidityFailure {
    Rule(TypeValidityViolation),
    UnknownType(TypeId),
}

impl fmt::Display for TypeValidityFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(violation) => write!(
                formatter,
                "{} for type {:?}",
                violation.rule().code(),
                violation.offending()
            ),
            Self::UnknownType(ty) => write!(formatter, "unknown type {ty:?} during validation"),
        }
    }
}

impl std::error::Error for TypeValidityFailure {}

/// Validates one normalized type in its semantic position.
///
/// This operation is iterative and may be reused after concrete generic substitution. Alias
/// expansion must already have occurred; aliases have no canonical `TypeKind` of their own.
///
/// # Errors
///
/// Returns a source-language rule for an invalid position or `UnknownType` for an inconsistent
/// store reference.
pub fn validate_type(
    types: &TypeStore,
    root: TypeId,
    position: TypePosition,
) -> Result<(), TypeValidityFailure> {
    let mut pending = vec![(root, position)];
    let mut visited = HashSet::new();
    while let Some((ty, position)) = pending.pop() {
        if !visited.insert((ty, position)) {
            continue;
        }
        let kind = types.get(ty).ok_or(TypeValidityFailure::UnknownType(ty))?;
        match kind {
            TypeKind::Builtin(BuiltinType::Void) if !permits_void(position) => {
                return rule(TypeValidityRule::VoidData, ty);
            }
            TypeKind::Builtin(BuiltinType::Never) if !permits_never(position) => {
                return rule(TypeValidityRule::NeverData, ty);
            }
            TypeKind::Builtin(BuiltinType::Str) if !permits_unsized(position) => {
                return rule(TypeValidityRule::UnsizedData, ty);
            }
            TypeKind::Builtin(_) | TypeKind::GenericParameter(_) | TypeKind::InterfaceSelf(_) => {}
            TypeKind::Nominal { arguments, .. }
            | TypeKind::Opaque { arguments, .. }
            | TypeKind::Closure { arguments, .. } => {
                pending.extend(
                    arguments
                        .iter()
                        .rev()
                        .copied()
                        .map(|argument| (argument, TypePosition::Data)),
                );
            }
            TypeKind::AssociatedProjection { base, .. } => {
                pending.push((*base, TypePosition::TypeOperand));
            }
            TypeKind::Pointer(pointee) => {
                pending.push((*pointee, TypePosition::PointerPointee));
            }
            TypeKind::Borrow { referent, .. } => {
                pending.push((*referent, TypePosition::BorrowPointee));
            }
            TypeKind::Slice(element) => {
                if !permits_unsized(position) {
                    return rule(TypeValidityRule::UnsizedData, ty);
                }
                pending.push((*element, TypePosition::Data));
            }
            TypeKind::FixedArray { element, .. } => {
                pending.push((*element, TypePosition::Data));
            }
            TypeKind::PackEntry { key, value } => {
                pending.push((*value, TypePosition::Data));
                pending.push((*key, TypePosition::Data));
            }
            TypeKind::Callable(contract) => {
                pending.push((contract.result(), TypePosition::CallableResult));
                if let Some(pack) = contract.pack() {
                    pending.push((pack.primary(), TypePosition::Data));
                    if let Some(value) = pack.value() {
                        pending.push((value, TypePosition::Data));
                    }
                }
                pending.extend(
                    contract
                        .parameters()
                        .iter()
                        .rev()
                        .copied()
                        .map(|parameter| (parameter, TypePosition::Data)),
                );
            }
            TypeKind::Optional(_) | TypeKind::Fallible(_) => {
                let outcome = outcome_payload(types, ty)?;
                if outcome.optional && outcome.payload_kind == PayloadKind::Void {
                    return rule(TypeValidityRule::OptionalVoid, ty);
                }
                if outcome.payload_kind == PayloadKind::Never {
                    return rule(TypeValidityRule::OutcomeNever, ty);
                }
                if outcome.payload_kind != PayloadKind::Void {
                    pending.push((outcome.payload, TypePosition::Data));
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PayloadKind {
    Ordinary,
    Void,
    Never,
}

struct Outcome {
    payload: TypeId,
    payload_kind: PayloadKind,
    optional: bool,
}

fn outcome_payload(types: &TypeStore, root: TypeId) -> Result<Outcome, TypeValidityFailure> {
    let mut current = root;
    let mut optional = false;
    let mut fallible = false;
    let mut layers = 0_u8;
    loop {
        let kind = types
            .get(current)
            .ok_or(TypeValidityFailure::UnknownType(current))?;
        match kind {
            TypeKind::Optional(payload) if !optional && layers < 2 => {
                optional = true;
                layers += 1;
                current = *payload;
            }
            TypeKind::Fallible(payload) if !fallible && layers < 2 => {
                fallible = true;
                layers += 1;
                current = *payload;
            }
            TypeKind::Optional(_) | TypeKind::Fallible(_) => {
                return rule(TypeValidityRule::InvalidOutcomeShape, root);
            }
            TypeKind::Builtin(BuiltinType::Void) => {
                return Ok(Outcome {
                    payload: current,
                    payload_kind: PayloadKind::Void,
                    optional,
                });
            }
            TypeKind::Builtin(BuiltinType::Never) => {
                return Ok(Outcome {
                    payload: current,
                    payload_kind: PayloadKind::Never,
                    optional,
                });
            }
            _ => {
                return Ok(Outcome {
                    payload: current,
                    payload_kind: PayloadKind::Ordinary,
                    optional,
                });
            }
        }
    }
}

const fn permits_void(position: TypePosition) -> bool {
    matches!(
        position,
        TypePosition::CallableResult | TypePosition::TypeOperand | TypePosition::PointerPointee
    )
}

const fn permits_never(position: TypePosition) -> bool {
    matches!(
        position,
        TypePosition::CallableResult | TypePosition::TypeOperand
    )
}

const fn permits_unsized(position: TypePosition) -> bool {
    matches!(
        position,
        TypePosition::TypeOperand | TypePosition::BorrowPointee | TypePosition::PointerPointee
    )
}

fn rule<T>(rule: TypeValidityRule, offending: TypeId) -> Result<T, TypeValidityFailure> {
    Err(TypeValidityFailure::Rule(TypeValidityViolation::new(
        rule, offending,
    )))
}
