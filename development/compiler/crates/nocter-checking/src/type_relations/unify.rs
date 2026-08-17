use std::collections::{HashMap, HashSet};
use std::fmt;

use nocter_model::{GenericParameterId, TypeId, TypeKind, TypeStore};

/// Generic bindings produced by first-order structural type unification.
///
/// The variable set is supplied by the caller. A generic identity outside that set is an opaque
/// type term, even when a prior binding moves it to the left side of a later equation.
#[derive(Clone, Debug, Default)]
pub(crate) struct GenericBindings {
    bindings: HashMap<GenericParameterId, TypeId>,
}

impl GenericBindings {
    #[must_use]
    pub(crate) fn get(&self, parameter: GenericParameterId) -> Option<TypeId> {
        self.bindings.get(&parameter).copied()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (GenericParameterId, TypeId)> + '_ {
        self.bindings
            .iter()
            .map(|(parameter, ty)| (*parameter, *ty))
    }
}

/// Collects generic identities structurally reachable from the supplied roots.
///
/// This operation is intentionally explicit: a caller decides whether those identities are
/// variables or opaque terms by choosing which roots contribute to the returned set.
pub(crate) fn collect_generic_parameters(
    types: &TypeStore,
    roots: impl IntoIterator<Item = TypeId>,
) -> Result<HashSet<GenericParameterId>, TypeUnificationError> {
    let mut parameters = HashSet::new();
    let mut visited = HashSet::new();
    let mut pending = roots.into_iter().collect::<Vec<_>>();
    while let Some(ty) = pending.pop() {
        if !visited.insert(ty) {
            continue;
        }
        let kind = types.get(ty).ok_or(TypeUnificationError::UnknownType(ty))?;
        if let TypeKind::GenericParameter(parameter) = kind {
            parameters.insert(*parameter);
        }
        append_references(kind, &mut pending);
    }
    Ok(parameters)
}

/// Unifies structural type equations while allowing only `variables` to receive bindings.
///
/// Equality is exact, including borrow/callable capability, nominal identity, fixed-array length,
/// callable result provenance, and outcome-layer order. The algorithm is iterative and applies an
/// occurs check before every binding.
pub(crate) fn unify_type_pairs(
    types: &TypeStore,
    variables: impl IntoIterator<Item = GenericParameterId>,
    equations: impl IntoIterator<Item = (TypeId, TypeId)>,
) -> Result<GenericBindings, TypeUnificationError> {
    TypeUnifier {
        types,
        variables: variables.into_iter().collect(),
        bindings: HashMap::new(),
        pending: equations.into_iter().collect(),
    }
    .solve()
}

struct TypeUnifier<'types> {
    types: &'types TypeStore,
    variables: HashSet<GenericParameterId>,
    bindings: HashMap<GenericParameterId, TypeId>,
    pending: Vec<(TypeId, TypeId)>,
}

impl TypeUnifier<'_> {
    fn solve(mut self) -> Result<GenericBindings, TypeUnificationError> {
        while let Some((left, right)) = self.pending.pop() {
            let left = self.resolve(left)?;
            let right = self.resolve(right)?;
            if left == right {
                continue;
            }
            let left_kind = self.kind(left)?.clone();
            let right_kind = self.kind(right)?.clone();
            if let Some(variable) = self.variable(&left_kind) {
                self.bind(variable, right)?;
                continue;
            }
            if let Some(variable) = self.variable(&right_kind) {
                self.bind(variable, left)?;
                continue;
            }
            if !decompose_pair(&left_kind, &right_kind, &mut self.pending) {
                return Err(TypeUnificationConflict { left, right }.into());
            }
        }
        Ok(GenericBindings {
            bindings: self.bindings,
        })
    }

    fn variable(&self, kind: &TypeKind) -> Option<GenericParameterId> {
        let TypeKind::GenericParameter(parameter) = kind else {
            return None;
        };
        self.variables.contains(parameter).then_some(*parameter)
    }

    fn bind(
        &mut self,
        parameter: GenericParameterId,
        replacement: TypeId,
    ) -> Result<(), TypeUnificationError> {
        if self.occurs(parameter, replacement)? {
            return Err(TypeUnificationError::RecursiveBinding {
                parameter,
                replacement,
            });
        }
        self.bindings.insert(parameter, replacement);
        Ok(())
    }

    fn resolve(&self, root: TypeId) -> Result<TypeId, TypeUnificationError> {
        let mut current = root;
        let mut visited = HashSet::new();
        loop {
            let kind = self.kind(current)?;
            let Some(variable) = self.variable(kind) else {
                return Ok(current);
            };
            let Some(next) = self.bindings.get(&variable).copied() else {
                return Ok(current);
            };
            if !visited.insert(variable) {
                return Err(TypeUnificationError::RecursiveBinding {
                    parameter: variable,
                    replacement: next,
                });
            }
            current = next;
        }
    }

    fn occurs(
        &self,
        expected: GenericParameterId,
        root: TypeId,
    ) -> Result<bool, TypeUnificationError> {
        let mut pending = vec![root];
        let mut visited = HashSet::new();
        while let Some(ty) = pending.pop() {
            let ty = self.resolve(ty)?;
            if !visited.insert(ty) {
                continue;
            }
            let kind = self.kind(ty)?;
            if self.variable(kind) == Some(expected) {
                return Ok(true);
            }
            append_references(kind, &mut pending);
        }
        Ok(false)
    }

    fn kind(&self, ty: TypeId) -> Result<&TypeKind, TypeUnificationError> {
        self.types
            .get(ty)
            .ok_or(TypeUnificationError::UnknownType(ty))
    }
}

fn decompose_pair(left: &TypeKind, right: &TypeKind, pending: &mut Vec<(TypeId, TypeId)>) -> bool {
    if let (TypeKind::Callable(left), TypeKind::Callable(right)) = (left, right) {
        return decompose_callable(left, right, pending);
    }
    match (left, right) {
        (TypeKind::Builtin(left), TypeKind::Builtin(right)) => left == right,
        (TypeKind::GenericParameter(left), TypeKind::GenericParameter(right)) => left == right,
        (TypeKind::InterfaceSelf(left), TypeKind::InterfaceSelf(right)) => left == right,
        (
            TypeKind::Closure {
                definition: left_definition,
                arguments: left_arguments,
            },
            TypeKind::Closure {
                definition: right_definition,
                arguments: right_arguments,
            },
        ) if left_definition == right_definition
            && left_arguments.len() == right_arguments.len() =>
        {
            append_paired(left_arguments, right_arguments, pending);
            true
        }
        (
            TypeKind::Nominal {
                definition: left_definition,
                arguments: left_arguments,
            },
            TypeKind::Nominal {
                definition: right_definition,
                arguments: right_arguments,
            },
        ) if left_definition == right_definition
            && left_arguments.len() == right_arguments.len() =>
        {
            append_paired(left_arguments, right_arguments, pending);
            true
        }
        (
            TypeKind::Opaque {
                definition: left_definition,
                arguments: left_arguments,
            },
            TypeKind::Opaque {
                definition: right_definition,
                arguments: right_arguments,
            },
        ) if left_definition == right_definition
            && left_arguments.len() == right_arguments.len() =>
        {
            append_paired(left_arguments, right_arguments, pending);
            true
        }
        (
            TypeKind::AssociatedProjection {
                base: left_base,
                associated: left_associated,
            },
            TypeKind::AssociatedProjection {
                base: right_base,
                associated: right_associated,
            },
        ) if left_associated == right_associated => {
            pending.push((*left_base, *right_base));
            true
        }
        (TypeKind::Pointer(left), TypeKind::Pointer(right))
        | (TypeKind::Slice(left), TypeKind::Slice(right))
        | (TypeKind::Optional(left), TypeKind::Optional(right))
        | (TypeKind::Fallible(left), TypeKind::Fallible(right)) => {
            pending.push((*left, *right));
            true
        }
        (
            TypeKind::Borrow {
                capability: left_capability,
                referent: left_referent,
            },
            TypeKind::Borrow {
                capability: right_capability,
                referent: right_referent,
            },
        ) if left_capability == right_capability => {
            pending.push((*left_referent, *right_referent));
            true
        }
        (
            TypeKind::FixedArray {
                element: left_element,
                length: left_length,
            },
            TypeKind::FixedArray {
                element: right_element,
                length: right_length,
            },
        ) if left_length == right_length => {
            pending.push((*left_element, *right_element));
            true
        }
        _ => false,
    }
}

fn decompose_callable(
    left: &nocter_model::CallableContract,
    right: &nocter_model::CallableContract,
    pending: &mut Vec<(TypeId, TypeId)>,
) -> bool {
    if left.capability() != right.capability()
        || left.provenance() != right.provenance()
        || left.parameters().len() != right.parameters().len()
    {
        return false;
    }
    append_paired(left.parameters(), right.parameters(), pending);
    pending.push((left.result(), right.result()));
    true
}

fn append_paired(left: &[TypeId], right: &[TypeId], pending: &mut Vec<(TypeId, TypeId)>) {
    pending.extend(left.iter().copied().zip(right.iter().copied()));
}

fn append_references(kind: &TypeKind, output: &mut Vec<TypeId>) {
    match kind {
        TypeKind::Builtin(_) | TypeKind::GenericParameter(_) | TypeKind::InterfaceSelf(_) => {}
        TypeKind::Nominal { arguments, .. }
        | TypeKind::Opaque { arguments, .. }
        | TypeKind::Closure { arguments, .. } => {
            output.extend(arguments.iter().copied());
        }
        TypeKind::AssociatedProjection { base, .. }
        | TypeKind::Pointer(base)
        | TypeKind::Borrow { referent: base, .. }
        | TypeKind::Slice(base)
        | TypeKind::FixedArray { element: base, .. }
        | TypeKind::Optional(base)
        | TypeKind::Fallible(base) => output.push(*base),
        TypeKind::Callable(contract) => {
            output.extend(contract.parameters().iter().copied());
            output.push(contract.result());
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TypeUnificationConflict {
    left: TypeId,
    right: TypeId,
}

impl TypeUnificationConflict {
    #[must_use]
    pub(crate) const fn left(self) -> TypeId {
        self.left
    }

    #[must_use]
    pub(crate) const fn right(self) -> TypeId {
        self.right
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TypeUnificationError {
    UnknownType(TypeId),
    Conflict(TypeUnificationConflict),
    RecursiveBinding {
        parameter: GenericParameterId,
        replacement: TypeId,
    },
}

impl From<TypeUnificationConflict> for TypeUnificationError {
    fn from(conflict: TypeUnificationConflict) -> Self {
        Self::Conflict(conflict)
    }
}

impl fmt::Display for TypeUnificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownType(ty) => write!(formatter, "unknown type {ty:?} in unification"),
            Self::Conflict(conflict) => write!(
                formatter,
                "types {:?} and {:?} cannot be unified",
                conflict.left, conflict.right
            ),
            Self::RecursiveBinding {
                parameter,
                replacement,
            } => write!(
                formatter,
                "generic parameter {parameter:?} occurs in replacement {replacement:?}"
            ),
        }
    }
}

impl std::error::Error for TypeUnificationError {}

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, GenericParameterId, TypeKind, TypeStore};

    use super::{collect_generic_parameters, unify_type_pairs};

    #[test]
    fn requested_generic_terms_never_become_pattern_variables() {
        let mut parameters = ArenaBuilder::<GenericParameterId, _>::new();
        let pattern = parameters.insert(());
        let requested_first = parameters.insert(());
        let requested_second = parameters.insert(());
        let _ = parameters.finish();
        let mut types = TypeStore::new();
        let pattern = types.intern(TypeKind::GenericParameter(pattern)).unwrap();
        let requested_first = types
            .intern(TypeKind::GenericParameter(requested_first))
            .unwrap();
        let requested_second = types
            .intern(TypeKind::GenericParameter(requested_second))
            .unwrap();
        let variables = collect_generic_parameters(&types, [pattern]).unwrap();

        assert!(
            unify_type_pairs(
                &types,
                variables,
                [(pattern, requested_first), (pattern, requested_second)],
            )
            .is_err()
        );
    }

    #[test]
    fn nested_structures_produce_one_binding_per_declared_variable() {
        let mut parameters = ArenaBuilder::<GenericParameterId, _>::new();
        let parameter = parameters.insert(());
        let _ = parameters.finish();
        let mut types = TypeStore::new();
        let variable = types.intern(TypeKind::GenericParameter(parameter)).unwrap();
        let pattern = types.intern(TypeKind::Optional(variable)).unwrap();
        let concrete = types.builtin(nocter_model::BuiltinType::I32);
        let actual = types.intern(TypeKind::Optional(concrete)).unwrap();
        let bindings = unify_type_pairs(&types, [parameter], [(pattern, actual)]).unwrap();

        assert_eq!(bindings.get(parameter), Some(concrete));
    }
}
