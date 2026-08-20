use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_compile_input::PrimitiveRoleInput;
pub use nocter_declarations::PrimitiveRole;
use nocter_model::CallableId;
use nocter_source_index::{SemanticEntity, SourceIndex, SourceRole, SyntaxOrigin};

/// The exact semantic declaration attached to one compiler-owned primitive role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrimitiveBinding {
    role: PrimitiveRole,
    callable: CallableId,
}

impl PrimitiveBinding {
    #[must_use]
    pub const fn new(role: PrimitiveRole, callable: CallableId) -> Self {
        Self { role, callable }
    }

    #[must_use]
    pub const fn role(self) -> PrimitiveRole {
        self.role
    }

    #[must_use]
    pub const fn callable(self) -> CallableId {
        self.callable
    }
}

/// A complete, canonical primitive-role attachment selected for one toolchain snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrimitiveRegistry {
    bindings: Box<[PrimitiveBinding]>,
}

impl PrimitiveRegistry {
    /// Resolves exact discovery-selected declaration tokens into a complete primitive registry.
    ///
    /// # Errors
    ///
    /// Returns an exact-resolution error when a token has no unique callable declaration binding,
    /// or a registry error when roles or callable identities are duplicated or incomplete.
    pub fn resolve(
        inputs: &[PrimitiveRoleInput],
        source_index: &SourceIndex,
    ) -> Result<Self, PrimitiveResolutionError> {
        let bindings = inputs
            .iter()
            .copied()
            .map(|input| resolve_binding(input, source_index))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(bindings).map_err(PrimitiveResolutionError::Registry)
    }

    /// Freezes a complete registry in canonical role order.
    ///
    /// # Errors
    ///
    /// Rejects a missing or duplicate role, or one callable attached to multiple roles.
    pub fn new(
        bindings: impl IntoIterator<Item = PrimitiveBinding>,
    ) -> Result<Self, PrimitiveBindingError> {
        let mut by_role = BTreeMap::new();
        let mut callables = BTreeSet::new();
        for binding in bindings {
            if by_role.insert(binding.role(), binding).is_some() {
                return Err(PrimitiveBindingError::DuplicateRole(binding.role()));
            }
            if !callables.insert(binding.callable()) {
                return Err(PrimitiveBindingError::DuplicateCallable(binding.callable()));
            }
        }
        let mut canonical = Vec::with_capacity(PrimitiveRole::ALL.len());
        for role in PrimitiveRole::ALL {
            canonical.push(
                by_role
                    .remove(role)
                    .ok_or(PrimitiveBindingError::MissingRole(*role))?,
            );
        }
        debug_assert!(by_role.is_empty());
        Ok(Self {
            bindings: canonical.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn bindings(&self) -> &[PrimitiveBinding] {
        &self.bindings
    }

    #[must_use]
    pub fn callable(&self, role: PrimitiveRole) -> CallableId {
        self.bindings[role_index(role)].callable()
    }

    #[must_use]
    pub fn role(&self, callable: CallableId) -> Option<PrimitiveRole> {
        self.bindings
            .iter()
            .find(|binding| binding.callable() == callable)
            .map(|binding| binding.role())
    }
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

/// Failure to turn exact primitive source identities into one canonical registry.
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

fn role_index(role: PrimitiveRole) -> usize {
    PrimitiveRole::ALL
        .iter()
        .position(|candidate| *candidate == role)
        .unwrap_or_else(|| unreachable!("closed primitive role is absent from PrimitiveRole::ALL"))
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum PrimitiveBindingError {
    MissingRole(PrimitiveRole),
    DuplicateRole(PrimitiveRole),
    DuplicateCallable(CallableId),
}

impl fmt::Debug for PrimitiveBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRole(role) => formatter.debug_tuple("MissingRole").field(role).finish(),
            Self::DuplicateRole(role) => {
                formatter.debug_tuple("DuplicateRole").field(role).finish()
            }
            Self::DuplicateCallable(callable) => formatter
                .debug_tuple("DuplicateCallable")
                .field(callable)
                .finish(),
        }
    }
}

impl fmt::Display for PrimitiveBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRole(_) => formatter.write_str("primitive registry is missing a role"),
            Self::DuplicateRole(_) => {
                formatter.write_str("primitive registry contains a duplicate role")
            }
            Self::DuplicateCallable(_) => {
                formatter.write_str("primitive registry attaches one callable to multiple roles")
            }
        }
    }
}

impl std::error::Error for PrimitiveBindingError {}

#[cfg(test)]
mod tests {
    use nocter_declarations::DeclarationArenaBuilder;

    use super::{PrimitiveBinding, PrimitiveBindingError, PrimitiveRegistry, PrimitiveRole};

    fn complete_bindings() -> Vec<PrimitiveBinding> {
        let mut declarations = DeclarationArenaBuilder::new();
        PrimitiveRole::ALL
            .iter()
            .copied()
            .map(|role| PrimitiveBinding::new(role, declarations.reserve_callable()))
            .collect()
    }

    #[test]
    fn registry_canonicalizes_complete_reversed_input() {
        let mut bindings = complete_bindings();
        bindings.reverse();
        let registry = PrimitiveRegistry::new(bindings).unwrap();
        assert_eq!(registry.bindings().len(), PrimitiveRole::ALL.len());
        assert!(
            registry
                .bindings()
                .iter()
                .map(|binding| binding.role())
                .eq(PrimitiveRole::ALL.iter().copied())
        );
    }

    #[test]
    fn registry_rejects_missing_duplicate_and_aliased_roles() {
        let mut missing = complete_bindings();
        let removed = missing.pop().unwrap();
        assert_eq!(
            PrimitiveRegistry::new(missing),
            Err(PrimitiveBindingError::MissingRole(removed.role()))
        );

        let mut duplicate = complete_bindings();
        duplicate.push(duplicate[0]);
        assert_eq!(
            PrimitiveRegistry::new(duplicate),
            Err(PrimitiveBindingError::DuplicateRole(
                PrimitiveRole::NewError
            ))
        );

        let mut aliased = complete_bindings();
        aliased[1] = PrimitiveBinding::new(aliased[1].role(), aliased[0].callable());
        assert_eq!(
            PrimitiveRegistry::new(aliased),
            Err(PrimitiveBindingError::DuplicateCallable(
                complete_bindings()[0].callable()
            ))
        );
    }
}
