use std::fmt;

use nocter_model::{GenericApplication, GenericParameterId, GenericValue};

use crate::{DeclarationArenas, GenericParameterDomain};

/// A schema mismatch in one ordered generic application.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenericApplicationError {
    Arity {
        expected: usize,
        actual: usize,
    },
    UnknownParameter(GenericParameterId),
    Domain {
        parameter: GenericParameterId,
        position: usize,
        expected: GenericParameterDomain,
        actual: GenericParameterDomain,
    },
}

impl GenericApplicationError {
    #[must_use]
    pub const fn position(self) -> Option<usize> {
        match self {
            Self::Domain { position, .. } => Some(position),
            Self::Arity { .. } | Self::UnknownParameter(_) => None,
        }
    }
}

impl fmt::Display for GenericApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arity { expected, actual } => write!(
                formatter,
                "generic application has {actual} values but its declaration requires {expected}"
            ),
            Self::UnknownParameter(parameter) => {
                write!(
                    formatter,
                    "generic application names unknown parameter {parameter:?}"
                )
            }
            Self::Domain {
                position,
                expected,
                actual,
                ..
            } => write!(
                formatter,
                "generic argument {position} has domain {actual:?}, expected {expected:?}"
            ),
        }
    }
}

impl std::error::Error for GenericApplicationError {}

impl DeclarationArenas {
    /// Validates one application against an owner-provided ordered parameter schema.
    ///
    /// This is the sole declaration-model authority for generic arity and argument domains.
    /// Callers select the owner and provide its stored parameter sequence; they do not reproduce
    /// domain matching rules.
    ///
    /// # Errors
    ///
    /// Returns the exact arity, missing-parameter, or positional domain mismatch.
    pub fn validate_generic_application(
        &self,
        parameters: &[GenericParameterId],
        application: &GenericApplication,
    ) -> Result<(), GenericApplicationError> {
        if parameters.len() != application.len() {
            return Err(GenericApplicationError::Arity {
                expected: parameters.len(),
                actual: application.len(),
            });
        }
        for (position, (parameter, value)) in parameters
            .iter()
            .copied()
            .zip(application.iter())
            .enumerate()
        {
            let expected = self
                .generic_parameters()
                .get(parameter)
                .ok_or(GenericApplicationError::UnknownParameter(parameter))?
                .domain();
            let actual = match value {
                GenericValue::Type(_) => GenericParameterDomain::Type,
                GenericValue::Usize(_) => GenericParameterDomain::UsizeConstant,
            };
            if expected != actual {
                return Err(GenericApplicationError::Domain {
                    parameter,
                    position,
                    expected,
                    actual,
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::{
        ArenaBuilder, BuiltinType, GenericApplication, GenericParameterId, GenericValue,
        NominalTypeId, SymbolTable, TypeStore,
    };

    use crate::{DeclarationArenaBuilder, GenericOwner, GenericParameter, GenericParameterDomain};

    use super::GenericApplicationError;

    #[test]
    fn validates_ordered_type_and_constant_domains() {
        let mut owners = ArenaBuilder::<NominalTypeId, _>::new();
        let owner = GenericOwner::NominalType(owners.insert(()));
        let _ = owners.finish();
        let symbols = SymbolTable::from_spellings(["N", "T"]);
        let mut declarations = DeclarationArenaBuilder::new();
        let ty = declarations.add_generic_parameter(GenericParameter::new(
            owner,
            symbols.get("T").unwrap(),
            0,
            GenericParameterDomain::Type,
        ));
        let size = declarations.add_generic_parameter(GenericParameter::new(
            owner,
            symbols.get("N").unwrap(),
            1,
            GenericParameterDomain::UsizeConstant,
        ));
        let declarations = declarations.finish().unwrap();
        let application = GenericApplication::new([
            GenericValue::Type(TypeStore::new().builtin(BuiltinType::U8)),
            GenericValue::Usize(16.into()),
        ]);

        assert_eq!(
            declarations.validate_generic_application(&[ty, size], &application),
            Ok(())
        );
        let reversed = GenericApplication::new([
            GenericValue::Usize(16.into()),
            GenericValue::Type(TypeStore::new().builtin(BuiltinType::U8)),
        ]);
        assert!(matches!(
            declarations.validate_generic_application(&[ty, size], &reversed),
            Err(GenericApplicationError::Domain {
                parameter,
                position: 0,
                expected: GenericParameterDomain::Type,
                actual: GenericParameterDomain::UsizeConstant,
            }) if parameter == ty
        ));
    }

    #[test]
    fn rejects_arity_before_inspecting_values() {
        let mut parameters = ArenaBuilder::<GenericParameterId, _>::new();
        let unknown = parameters.insert(());
        let _ = parameters.finish();
        let declarations = DeclarationArenaBuilder::new().finish().unwrap();

        assert_eq!(
            declarations.validate_generic_application(&[unknown], &GenericApplication::default()),
            Err(GenericApplicationError::Arity {
                expected: 1,
                actual: 0,
            })
        );
    }
}
