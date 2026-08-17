use std::collections::{HashMap, HashSet};
use std::fmt;

use nocter_model::{
    CallableContract, GenericParameterId, InterfaceId, TypeId, TypeKind, TypeStore,
};

#[derive(Clone, Debug, Default)]
pub struct TypeSubstitution {
    interface_self: Option<(InterfaceId, TypeId)>,
    generics: HashMap<GenericParameterId, TypeId>,
    associated: HashMap<nocter_model::AssociatedTypeId, TypeId>,
}

impl TypeSubstitution {
    pub fn set_interface_self(&mut self, interface: InterfaceId, target: TypeId) {
        self.interface_self = Some((interface, target));
    }

    pub fn bind_generic(&mut self, source: GenericParameterId, target_type: TypeId) {
        self.generics.insert(source, target_type);
    }

    pub fn bind_associated(&mut self, declaration: nocter_model::AssociatedTypeId, target: TypeId) {
        self.associated.insert(declaration, target);
    }

    /// Applies this semantic substitution and interns the normalized result in `types`.
    ///
    /// # Errors
    ///
    /// Returns a typed failure when the source store is incomplete, a replacement cycle exists,
    /// or rebuilding a structural type would violate store integrity.
    pub fn apply_type(
        &self,
        types: &mut TypeStore,
        root: TypeId,
    ) -> Result<TypeId, SubstitutionError> {
        enum Action {
            Enter(TypeId),
            Replace { source: TypeId, target: TypeId },
            Rebuild { source: TypeId, kind: TypeKind },
        }

        let mut finished = HashMap::new();
        let mut active = HashSet::new();
        let mut pending = vec![Action::Enter(root)];
        while let Some(action) = pending.pop() {
            match action {
                Action::Enter(ty) => {
                    if finished.contains_key(&ty) {
                        continue;
                    }
                    if !active.insert(ty) {
                        return Err(SubstitutionError::CyclicReplacement(ty));
                    }
                    let kind = types
                        .get(ty)
                        .cloned()
                        .ok_or(SubstitutionError::UnknownType(ty))?;
                    if let Some(replacement) = self
                        .direct_replacement(types, &kind)
                        .filter(|replacement| *replacement != ty)
                    {
                        pending.push(Action::Replace {
                            source: ty,
                            target: replacement,
                        });
                        pending.push(Action::Enter(replacement));
                    } else {
                        let mut children = Vec::new();
                        kind.references(|child| children.push(child));
                        pending.push(Action::Rebuild { source: ty, kind });
                        pending.extend(children.into_iter().rev().map(Action::Enter));
                    }
                }
                Action::Replace { source, target } => {
                    let replacement = finished
                        .get(&target)
                        .copied()
                        .ok_or(SubstitutionError::InvalidStore)?;
                    active.remove(&source);
                    finished.insert(source, replacement);
                }
                Action::Rebuild { source, kind } => {
                    let rebuilt = rebuild(kind, &finished)?;
                    let normalized = types
                        .intern(rebuilt)
                        .map_err(|_| SubstitutionError::InvalidStore)?;
                    active.remove(&source);
                    finished.insert(source, normalized);
                }
            }
        }
        finished
            .get(&root)
            .copied()
            .ok_or(SubstitutionError::InvalidStore)
    }

    fn direct_replacement(&self, types: &TypeStore, kind: &TypeKind) -> Option<TypeId> {
        match kind {
            TypeKind::GenericParameter(parameter) => self.generics.get(parameter).copied(),
            TypeKind::InterfaceSelf(interface) => self
                .interface_self
                .filter(|(expected, _)| expected == interface)
                .map(|(_, target)| target),
            TypeKind::AssociatedProjection { base, associated }
                if matches!(
                    (types.get(*base), self.interface_self),
                    (Some(TypeKind::InterfaceSelf(actual)), Some((expected, _))) if actual == &expected
                ) =>
            {
                self.associated.get(associated).copied()
            }
            _ => None,
        }
    }
}

fn rebuild(
    kind: TypeKind,
    finished: &HashMap<TypeId, TypeId>,
) -> Result<TypeKind, SubstitutionError> {
    let mapped = |ty: TypeId| {
        finished
            .get(&ty)
            .copied()
            .ok_or(SubstitutionError::InvalidStore)
    };
    Ok(match kind {
        TypeKind::Builtin(builtin) => TypeKind::Builtin(builtin),
        TypeKind::GenericParameter(parameter) => TypeKind::GenericParameter(parameter),
        TypeKind::InterfaceSelf(interface) => TypeKind::InterfaceSelf(interface),
        TypeKind::Closure(closure) => TypeKind::Closure(closure),
        TypeKind::Nominal {
            definition,
            arguments,
        } => TypeKind::Nominal {
            definition,
            arguments: arguments
                .iter()
                .map(|argument| mapped(*argument))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        },
        TypeKind::AssociatedProjection { base, associated } => TypeKind::AssociatedProjection {
            base: mapped(base)?,
            associated,
        },
        TypeKind::Opaque {
            definition,
            arguments,
        } => TypeKind::Opaque {
            definition,
            arguments: arguments
                .iter()
                .map(|argument| mapped(*argument))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        },
        TypeKind::Pointer(base) => TypeKind::Pointer(mapped(base)?),
        TypeKind::Borrow {
            capability,
            referent,
        } => TypeKind::Borrow {
            capability,
            referent: mapped(referent)?,
        },
        TypeKind::Slice(element) => TypeKind::Slice(mapped(element)?),
        TypeKind::FixedArray { element, length } => TypeKind::FixedArray {
            element: mapped(element)?,
            length,
        },
        TypeKind::Callable(contract) => TypeKind::Callable(
            CallableContract::new(
                contract.capability(),
                contract
                    .parameters()
                    .iter()
                    .map(|parameter| mapped(*parameter))
                    .collect::<Result<Vec<_>, _>>()?,
                mapped(contract.result())?,
                contract.provenance().clone(),
            )
            .map_err(|_| SubstitutionError::InvalidStore)?,
        ),
        TypeKind::Optional(payload) => TypeKind::Optional(mapped(payload)?),
        TypeKind::Fallible(payload) => TypeKind::Fallible(mapped(payload)?),
    })
}

trait TypeReferences {
    fn references(&self, visit: impl FnMut(TypeId));
}

impl TypeReferences for TypeKind {
    fn references(&self, mut visit: impl FnMut(TypeId)) {
        match self {
            Self::Builtin(_)
            | Self::GenericParameter(_)
            | Self::InterfaceSelf(_)
            | Self::Closure(_) => {}
            Self::Nominal { arguments, .. } | Self::Opaque { arguments, .. } => {
                arguments.iter().copied().for_each(&mut visit);
            }
            Self::AssociatedProjection { base, .. }
            | Self::Pointer(base)
            | Self::Borrow { referent: base, .. }
            | Self::Slice(base)
            | Self::FixedArray { element: base, .. }
            | Self::Optional(base)
            | Self::Fallible(base) => visit(*base),
            Self::Callable(contract) => {
                contract.parameters().iter().copied().for_each(&mut visit);
                visit(contract.result());
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubstitutionError {
    UnknownType(TypeId),
    CyclicReplacement(TypeId),
    InvalidStore,
}

impl fmt::Display for SubstitutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownType(ty) => write!(formatter, "unknown type {ty:?} in substitution"),
            Self::CyclicReplacement(ty) => {
                write!(
                    formatter,
                    "cyclic replacement reached {ty:?} in substitution"
                )
            }
            Self::InvalidStore => formatter.write_str("type substitution produced invalid store"),
        }
    }
}

impl std::error::Error for SubstitutionError {}

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, GenericParameterId, TypeKind, TypeStore};

    use super::TypeSubstitution;

    #[test]
    fn chained_generic_replacements_reach_one_canonical_type() {
        let mut parameters = ArenaBuilder::<GenericParameterId, _>::new();
        let first = parameters.insert(());
        let second = parameters.insert(());
        let _ = parameters.finish();
        let mut types = TypeStore::new();
        let first_type = types.intern(TypeKind::GenericParameter(first)).unwrap();
        let second_type = types.intern(TypeKind::GenericParameter(second)).unwrap();
        let expected = types.builtin(nocter_model::BuiltinType::I32);
        let mut substitution = TypeSubstitution::default();
        substitution.bind_generic(first, second_type);
        substitution.bind_generic(second, expected);

        assert_eq!(
            substitution.apply_type(&mut types, first_type).unwrap(),
            expected
        );
    }

    #[test]
    fn identity_generic_replacement_is_a_no_op() {
        let mut parameters = ArenaBuilder::<GenericParameterId, _>::new();
        let parameter = parameters.insert(());
        let _ = parameters.finish();
        let mut types = TypeStore::new();
        let generic = types.intern(TypeKind::GenericParameter(parameter)).unwrap();
        let mut substitution = TypeSubstitution::default();
        substitution.bind_generic(parameter, generic);

        assert_eq!(substitution.apply_type(&mut types, generic), Ok(generic));
    }
}
