use std::collections::{HashMap, HashSet};
use std::fmt;

use nocter_model::{GenericParameterId, InterfaceId, TypeId, TypeKind, TypeStore};

use super::{map_type_children, visit_type_children};

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

    pub(crate) fn extend(&mut self, other: &Self) {
        if let Some((interface, target)) = other.interface_self {
            self.set_interface_self(interface, target);
        }
        self.generics.extend(
            other
                .generics
                .iter()
                .map(|(parameter, ty)| (*parameter, *ty)),
        );
        self.associated.extend(
            other
                .associated
                .iter()
                .map(|(declaration, ty)| (*declaration, *ty)),
        );
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
                        visit_type_children(&kind, |child| children.push(child));
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
                    let rebuilt = map_type_children(kind, |ty| {
                        finished
                            .get(&ty)
                            .copied()
                            .ok_or(SubstitutionError::InvalidStore)
                    })?;
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
