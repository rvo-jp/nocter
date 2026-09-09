use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_model::NominalTypeId;

/// Closed compiler-owned storage representations attached to exact standard declarations.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeStorageRole {
    /// Fixed native owner shared by Network.framework connections and listeners.
    NetworkOwner,
}

impl RuntimeStorageRole {
    /// Every storage role required by this compiler release.
    pub const ALL: &'static [Self] = &[Self::NetworkOwner];

    /// Returns the size and alignment owned by the runtime ABI contract.
    #[must_use]
    pub const fn layout(self, abi: super::RuntimeAbiIdentity) -> Option<RuntimeStorageLayout> {
        match (self, abi) {
            (Self::NetworkOwner, super::RuntimeAbiIdentity::Arm64DarwinV1) => {
                Some(RuntimeStorageLayout::new(
                    super::DarwinNetworkOwnerAbiSchema::ARM64_DARWIN.size(),
                    super::DarwinNetworkOwnerAbiSchema::ARM64_DARWIN.alignment(),
                ))
            }
        }
    }
}

/// Target storage dimensions selected by one closed runtime role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeStorageLayout {
    size: u64,
    alignment: u64,
}

impl RuntimeStorageLayout {
    #[must_use]
    pub const fn new(size: u64, alignment: u64) -> Self {
        Self { size, alignment }
    }

    #[must_use]
    pub const fn size(self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn alignment(self) -> u64 {
        self.alignment
    }
}

/// One exact semantic declaration selected for a compiler-owned storage role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeStorageBinding {
    role: RuntimeStorageRole,
    declaration: NominalTypeId,
}

impl RuntimeStorageBinding {
    #[must_use]
    pub const fn new(role: RuntimeStorageRole, declaration: NominalTypeId) -> Self {
        Self { role, declaration }
    }

    #[must_use]
    pub const fn role(self) -> RuntimeStorageRole {
        self.role
    }

    #[must_use]
    pub const fn declaration(self) -> NominalTypeId {
        self.declaration
    }
}

/// Complete one-to-one attachment of runtime storage roles to semantic declarations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStorageRegistry {
    bindings: Box<[RuntimeStorageBinding]>,
}

impl RuntimeStorageRegistry {
    /// An explicitly unavailable runtime-storage capability set.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            bindings: Box::new([]),
        }
    }

    /// Freezes a complete registry in canonical role order.
    ///
    /// # Errors
    ///
    /// Rejects missing or duplicate roles and declarations selected for more than one role.
    pub fn new(
        bindings: impl IntoIterator<Item = RuntimeStorageBinding>,
    ) -> Result<Self, RuntimeStorageBindingError> {
        let mut by_role = BTreeMap::new();
        let mut declarations = BTreeSet::new();
        for binding in bindings {
            if by_role.insert(binding.role(), binding).is_some() {
                return Err(RuntimeStorageBindingError::DuplicateRole(binding.role()));
            }
            if !declarations.insert(binding.declaration()) {
                return Err(RuntimeStorageBindingError::DuplicateDeclaration(
                    binding.declaration(),
                ));
            }
        }
        if by_role.is_empty() {
            return Ok(Self::empty());
        }
        let mut canonical = Vec::with_capacity(RuntimeStorageRole::ALL.len());
        for role in RuntimeStorageRole::ALL {
            let binding = by_role
                .remove(role)
                .ok_or(RuntimeStorageBindingError::MissingRole(*role))?;
            canonical.push(binding);
        }
        debug_assert!(by_role.is_empty());
        Ok(Self {
            bindings: canonical.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn bindings(&self) -> &[RuntimeStorageBinding] {
        &self.bindings
    }

    #[must_use]
    pub fn declaration(&self, role: RuntimeStorageRole) -> Option<NominalTypeId> {
        self.bindings
            .get(role as usize)
            .filter(|binding| binding.role() == role)
            .map(|binding| binding.declaration())
    }

    #[must_use]
    pub fn role(&self, declaration: NominalTypeId) -> Option<RuntimeStorageRole> {
        self.bindings
            .iter()
            .find(|binding| binding.declaration() == declaration)
            .map(|binding| binding.role())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeStorageBindingError {
    MissingRole(RuntimeStorageRole),
    DuplicateRole(RuntimeStorageRole),
    DuplicateDeclaration(NominalTypeId),
}

impl fmt::Display for RuntimeStorageBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid runtime storage registry: {self:?}")
    }
}

impl std::error::Error for RuntimeStorageBindingError {}

#[cfg(test)]
mod tests {
    use nocter_model::ArenaBuilder;

    use super::{RuntimeStorageBinding, RuntimeStorageRegistry, RuntimeStorageRole};

    #[test]
    fn registry_requires_one_distinct_declaration_per_closed_role() {
        let mut declarations = ArenaBuilder::new();
        let declaration = declarations.insert(());
        let registry = RuntimeStorageRegistry::new([RuntimeStorageBinding::new(
            RuntimeStorageRole::NetworkOwner,
            declaration,
        )])
        .unwrap();
        assert_eq!(
            registry.role(declaration),
            Some(RuntimeStorageRole::NetworkOwner)
        );
        assert_eq!(
            registry.declaration(RuntimeStorageRole::NetworkOwner),
            Some(declaration)
        );
        assert_eq!(
            RuntimeStorageRegistry::new([]).unwrap(),
            RuntimeStorageRegistry::empty()
        );
    }
}
