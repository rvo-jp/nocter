use crate::TypeId;

/// Source-independent operation permission. This is not a borrow capability or a coercion rank.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueAccessMode {
    Read,
    Update,
    Own,
}

/// Transient operation access to one resolved data shape, not a qualified semantic data type.
///
/// The owner of the surrounding semantic product validates `data` against its type authority.
/// This descriptor does not flatten an outcome of access into access to an outcome, grant Copy
/// evidence, or describe a physical pointer/storage layout.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueAccess {
    data: TypeId,
    mode: ValueAccessMode,
}

impl ValueAccess {
    #[must_use]
    pub const fn new(data: TypeId, mode: ValueAccessMode) -> Self {
        Self { data, mode }
    }

    #[must_use]
    pub const fn data(self) -> TypeId {
        self.data
    }

    #[must_use]
    pub const fn mode(self) -> ValueAccessMode {
        self.mode
    }
}

/// Operation selected by a callable input, independently of the input's data type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueInputOperation {
    Read,
    Edit,
    Take,
}
impl ValueInputOperation {
    #[must_use]
    pub const fn access_mode(self) -> ValueAccessMode {
        match self {
            Self::Read => ValueAccessMode::Read,
            Self::Edit => ValueAccessMode::Update,
            Self::Take => ValueAccessMode::Own,
        }
    }
    #[must_use]
    pub const fn binding_kind(self) -> ValueBindingKind {
        match self {
            Self::Read => ValueBindingKind::ReadAlias,
            Self::Edit => ValueBindingKind::EditAlias,
            Self::Take => ValueBindingKind::MutableValue,
        }
    }
}

/// How a local stores a value or accesses a separate existing place.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueBindingKind {
    FixedValue,
    MutableValue,
    ReadAlias,
    EditAlias,
}
impl ValueBindingKind {
    #[must_use]
    pub const fn access_mode(self) -> ValueAccessMode {
        match self {
            Self::FixedValue | Self::MutableValue => ValueAccessMode::Own,
            Self::ReadAlias => ValueAccessMode::Read,
            Self::EditAlias => ValueAccessMode::Update,
        }
    }
    #[must_use]
    pub const fn writable(self) -> bool {
        matches!(self, Self::MutableValue | Self::EditAlias)
    }
}

/// A concrete code declaration or the statically bound witness supplied to a formal input.
/// A parameter origin is symbolic, never a runtime-erased function pointer.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueFunctionOrigin {
    Declaration(crate::AccessCallableId),
    Parameter(crate::AccessParameterId),
    /// A declared callable result, closed by checking its reachable return producers.
    Result(crate::AccessCallableId),
}

/// Invocation access, independent of a callable value's binding or explicit inputs.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueInvocation {
    Read,
    Edit,
    Take,
}
impl ValueInvocation {
    #[must_use]
    pub const fn operation(self) -> ValueInputOperation {
        match self {
            Self::Read => ValueInputOperation::Read,
            Self::Edit => ValueInputOperation::Edit,
            Self::Take => ValueInputOperation::Take,
        }
    }
    #[must_use]
    pub const fn can_supply(self, expected: Self) -> bool {
        matches!(
            (self, expected),
            (Self::Read, _) | (Self::Edit, Self::Edit | Self::Take) | (Self::Take, Self::Take)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccessCallableId, ArenaBuilder, TypeAuthority, TypeKind};
    #[test]
    fn named_witness_projection_preserves_identity_and_invocation_view() {
        let mut declarations = ArenaBuilder::<AccessCallableId, ()>::new();
        let first = declarations.insert(());
        let second = declarations.insert(());
        let base = TypeAuthority::new();
        let mut types = base.transaction();
        let mut ids = Vec::new();
        for (declaration, invocation) in [
            (first, ValueInvocation::Read),
            (first, ValueInvocation::Edit),
            (second, ValueInvocation::Read),
        ] {
            ids.push(
                types
                    .intern(TypeKind::ValueFunction {
                        origin: ValueFunctionOrigin::Declaration(declaration),
                        invocation,
                    })
                    .unwrap(),
            );
        }
        assert!(ids[0] != ids[1] && ids[0] != ids[2]);
        for id in ids {
            assert_eq!(types.is_concrete(id), Some(true));
            assert!(!types.may_carry_storage(id));
            let projected = types.project(id).unwrap();
            assert_eq!(projected.types().get(projected.root()), types.get(id));
        }
    }
}

#[cfg(test)]
mod formal_tests {
    use super::*;
    use crate::{AccessParameterId, ArenaBuilder, TypeAuthority, TypeKind};
    #[test]
    fn formal_witnesses_preserve_symbolic_storage_and_identity_on_projection() {
        let mut parameters = ArenaBuilder::<AccessParameterId, ()>::new();
        let first = parameters.insert(());
        let second = parameters.insert(());
        let base = TypeAuthority::new();
        let mut types = base.transaction();
        let mut ids = Vec::new();
        for parameter in [first, second] {
            let data = types
                .intern(TypeKind::ValueFunction {
                    origin: ValueFunctionOrigin::Parameter(parameter),
                    invocation: ValueInvocation::Read,
                })
                .unwrap();
            assert_eq!(types.is_concrete(data), Some(false));
            assert!(types.may_carry_storage(data));
            let projected = types.project(data).unwrap();
            assert_eq!(projected.types().get(projected.root()), types.get(data));
            ids.push(data);
        }
        assert_ne!(ids[0], ids[1]);
    }
}

#[cfg(test)]
mod result_tests {
    use super::*;
    use crate::{AccessCallableId, ArenaBuilder, TypeAuthority, TypeKind};
    #[test]
    fn result_signature_identity_is_symbolic_until_its_producer_is_closed() {
        let mut declarations = ArenaBuilder::<AccessCallableId, ()>::new();
        let id = declarations.insert(());
        let base = TypeAuthority::new();
        let mut types = base.transaction();
        let result = types
            .intern(TypeKind::ValueFunction {
                origin: ValueFunctionOrigin::Result(id),
                invocation: ValueInvocation::Read,
            })
            .unwrap();
        let named = types
            .intern(TypeKind::ValueFunction {
                origin: ValueFunctionOrigin::Declaration(id),
                invocation: ValueInvocation::Read,
            })
            .unwrap();
        assert_ne!(result, named);
        assert_eq!(types.is_concrete(result), Some(false));
        assert!(types.may_carry_storage(result));
        let projected = types.project(result).unwrap();
        assert_eq!(projected.types().get(projected.root()), types.get(result));
    }
}
