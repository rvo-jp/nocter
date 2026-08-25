use std::fmt;

use nocter_compile_input::PrimitiveRoleInput;
use nocter_frontend_bindings::{FrontendBindings, FrontendDeclaration};
use nocter_runtime_contract::{
    PrimitiveBinding, PrimitiveBindingError, PrimitiveRegistry, PrimitiveRole,
};

/// Resolves discovery-selected primitive declaration tokens while declaration lowering owns the
/// exact syntax-to-semantic projection.
///
/// # Errors
///
/// Returns an exact projection failure or an incomplete/ambiguous registry failure.
pub fn resolve_primitive_bindings(
    inputs: &[PrimitiveRoleInput],
    frontend_bindings: &FrontendBindings,
) -> Result<PrimitiveRegistry, PrimitiveResolutionError> {
    let bindings = inputs
        .iter()
        .copied()
        .map(|input| resolve_binding(input, frontend_bindings))
        .collect::<Result<Vec<_>, _>>()?;
    PrimitiveRegistry::new(bindings).map_err(PrimitiveResolutionError::Registry)
}

fn resolve_binding(
    input: PrimitiveRoleInput,
    frontend_bindings: &FrontendBindings,
) -> Result<PrimitiveBinding, PrimitiveResolutionError> {
    let matches = frontend_bindings.declarations(input.declaration());
    let [declaration] = matches else {
        return Err(if matches.is_empty() {
            PrimitiveResolutionError::MissingDeclaration(input.role())
        } else {
            PrimitiveResolutionError::AmbiguousDeclaration(input.role())
        });
    };
    let FrontendDeclaration::Callable(callable) = declaration else {
        return Err(PrimitiveResolutionError::NotCallable(input.role()));
    };
    Ok(PrimitiveBinding::new(input.role(), *callable))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrimitiveResolutionError {
    MissingDeclaration(PrimitiveRole),
    AmbiguousDeclaration(PrimitiveRole),
    NotCallable(PrimitiveRole),
    Registry(PrimitiveBindingError),
}

impl fmt::Display for PrimitiveResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDeclaration(role) => {
                write!(
                    formatter,
                    "primitive role {role:?} has no declaration binding"
                )
            }
            Self::AmbiguousDeclaration(role) => write!(
                formatter,
                "primitive role {role:?} has multiple declaration bindings"
            ),
            Self::NotCallable(role) => {
                write!(
                    formatter,
                    "primitive role {role:?} does not select a callable"
                )
            }
            Self::Registry(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PrimitiveResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Registry(error) => Some(error),
            Self::MissingDeclaration(_) | Self::AmbiguousDeclaration(_) | Self::NotCallable(_) => {
                None
            }
        }
    }
}
