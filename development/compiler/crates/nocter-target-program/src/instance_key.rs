use std::fmt;

use nocter_checking::{GenericArguments, SubstitutionError, TypeSubstitution, is_concrete_type};
use nocter_declarations::CallableOwner;
use nocter_model::{CallableId, GenericParameterId, TypeId, TypeStore};

use crate::{ExecutableEntry, TargetProgram};

/// The canonical identity of one concrete callable body specialization.
///
/// A callable inherits generic parameters from its declaration owner. Keeping both owner and
/// callable parameters in one canonical argument set prevents two independently assembled keys
/// from naming the same generated body. Concrete receiver types are deliberately not duplicated
/// in this key: an instance, construction, or conformance target is reconstructed from these
/// arguments and its declaration-owned target type.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableInstanceKey {
    callable: CallableId,
    generic_arguments: GenericArguments,
}

impl CallableInstanceKey {
    /// Creates and validates one concrete specialization identity.
    ///
    /// # Errors
    ///
    /// Returns a typed failure when the callable or its owner is absent, the argument domain does
    /// not exactly match the declaration's complete generic domain, or an argument remains
    /// symbolic.
    pub fn new(
        program: &TargetProgram,
        callable: CallableId,
        generic_arguments: GenericArguments,
    ) -> Result<Self, CallableInstanceKeyError> {
        Self::new_in(
            program,
            program.checked().types(),
            callable,
            generic_arguments,
        )
    }

    /// Creates an identity in an executable specialization type-store fork.
    ///
    /// # Errors
    ///
    /// Returns the same closed validation failures as [`Self::new`]. `types` must preserve the
    /// checked store's identity prefix and may contain additional concrete specialized types.
    pub fn new_in(
        program: &TargetProgram,
        types: &TypeStore,
        callable: CallableId,
        generic_arguments: GenericArguments,
    ) -> Result<Self, CallableInstanceKeyError> {
        let graph = program.checked().graph();
        let declaration = graph
            .declarations()
            .callables()
            .get(callable)
            .ok_or(CallableInstanceKeyError::UnknownCallable(callable))?;
        let expected = graph
            .declarations()
            .callable_generic_domain(callable)
            .ok_or(CallableInstanceKeyError::UnknownOwner {
                callable,
                owner: declaration.owner(),
            })?;
        let actual = generic_arguments
            .as_slice()
            .iter()
            .map(|argument| argument.parameter())
            .collect::<Vec<_>>();
        if actual.as_slice() != expected.as_ref() {
            return Err(CallableInstanceKeyError::GenericDomainMismatch {
                callable,
                expected,
                actual: actual.into_boxed_slice(),
            });
        }
        for argument in generic_arguments.as_slice() {
            let concrete = is_concrete_type(types, argument.ty())
                .map_err(CallableInstanceKeyError::InvalidTypeStore)?;
            if !concrete {
                return Err(CallableInstanceKeyError::SymbolicArgument {
                    callable,
                    parameter: argument.parameter(),
                    ty: argument.ty(),
                });
            }
        }
        Ok(Self {
            callable,
            generic_arguments,
        })
    }

    /// Creates the non-generic specialization selected as a process entry.
    ///
    /// # Errors
    ///
    /// Returns the same closed validation failures as [`Self::new`].
    pub fn for_entry(
        program: &TargetProgram,
        entry: ExecutableEntry,
    ) -> Result<Self, CallableInstanceKeyError> {
        Self::new(program, entry.callable(), GenericArguments::default())
    }

    #[must_use]
    pub const fn callable(&self) -> CallableId {
        self.callable
    }

    #[must_use]
    pub const fn generic_arguments(&self) -> &GenericArguments {
        &self.generic_arguments
    }

    /// Builds the single substitution authority for this specialization.
    #[must_use]
    pub fn substitution(&self) -> TypeSubstitution {
        let mut substitution = TypeSubstitution::default();
        for argument in self.generic_arguments.as_slice() {
            substitution.bind_generic(argument.parameter(), argument.ty());
        }
        substitution
    }

    /// Applies this specialization to a type store fork.
    ///
    /// # Errors
    ///
    /// Returns a substitution failure when `ty` or one of its replacements is invalid.
    pub fn apply_type(
        &self,
        types: &mut TypeStore,
        ty: TypeId,
    ) -> Result<TypeId, SubstitutionError> {
        self.substitution().apply_type(types, ty)
    }
}

/// Failure to construct one canonical callable-specialization identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableInstanceKeyError {
    UnknownCallable(CallableId),
    UnknownOwner {
        callable: CallableId,
        owner: CallableOwner,
    },
    GenericDomainMismatch {
        callable: CallableId,
        expected: Box<[GenericParameterId]>,
        actual: Box<[GenericParameterId]>,
    },
    SymbolicArgument {
        callable: CallableId,
        parameter: GenericParameterId,
        ty: TypeId,
    },
    InvalidTypeStore(SubstitutionError),
}

impl fmt::Display for CallableInstanceKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCallable(_) => {
                formatter.write_str("callable instance names an unknown callable")
            }
            Self::UnknownOwner { .. } => {
                formatter.write_str("callable instance names an unknown declaration owner")
            }
            Self::GenericDomainMismatch { .. } => formatter
                .write_str("callable instance arguments do not match its complete generic domain"),
            Self::SymbolicArgument { .. } => {
                formatter.write_str("callable instance contains a symbolic generic argument")
            }
            Self::InvalidTypeStore(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CallableInstanceKeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidTypeStore(error) => Some(error),
            Self::UnknownCallable(_)
            | Self::UnknownOwner { .. }
            | Self::GenericDomainMismatch { .. }
            | Self::SymbolicArgument { .. } => None,
        }
    }
}
