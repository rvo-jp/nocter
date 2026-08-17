use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_model::{CallableId, CompilationTarget};

/// One compiler-owned primitive contract in canonical registry order.
///
/// The enum is deliberately closed. Discovery may attach a semantic callable to a role, but it
/// cannot add a backend operation or turn an arbitrary standard-library spelling into one.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PrimitiveRole {
    NewError,
    CurrentAllocatorState,
    CurrentAllocatorKind,
    AllocationAbort,
    PointerAddress,
    PointerFromReference,
    PointerFromReadWriteReference,
    PointerFromAddress,
    PointeeSize,
    PointeeAlignment,
    CopyStringToPointer,
    CopyPointerToPointer,
    StoreByteToPointer,
    StoreValueToPointer,
    DropValueAtPointer,
    TakeValueAtPointer,
    StringFromRawParts,
    ByteSliceFromRawParts,
    MutableByteSliceFromRawParts,
    ValueSliceFromRawParts,
    MutableValueSliceFromRawParts,
    BytesFromString,
    StringSubviewUnchecked,
    SliceLength,
    SlicePointerAddress,
    StringLength,
    StringPointerAddress,
    ProcessExit,
    ProcessArgumentCount,
    ProcessArgument,
    ProcessEnvironmentCount,
    ProcessEnvironmentName,
    ProcessEnvironmentValue,
    OpenRead,
    CreateFile,
    AppendFile,
    WriteText,
    WriteBytes,
    ReadBytes,
    CloseFileDescriptor,
    Syscall0,
    Syscall1,
    Syscall2,
    Syscall3,
    Syscall4,
    Syscall5,
    Syscall6,
    Trap,
    Unreachable,
}

impl PrimitiveRole {
    pub const ALL: &'static [Self] = &[
        Self::NewError,
        Self::CurrentAllocatorState,
        Self::CurrentAllocatorKind,
        Self::AllocationAbort,
        Self::PointerAddress,
        Self::PointerFromReference,
        Self::PointerFromReadWriteReference,
        Self::PointerFromAddress,
        Self::PointeeSize,
        Self::PointeeAlignment,
        Self::CopyStringToPointer,
        Self::CopyPointerToPointer,
        Self::StoreByteToPointer,
        Self::StoreValueToPointer,
        Self::DropValueAtPointer,
        Self::TakeValueAtPointer,
        Self::StringFromRawParts,
        Self::ByteSliceFromRawParts,
        Self::MutableByteSliceFromRawParts,
        Self::ValueSliceFromRawParts,
        Self::MutableValueSliceFromRawParts,
        Self::BytesFromString,
        Self::StringSubviewUnchecked,
        Self::SliceLength,
        Self::SlicePointerAddress,
        Self::StringLength,
        Self::StringPointerAddress,
        Self::ProcessExit,
        Self::ProcessArgumentCount,
        Self::ProcessArgument,
        Self::ProcessEnvironmentCount,
        Self::ProcessEnvironmentName,
        Self::ProcessEnvironmentValue,
        Self::OpenRead,
        Self::CreateFile,
        Self::AppendFile,
        Self::WriteText,
        Self::WriteBytes,
        Self::ReadBytes,
        Self::CloseFileDescriptor,
        Self::Syscall0,
        Self::Syscall1,
        Self::Syscall2,
        Self::Syscall3,
        Self::Syscall4,
        Self::Syscall5,
        Self::Syscall6,
        Self::Trap,
        Self::Unreachable,
    ];

    /// Returns the target gate required by this registry role.
    #[must_use]
    pub const fn target(self) -> Option<CompilationTarget> {
        match self {
            Self::ProcessExit
            | Self::ProcessArgumentCount
            | Self::ProcessArgument
            | Self::ProcessEnvironmentCount
            | Self::ProcessEnvironmentName
            | Self::ProcessEnvironmentValue
            | Self::OpenRead
            | Self::CreateFile
            | Self::AppendFile
            | Self::WriteText
            | Self::WriteBytes
            | Self::ReadBytes
            | Self::CloseFileDescriptor
            | Self::Syscall0
            | Self::Syscall1
            | Self::Syscall2
            | Self::Syscall3
            | Self::Syscall4
            | Self::Syscall5
            | Self::Syscall6
            | Self::Trap
            | Self::Unreachable => Some(CompilationTarget::Arm64Darwin),
            Self::NewError
            | Self::CurrentAllocatorState
            | Self::CurrentAllocatorKind
            | Self::AllocationAbort
            | Self::PointerAddress
            | Self::PointerFromReference
            | Self::PointerFromReadWriteReference
            | Self::PointerFromAddress
            | Self::PointeeSize
            | Self::PointeeAlignment
            | Self::CopyStringToPointer
            | Self::CopyPointerToPointer
            | Self::StoreByteToPointer
            | Self::StoreValueToPointer
            | Self::DropValueAtPointer
            | Self::TakeValueAtPointer
            | Self::StringFromRawParts
            | Self::ByteSliceFromRawParts
            | Self::MutableByteSliceFromRawParts
            | Self::ValueSliceFromRawParts
            | Self::MutableValueSliceFromRawParts
            | Self::BytesFromString
            | Self::StringSubviewUnchecked
            | Self::SliceLength
            | Self::SlicePointerAddress
            | Self::StringLength
            | Self::StringPointerAddress => None,
        }
    }

    /// Returns the canonical standard-package module path owned by this role.
    #[must_use]
    pub fn module_path(self) -> &'static [&'static str] {
        crate::primitive_contracts::primitive_location(self).0
    }

    /// Returns the canonical declaration name owned by this role.
    #[must_use]
    pub fn declaration_name(self) -> &'static str {
        crate::primitive_contracts::primitive_location(self).1
    }
}

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
