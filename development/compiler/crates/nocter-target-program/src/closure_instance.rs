use std::fmt;

use nocter_checking::{GenericArguments, SubstitutionError, TypeSubstitution, is_concrete_type};
use nocter_model::{BodyId, ClosureId, GenericParameterId, TypeId};

/// The canonical identity of one specialized anonymous closure body and environment.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ClosureInstanceKey {
    closure: ClosureId,
    generic_arguments: GenericArguments,
}

impl ClosureInstanceKey {
    /// Creates one closed closure identity in the executable specialization type store.
    ///
    /// # Errors
    ///
    /// Returns a typed failure when the closure, its owner body, or its complete generic domain is
    /// invalid, or when an argument remains symbolic.
    pub(crate) fn new_in(
        specialization: crate::executable::ExecutableSpecialization<'_>,
        closure: ClosureId,
        generic_arguments: GenericArguments,
    ) -> Result<Self, ClosureInstanceKeyError> {
        let program = specialization.target();
        let types = specialization.types();
        let definition = program
            .checked()
            .closures()
            .get(closure)
            .ok_or(ClosureInstanceKeyError::UnknownClosure(closure))?;
        let body = definition.owner();
        let expected = program
            .checked()
            .graph()
            .declarations()
            .body_generic_domain(body)
            .ok_or(ClosureInstanceKeyError::UnknownOwnerBody { closure, body })?;
        let actual = generic_arguments
            .as_slice()
            .iter()
            .map(|argument| argument.parameter())
            .collect::<Vec<_>>();
        if actual.as_slice() != expected.as_ref() {
            return Err(ClosureInstanceKeyError::GenericDomainMismatch {
                closure,
                expected,
                actual: actual.into_boxed_slice(),
            });
        }
        for argument in generic_arguments.as_slice() {
            if !is_concrete_type(types, argument.ty())
                .map_err(ClosureInstanceKeyError::InvalidTypeStore)?
            {
                return Err(ClosureInstanceKeyError::SymbolicArgument {
                    closure,
                    parameter: argument.parameter(),
                    ty: argument.ty(),
                });
            }
        }
        Ok(Self {
            closure,
            generic_arguments,
        })
    }

    #[must_use]
    pub const fn closure(&self) -> ClosureId {
        self.closure
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
pub enum ClosureInstanceKeyError {
    UnknownClosure(ClosureId),
    UnknownOwnerBody {
        closure: ClosureId,
        body: BodyId,
    },
    GenericDomainMismatch {
        closure: ClosureId,
        expected: Box<[GenericParameterId]>,
        actual: Box<[GenericParameterId]>,
    },
    SymbolicArgument {
        closure: ClosureId,
        parameter: GenericParameterId,
        ty: TypeId,
    },
    InvalidTypeStore(SubstitutionError),
}

impl fmt::Display for ClosureInstanceKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid closure instance key: {self:?}")
    }
}

impl std::error::Error for ClosureInstanceKeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidTypeStore(error) => Some(error),
            _ => None,
        }
    }
}
