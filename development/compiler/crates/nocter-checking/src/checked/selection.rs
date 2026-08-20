use std::fmt;

use nocter_model::{CallableId, DropId, GenericParameterId, InterfaceId, RequirementId, TypeId};

/// Static operation selected during body checking.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StaticDispatch {
    Direct(CallableId),
    InterfaceMethod {
        requirement: RequirementId,
        method: CallableId,
    },
    /// A call from an interface default body to that interface's surface under intrinsic `Self`
    /// evidence. Concrete specialization selects the implementing conformance.
    InterfaceSelfMethod {
        interface: InterfaceId,
        method: CallableId,
    },
    /// One interface-owned default body selected for an exact receiver type.
    InterfaceDefault {
        interface: InterfaceId,
        receiver: TypeId,
        method: CallableId,
    },
    OpaqueMethod {
        opaque: TypeId,
        method: CallableId,
    },
    StructuralRequirement(RequirementId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenericArgument {
    parameter: GenericParameterId,
    ty: TypeId,
}

impl GenericArgument {
    #[must_use]
    pub const fn new(parameter: GenericParameterId, ty: TypeId) -> Self {
        Self { parameter, ty }
    }

    #[must_use]
    pub const fn parameter(self) -> GenericParameterId {
        self.parameter
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

/// Canonically ordered substitution selected for one generic operation.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenericArguments(Box<[GenericArgument]>);

impl GenericArguments {
    /// Creates arguments ordered by semantic parameter identity.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateGenericArgument`] when one parameter is bound twice.
    pub fn new(
        arguments: impl IntoIterator<Item = GenericArgument>,
    ) -> Result<Self, DuplicateGenericArgument> {
        let mut arguments = arguments.into_iter().collect::<Vec<_>>();
        arguments.sort_unstable_by_key(|argument| argument.parameter());
        if let Some(parameter) = arguments
            .windows(2)
            .find(|pair| pair[0].parameter() == pair[1].parameter())
            .map(|pair| pair[0].parameter())
        {
            return Err(DuplicateGenericArgument(parameter));
        }
        Ok(Self(arguments.into_boxed_slice()))
    }

    #[must_use]
    pub const fn as_slice(&self) -> &[GenericArgument] {
        &self.0
    }

    #[must_use]
    pub fn get(&self, parameter: GenericParameterId) -> Option<TypeId> {
        self.0
            .binary_search_by_key(&parameter, |argument| argument.parameter())
            .ok()
            .map(|index| self.0[index].ty())
    }
}

/// One exact static dispatch edge and the complete declaration-generic substitution it requires.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct StaticSelection {
    dispatch: StaticDispatch,
    generic_arguments: GenericArguments,
}

impl StaticSelection {
    #[must_use]
    pub const fn new(dispatch: StaticDispatch, generic_arguments: GenericArguments) -> Self {
        Self {
            dispatch,
            generic_arguments,
        }
    }

    #[must_use]
    pub const fn dispatch(&self) -> StaticDispatch {
        self.dispatch
    }

    #[must_use]
    pub const fn generic_arguments(&self) -> &GenericArguments {
        &self.generic_arguments
    }
}

/// One exact type-owned drop body and the complete declaration-generic substitution it requires.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DropSelection {
    declaration: DropId,
    generic_arguments: GenericArguments,
}

impl DropSelection {
    #[must_use]
    pub const fn new(declaration: DropId, generic_arguments: GenericArguments) -> Self {
        Self {
            declaration,
            generic_arguments,
        }
    }

    #[must_use]
    pub const fn declaration(&self) -> DropId {
        self.declaration
    }

    #[must_use]
    pub const fn generic_arguments(&self) -> &GenericArguments {
        &self.generic_arguments
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct DuplicateGenericArgument(GenericParameterId);

impl DuplicateGenericArgument {
    #[must_use]
    pub const fn parameter(self) -> GenericParameterId {
        self.0
    }
}

impl fmt::Debug for DuplicateGenericArgument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("DuplicateGenericArgument")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for DuplicateGenericArgument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "generic parameter {:?} is bound twice", self.0)
    }
}

impl std::error::Error for DuplicateGenericArgument {}

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, BuiltinType, GenericParameterId, TypeStore};

    use super::{GenericArgument, GenericArguments};

    #[test]
    fn generic_arguments_are_canonical_and_unique() {
        let mut parameters = ArenaBuilder::<GenericParameterId, _>::new();
        let first = parameters.insert(());
        let second = parameters.insert(());
        let _ = parameters.finish();
        let types = TypeStore::new();
        let first_type = types.builtin(BuiltinType::I32);
        let second_type = types.builtin(BuiltinType::U32);
        let arguments = GenericArguments::new([
            GenericArgument::new(second, second_type),
            GenericArgument::new(first, first_type),
        ])
        .unwrap();

        assert_eq!(arguments.as_slice()[0].parameter(), first);
        assert_eq!(arguments.get(second), Some(second_type));
        assert_eq!(
            GenericArguments::new([
                GenericArgument::new(first, first_type),
                GenericArgument::new(first, second_type),
            ])
            .unwrap_err()
            .parameter(),
            first
        );
    }
}
