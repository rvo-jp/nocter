use std::collections::{HashMap, HashSet};
use std::fmt;

use nocter_model::{GenericParameterId, GenericValue, InterfaceId, TypeId, TypeKind, TypeStore};

use super::{map_type_children, visit_type_children};

#[derive(Clone, Debug, Default)]
pub struct TypeSubstitution {
    interface_self: Option<(InterfaceId, TypeId)>,
    generics: HashMap<GenericParameterId, TypeId>,
    constants: HashMap<GenericParameterId, nocter_model::UsizeTerm>,
    associated: HashMap<nocter_model::AssociatedTypeId, TypeId>,
}

impl TypeSubstitution {
    pub fn set_interface_self(&mut self, interface: InterfaceId, target: TypeId) {
        self.interface_self = Some((interface, target));
    }

    pub fn bind_generic(&mut self, source: GenericParameterId, target_type: TypeId) {
        self.generics.insert(source, target_type);
    }

    pub fn bind_constant(&mut self, source: GenericParameterId, value: u64) {
        self.constants.insert(source, value.into());
    }

    pub fn bind_constant_term(
        &mut self,
        source: GenericParameterId,
        value: nocter_model::UsizeTerm,
    ) {
        self.constants.insert(source, value);
    }

    pub fn bind_value(&mut self, source: GenericParameterId, value: GenericValue) {
        match value {
            GenericValue::Type(ty) => self.bind_generic(source, ty),
            GenericValue::Usize(value) => self.bind_constant_term(source, value),
        }
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
        self.constants.extend(
            other
                .constants
                .iter()
                .map(|(parameter, value)| (*parameter, *value)),
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
        types: &mut nocter_model::TypeTransaction,
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
                    let rebuilt = self.apply_constant_terms(rebuilt);
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

    pub(crate) fn apply_value(
        &self,
        types: &mut nocter_model::TypeTransaction,
        value: GenericValue,
    ) -> Result<GenericValue, SubstitutionError> {
        match value {
            GenericValue::Type(ty) => self.apply_type(types, ty).map(GenericValue::Type),
            GenericValue::Usize(value) => Ok(GenericValue::Usize(self.constant_term(value))),
        }
    }

    pub(crate) fn apply_application(
        &self,
        types: &mut nocter_model::TypeTransaction,
        application: &nocter_model::GenericApplication,
    ) -> Result<nocter_model::GenericApplication, SubstitutionError> {
        application
            .iter()
            .copied()
            .map(|value| self.apply_value(types, value))
            .collect::<Result<Vec<_>, _>>()
            .map(nocter_model::GenericApplication::new)
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

    fn apply_constant_terms(&self, kind: TypeKind) -> TypeKind {
        match kind {
            TypeKind::FixedArray {
                element,
                length: nocter_model::UsizeTerm::Parameter(parameter),
            } => TypeKind::FixedArray {
                element,
                length: self.constant_term(nocter_model::UsizeTerm::Parameter(parameter)),
            },
            TypeKind::Nominal {
                definition,
                arguments,
            } => TypeKind::Nominal {
                definition,
                arguments: self.apply_application_constants(arguments),
            },
            TypeKind::Opaque {
                definition,
                arguments,
            } => TypeKind::Opaque {
                definition,
                arguments: self.apply_application_constants(arguments),
            },
            TypeKind::Closure {
                definition,
                arguments,
            } => TypeKind::Closure {
                definition,
                arguments: self.apply_application_constants(arguments),
            },
            other => other,
        }
    }

    fn apply_application_constants(
        &self,
        application: nocter_model::GenericApplication,
    ) -> nocter_model::GenericApplication {
        nocter_model::GenericApplication::new(
            application
                .iter()
                .map(|value| match value {
                    GenericValue::Type(ty) => GenericValue::Type(*ty),
                    GenericValue::Usize(value) => GenericValue::Usize(self.constant_term(*value)),
                })
                .collect::<Vec<_>>(),
        )
    }

    fn constant_term(&self, value: nocter_model::UsizeTerm) -> nocter_model::UsizeTerm {
        let mut current = value;
        let mut visited = HashSet::new();
        while let nocter_model::UsizeTerm::Parameter(parameter) = current {
            if !visited.insert(parameter) {
                break;
            }
            let Some(replacement) = self.constants.get(&parameter).copied() else {
                break;
            };
            current = replacement;
        }
        current
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
    use nocter_model::{ArenaBuilder, GenericParameterId, NominalTypeId, TypeAuthority, TypeKind};

    use super::TypeSubstitution;

    #[test]
    fn chained_generic_replacements_reach_one_canonical_type() {
        let mut parameters = ArenaBuilder::<GenericParameterId, _>::new();
        let first = parameters.insert(());
        let second = parameters.insert(());
        let _ = parameters.finish();
        let mut types = TypeAuthority::new().transaction();
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
        let mut types = TypeAuthority::new().transaction();
        let generic = types.intern(TypeKind::GenericParameter(parameter)).unwrap();
        let mut substitution = TypeSubstitution::default();
        substitution.bind_generic(parameter, generic);

        assert_eq!(substitution.apply_type(&mut types, generic), Ok(generic));
    }

    #[test]
    fn constant_replacement_closes_symbolic_array_lengths() {
        let mut parameters = ArenaBuilder::<GenericParameterId, _>::new();
        let parameter = parameters.insert(());
        let _ = parameters.finish();
        let mut types = TypeAuthority::new().transaction();
        let byte = types.builtin(nocter_model::BuiltinType::U8);
        let symbolic = types
            .intern(TypeKind::FixedArray {
                element: byte,
                length: nocter_model::UsizeTerm::Parameter(parameter),
            })
            .unwrap();
        let mut substitution = TypeSubstitution::default();
        substitution.bind_constant(parameter, 16);

        let concrete = substitution.apply_type(&mut types, symbolic).unwrap();
        assert!(matches!(
            types.get(concrete),
            Some(TypeKind::FixedArray { length, .. }) if length.closed_value() == Some(16)
        ));
        assert_eq!(types.is_concrete(concrete), Some(true));
    }

    #[test]
    fn constant_replacement_closes_nominal_application_values() {
        let mut parameters = ArenaBuilder::<GenericParameterId, _>::new();
        let parameter = parameters.insert(());
        let _ = parameters.finish();
        let mut types = TypeAuthority::new().transaction();
        let byte = types.builtin(nocter_model::BuiltinType::U8);
        let symbolic = types
            .intern(TypeKind::Nominal {
                definition: {
                    let mut definitions = ArenaBuilder::<NominalTypeId, _>::new();
                    let definition = definitions.insert(());
                    let _ = definitions.finish();
                    definition
                },
                arguments: nocter_model::GenericApplication::new([
                    nocter_model::GenericValue::Type(byte),
                    nocter_model::GenericValue::Usize(nocter_model::UsizeTerm::Parameter(
                        parameter,
                    )),
                ]),
            })
            .unwrap();
        let mut substitution = TypeSubstitution::default();
        substitution.bind_constant(parameter, 16);

        let concrete = substitution.apply_type(&mut types, symbolic).unwrap();
        let Some(TypeKind::Nominal { arguments, .. }) = types.get(concrete) else {
            panic!("substitution must preserve nominal identity")
        };
        assert_eq!(
            arguments.as_slice()[1].as_usize().unwrap().closed_value(),
            Some(16)
        );
        assert_eq!(types.is_concrete(concrete), Some(true));
    }
}
