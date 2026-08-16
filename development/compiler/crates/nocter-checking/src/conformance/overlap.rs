use std::collections::HashMap;

use nocter_declarations::InterfaceApplication;
use nocter_model::{GenericParameterId, TypeId, TypeKind, TypeStore};

use super::substitution::{SubstitutionError, TypeSubstitution};

/// Reports whether two normalized conformance patterns can denote one concrete application.
///
/// Generic parameter identities are compile-unit global, so variables from two conformances
/// cannot alias accidentally. Non-refinement requirements do not make a pattern disjoint: a
/// concrete type may satisfy both sets of capabilities.
pub(super) fn patterns_overlap(
    types: &TypeStore,
    left_interface: &InterfaceApplication,
    left_target: TypeId,
    right_interface: &InterfaceApplication,
    right_target: TypeId,
) -> Result<bool, SubstitutionError> {
    if left_interface.interface() != right_interface.interface()
        || left_interface.arguments().len() != right_interface.arguments().len()
    {
        return Ok(false);
    }
    let mut unifier = PatternUnifier::new(types);
    unifier.pending.push((left_target, right_target));
    unifier.pending.extend(
        left_interface
            .arguments()
            .iter()
            .copied()
            .zip(right_interface.arguments().iter().copied()),
    );
    unifier.solve()
}

/// Matches a conformance pattern against one requested application.
///
/// Only generic parameters in the pattern are variables. A generic parameter in the requested
/// application remains an opaque term owned by the requesting context.
pub(super) fn match_pattern(
    types: &TypeStore,
    pattern_interface: &InterfaceApplication,
    pattern_target: TypeId,
    requested_interface: &InterfaceApplication,
    requested_target: TypeId,
) -> Result<Option<TypeSubstitution>, SubstitutionError> {
    if pattern_interface.interface() != requested_interface.interface()
        || pattern_interface.arguments().len() != requested_interface.arguments().len()
    {
        return Ok(None);
    }
    let mut matcher = PatternMatcher::new(types);
    matcher.pending.push((pattern_target, requested_target));
    matcher.pending.extend(
        pattern_interface
            .arguments()
            .iter()
            .copied()
            .zip(requested_interface.arguments().iter().copied()),
    );
    matcher.solve()
}

struct PatternMatcher<'types> {
    types: &'types TypeStore,
    bindings: HashMap<GenericParameterId, TypeId>,
    pending: Vec<(TypeId, TypeId)>,
}

impl<'types> PatternMatcher<'types> {
    fn new(types: &'types TypeStore) -> Self {
        Self {
            types,
            bindings: HashMap::new(),
            pending: Vec::new(),
        }
    }

    fn solve(mut self) -> Result<Option<TypeSubstitution>, SubstitutionError> {
        while let Some((pattern, requested)) = self.pending.pop() {
            if pattern == requested {
                continue;
            }
            let pattern_kind = self.kind(pattern)?.clone();
            let requested_kind = self.kind(requested)?.clone();
            if let TypeKind::GenericParameter(variable) = pattern_kind {
                if let Some(previous) = self.bindings.get(&variable).copied() {
                    self.pending.push((previous, requested));
                } else {
                    if contains(self.types, requested, variable)? {
                        return Ok(None);
                    }
                    self.bindings.insert(variable, requested);
                }
                continue;
            }
            if !decompose_pair(&pattern_kind, &requested_kind, &mut self.pending) {
                return Ok(None);
            }
        }
        let mut substitution = TypeSubstitution::default();
        for (parameter, ty) in self.bindings {
            substitution.bind_generic(parameter, ty);
        }
        Ok(Some(substitution))
    }

    fn kind(&self, ty: TypeId) -> Result<&TypeKind, SubstitutionError> {
        self.types.get(ty).ok_or(SubstitutionError::UnknownType(ty))
    }
}

struct PatternUnifier<'types> {
    types: &'types TypeStore,
    bindings: HashMap<GenericParameterId, TypeId>,
    pending: Vec<(TypeId, TypeId)>,
}

impl<'types> PatternUnifier<'types> {
    fn new(types: &'types TypeStore) -> Self {
        Self {
            types,
            bindings: HashMap::new(),
            pending: Vec::new(),
        }
    }

    fn solve(mut self) -> Result<bool, SubstitutionError> {
        while let Some((left, right)) = self.pending.pop() {
            let left = self.resolve(left)?;
            let right = self.resolve(right)?;
            if left == right {
                continue;
            }
            let left_kind = self.kind(left)?.clone();
            let right_kind = self.kind(right)?.clone();
            match (&left_kind, &right_kind) {
                (TypeKind::GenericParameter(variable), _) => {
                    if self.occurs(*variable, right)? {
                        return Ok(false);
                    }
                    self.bindings.insert(*variable, right);
                }
                (_, TypeKind::GenericParameter(variable)) => {
                    if self.occurs(*variable, left)? {
                        return Ok(false);
                    }
                    self.bindings.insert(*variable, left);
                }
                (left, right) if !self.decompose(left, right) => return Ok(false),
                _ => {}
            }
        }
        Ok(true)
    }

    fn resolve(&self, root: TypeId) -> Result<TypeId, SubstitutionError> {
        let mut current = root;
        let mut visited = std::collections::HashSet::new();
        loop {
            let TypeKind::GenericParameter(variable) = self.kind(current)? else {
                return Ok(current);
            };
            let Some(next) = self.bindings.get(variable).copied() else {
                return Ok(current);
            };
            if !visited.insert(*variable) {
                return Err(SubstitutionError::CyclicReplacement(current));
            }
            current = next;
        }
    }

    fn occurs(
        &self,
        expected: GenericParameterId,
        root: TypeId,
    ) -> Result<bool, SubstitutionError> {
        let mut pending = vec![root];
        let mut visited = std::collections::HashSet::new();
        while let Some(ty) = pending.pop() {
            let ty = self.resolve(ty)?;
            if !visited.insert(ty) {
                continue;
            }
            match self.kind(ty)? {
                TypeKind::GenericParameter(actual) if *actual == expected => return Ok(true),
                kind => references(kind, &mut pending),
            }
        }
        Ok(false)
    }

    fn decompose(&mut self, left: &TypeKind, right: &TypeKind) -> bool {
        decompose_pair(left, right, &mut self.pending)
    }

    fn kind(&self, ty: TypeId) -> Result<&TypeKind, SubstitutionError> {
        self.types.get(ty).ok_or(SubstitutionError::UnknownType(ty))
    }
}

fn decompose_pair(left: &TypeKind, right: &TypeKind, pending: &mut Vec<(TypeId, TypeId)>) -> bool {
    if let (TypeKind::Callable(left), TypeKind::Callable(right)) = (left, right) {
        return decompose_callable(left, right, pending);
    }
    match (left, right) {
        (TypeKind::Builtin(left), TypeKind::Builtin(right)) => left == right,
        (TypeKind::InterfaceSelf(left), TypeKind::InterfaceSelf(right)) => left == right,
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
            pending.extend(
                left_arguments
                    .iter()
                    .copied()
                    .zip(right_arguments.iter().copied()),
            );
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
            pending.extend(
                left_arguments
                    .iter()
                    .copied()
                    .zip(right_arguments.iter().copied()),
            );
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
    pending.extend(
        left.parameters()
            .iter()
            .copied()
            .zip(right.parameters().iter().copied()),
    );
    pending.push((left.result(), right.result()));
    true
}

fn contains(
    types: &TypeStore,
    root: TypeId,
    expected: GenericParameterId,
) -> Result<bool, SubstitutionError> {
    let mut pending = vec![root];
    let mut visited = std::collections::HashSet::new();
    while let Some(ty) = pending.pop() {
        if !visited.insert(ty) {
            continue;
        }
        let kind = types.get(ty).ok_or(SubstitutionError::UnknownType(ty))?;
        if matches!(kind, TypeKind::GenericParameter(actual) if *actual == expected) {
            return Ok(true);
        }
        references(kind, &mut pending);
    }
    Ok(false)
}

fn references(kind: &TypeKind, output: &mut Vec<TypeId>) {
    match kind {
        TypeKind::Builtin(_) | TypeKind::GenericParameter(_) | TypeKind::InterfaceSelf(_) => {}
        TypeKind::Nominal { arguments, .. } | TypeKind::Opaque { arguments, .. } => {
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
