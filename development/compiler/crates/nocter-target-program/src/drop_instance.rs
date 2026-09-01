use std::fmt;

use nocter_checking::{GenericArguments, SubstitutionError, TypeSubstitution, is_concrete_type};
use nocter_model::{DropId, GenericParameterId, TypeId};

/// The canonical identity of one specialized user-authored drop body.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DropInstanceKey {
    drop: DropId,
    generic_arguments: GenericArguments,
}

impl DropInstanceKey {
    /// Creates one closed drop-body identity in the executable specialization type store.
    ///
    /// # Errors
    ///
    /// Returns a typed failure when the drop declaration or its complete generic domain is
    /// invalid, or when an argument remains symbolic.
    pub(crate) fn new_in(
        specialization: crate::executable::ExecutableSpecialization<'_>,
        drop: DropId,
        generic_arguments: GenericArguments,
    ) -> Result<Self, DropInstanceKeyError> {
        let program = specialization.target();
        let types = specialization.types();
        let declaration = program
            .checked()
            .graph()
            .declarations()
            .drops()
            .get(drop)
            .ok_or(DropInstanceKeyError::UnknownDrop(drop))?;
        let expected = declaration.generic_parameters();
        let actual = generic_arguments
            .as_slice()
            .iter()
            .map(|argument| argument.parameter())
            .collect::<Vec<_>>();
        if actual.as_slice() != expected {
            return Err(DropInstanceKeyError::GenericDomainMismatch {
                drop,
                expected: Box::from(expected),
                actual: actual.into_boxed_slice(),
            });
        }
        for argument in generic_arguments.as_slice() {
            if !is_concrete_type(types, argument.ty())
                .map_err(DropInstanceKeyError::InvalidTypeStore)?
            {
                return Err(DropInstanceKeyError::SymbolicArgument {
                    drop,
                    parameter: argument.parameter(),
                    ty: argument.ty(),
                });
            }
        }
        Ok(Self {
            drop,
            generic_arguments,
        })
    }

    #[must_use]
    pub const fn drop(&self) -> DropId {
        self.drop
    }

    #[must_use]
    pub const fn generic_arguments(&self) -> &GenericArguments {
        &self.generic_arguments
    }

    #[must_use]
    pub fn substitution(&self) -> TypeSubstitution {
        let mut substitution = TypeSubstitution::default();
        for argument in self.generic_arguments.as_slice() {
            substitution.bind_generic(argument.parameter(), argument.ty());
        }
        substitution
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DropInstanceKeyError {
    UnknownDrop(DropId),
    GenericDomainMismatch {
        drop: DropId,
        expected: Box<[GenericParameterId]>,
        actual: Box<[GenericParameterId]>,
    },
    SymbolicArgument {
        drop: DropId,
        parameter: GenericParameterId,
        ty: TypeId,
    },
    InvalidTypeStore(SubstitutionError),
}

impl fmt::Display for DropInstanceKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid drop instance key: {self:?}")
    }
}

impl std::error::Error for DropInstanceKeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidTypeStore(error) => Some(error),
            _ => None,
        }
    }
}
