use std::fmt;

use nocter_compile_input::PrimitiveRoleInput;
use nocter_runtime_contract::{
    PrimitiveBinding, PrimitiveBindingError, PrimitiveRegistry, PrimitiveRole,
};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceRole, SyntaxOrigin};

/// Resolves discovery-selected primitive declaration tokens while declaration lowering owns the
/// exact syntax-to-semantic projection.
///
/// # Errors
///
/// Returns an exact projection failure or an incomplete/ambiguous registry failure.
pub fn resolve_primitive_bindings(
    inputs: &[PrimitiveRoleInput],
    source_index: &SourceIndex,
) -> Result<PrimitiveRegistry, PrimitiveResolutionError> {
    let bindings = inputs
        .iter()
        .copied()
        .map(|input| resolve_binding(input, source_index))
        .collect::<Result<Vec<_>, _>>()?;
    PrimitiveRegistry::new(bindings).map_err(PrimitiveResolutionError::Registry)
}

fn resolve_binding(
    input: PrimitiveRoleInput,
    source_index: &SourceIndex,
) -> Result<PrimitiveBinding, PrimitiveResolutionError> {
    let token = input.declaration();
    let mut matches = source_index
        .bindings_at(token.source(), token.range().start())
        .filter(|binding| {
            binding.role() == SourceRole::Declaration
                && binding.origin().syntax() == SyntaxOrigin::Token(token)
        })
        .map(|binding| binding.entity());
    let Some(entity) = matches.next() else {
        return Err(PrimitiveResolutionError::MissingDeclaration(input.role()));
    };
    if matches.next().is_some() {
        return Err(PrimitiveResolutionError::AmbiguousDeclaration(input.role()));
    }
    let SemanticEntity::Callable(callable) = entity else {
        return Err(PrimitiveResolutionError::NotCallable(input.role()));
    };
    Ok(PrimitiveBinding::new(input.role(), callable))
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
