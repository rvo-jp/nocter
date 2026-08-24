use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_model::CallableId;

/// Compiler-defined meaning assigned to one exact bodyless standard callable.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PrimitiveRole {
    NewError,
    ErrorContext,
    ErrorCode,
    ErrorMessage,
    AllocationFailureError,
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
        Self::ErrorContext,
        Self::ErrorCode,
        Self::ErrorMessage,
        Self::AllocationFailureError,
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

    const fn index(self) -> usize {
        self as usize
    }
}

/// The exact semantic callable attached to one compiler-owned primitive role.
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

/// A complete primitive-role attachment in canonical role order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrimitiveRegistry {
    bindings: Box<[PrimitiveBinding]>,
}

impl PrimitiveRegistry {
    /// Freezes one complete registry.
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
        self.bindings[role.index()].callable()
    }

    #[must_use]
    pub fn role(&self, callable: CallableId) -> Option<PrimitiveRole> {
        self.bindings
            .iter()
            .find(|binding| binding.callable() == callable)
            .map(|binding| binding.role())
    }
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
    use nocter_model::{ArenaBuilder, CallableId};

    use super::{PrimitiveBinding, PrimitiveBindingError, PrimitiveRegistry, PrimitiveRole};

    fn complete_bindings() -> Vec<PrimitiveBinding> {
        let mut callables = ArenaBuilder::<CallableId, ()>::new();
        PrimitiveRole::ALL
            .iter()
            .copied()
            .map(|role| PrimitiveBinding::new(role, callables.insert(())))
            .collect()
    }

    #[test]
    fn registry_canonicalizes_and_rejects_incomplete_domains() {
        let mut reversed = complete_bindings();
        reversed.reverse();
        let registry = PrimitiveRegistry::new(reversed).unwrap();
        assert!(
            registry
                .bindings()
                .iter()
                .map(|binding| binding.role())
                .eq(PrimitiveRole::ALL.iter().copied())
        );

        let mut missing = complete_bindings();
        let removed = missing.pop().unwrap();
        assert_eq!(
            PrimitiveRegistry::new(missing),
            Err(PrimitiveBindingError::MissingRole(removed.role()))
        );
    }
}
