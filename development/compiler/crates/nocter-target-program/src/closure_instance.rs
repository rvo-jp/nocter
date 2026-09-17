use std::fmt;

use nocter_checking::{
    GenericArguments, SubstitutionError, TypeSubstitution, is_concrete_generic_value,
};
use nocter_model::{BodyId, ClosureId, GenericParameterId, GenericValue};

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
        let application = nocter_model::GenericApplication::new(
            generic_arguments
                .as_slice()
                .iter()
                .map(|argument| argument.value())
                .collect::<Vec<_>>(),
        );
        program
            .checked()
            .graph()
            .declarations()
            .validate_generic_application(&expected, &application)
            .map_err(ClosureInstanceKeyError::InvalidGenericApplication)?;
        for argument in generic_arguments.as_slice() {
            if !is_concrete_generic_value(types, argument.value())
                .map_err(ClosureInstanceKeyError::InvalidTypeStore)?
            {
                return Err(ClosureInstanceKeyError::SymbolicArgument {
                    closure,
                    parameter: argument.parameter(),
                    value: argument.value(),
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
            substitution.bind_value(argument.parameter(), argument.value());
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
    InvalidGenericApplication(nocter_declarations::GenericApplicationError),
    SymbolicArgument {
        closure: ClosureId,
        parameter: GenericParameterId,
        value: GenericValue,
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
            Self::InvalidGenericApplication(error) => Some(error),
            Self::InvalidTypeStore(error) => Some(error),
            _ => None,
        }
    }
}
