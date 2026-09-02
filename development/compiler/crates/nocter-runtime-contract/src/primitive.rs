use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_model::CallableId;

/// Runtime effects certified by the compiler for one closed primitive role.
///
/// This is positive implementation evidence, not source syntax. New primitive roles must state
/// their behavior here before an authored guarantee can rely on them.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PrimitiveEffects {
    may_allocate: bool,
}

impl PrimitiveEffects {
    #[must_use]
    pub const fn may_allocate(self) -> bool {
        self.may_allocate
    }
}

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
    U8Truncate,
    U16Truncate,
    U32Truncate,
    I8Truncate,
    I16Truncate,
    I32Truncate,
    U64WrappingAdd,
    U64WrappingMultiply,
    U64BitwiseXor,
    U64RotateRight,
    ProcessExit,
    ProcessArgumentCount,
    ProcessArgument,
    ProcessEnvironmentCount,
    ProcessEnvironmentName,
    ProcessEnvironmentValue,
    MonotonicCounterRead,
    MonotonicCounterFrequency,
    MonotonicCounterDelta,
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
        Self::U8Truncate,
        Self::U16Truncate,
        Self::U32Truncate,
        Self::I8Truncate,
        Self::I16Truncate,
        Self::I32Truncate,
        Self::U64WrappingAdd,
        Self::U64WrappingMultiply,
        Self::U64BitwiseXor,
        Self::U64RotateRight,
        Self::ProcessExit,
        Self::ProcessArgumentCount,
        Self::ProcessArgument,
        Self::ProcessEnvironmentCount,
        Self::ProcessEnvironmentName,
        Self::ProcessEnvironmentValue,
        Self::MonotonicCounterRead,
        Self::MonotonicCounterFrequency,
        Self::MonotonicCounterDelta,
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

    /// Returns the stable compiler-contract name of this primitive role.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::NewError => "new_error",
            Self::ErrorContext => "error_context",
            Self::ErrorCode => "error_code",
            Self::ErrorMessage => "error_message",
            Self::AllocationFailureError => "allocation_failure_error",
            Self::CurrentAllocatorState => "current_allocator_state",
            Self::CurrentAllocatorKind => "current_allocator_kind",
            Self::AllocationAbort => "allocation_abort",
            Self::PointerAddress => "pointer_address",
            Self::PointerFromReference => "pointer_from_reference",
            Self::PointerFromReadWriteReference => "pointer_from_read_write_reference",
            Self::PointerFromAddress => "pointer_from_address",
            Self::PointeeSize => "pointee_size",
            Self::PointeeAlignment => "pointee_alignment",
            Self::CopyStringToPointer => "copy_string_to_pointer",
            Self::CopyPointerToPointer => "copy_pointer_to_pointer",
            Self::StoreByteToPointer => "store_byte_to_pointer",
            Self::StoreValueToPointer => "store_value_to_pointer",
            Self::DropValueAtPointer => "drop_value_at_pointer",
            Self::TakeValueAtPointer => "take_value_at_pointer",
            Self::StringFromRawParts => "string_from_raw_parts",
            Self::ByteSliceFromRawParts => "byte_slice_from_raw_parts",
            Self::MutableByteSliceFromRawParts => "mutable_byte_slice_from_raw_parts",
            Self::ValueSliceFromRawParts => "value_slice_from_raw_parts",
            Self::MutableValueSliceFromRawParts => "mutable_value_slice_from_raw_parts",
            Self::BytesFromString => "bytes_from_string",
            Self::StringSubviewUnchecked => "string_subview_unchecked",
            Self::SliceLength => "slice_length",
            Self::SlicePointerAddress => "slice_pointer_address",
            Self::StringLength => "string_length",
            Self::StringPointerAddress => "string_pointer_address",
            Self::U8Truncate => "u8_truncate",
            Self::U16Truncate => "u16_truncate",
            Self::U32Truncate => "u32_truncate",
            Self::I8Truncate => "i8_truncate",
            Self::I16Truncate => "i16_truncate",
            Self::I32Truncate => "i32_truncate",
            Self::U64WrappingAdd => "u64_wrapping_add",
            Self::U64WrappingMultiply => "u64_wrapping_multiply",
            Self::U64BitwiseXor => "u64_bitwise_xor",
            Self::U64RotateRight => "u64_rotate_right",
            Self::ProcessExit => "process_exit",
            Self::ProcessArgumentCount => "process_argument_count",
            Self::ProcessArgument => "process_argument",
            Self::ProcessEnvironmentCount => "process_environment_count",
            Self::ProcessEnvironmentName => "process_environment_name",
            Self::ProcessEnvironmentValue => "process_environment_value",
            Self::MonotonicCounterRead => "monotonic_counter_read",
            Self::MonotonicCounterFrequency => "monotonic_counter_frequency",
            Self::MonotonicCounterDelta => "monotonic_counter_delta",
            Self::Syscall0 => "syscall_0",
            Self::Syscall1 => "syscall_1",
            Self::Syscall2 => "syscall_2",
            Self::Syscall3 => "syscall_3",
            Self::Syscall4 => "syscall_4",
            Self::Syscall5 => "syscall_5",
            Self::Syscall6 => "syscall_6",
            Self::Trap => "trap",
            Self::Unreachable => "unreachable",
        }
    }

    /// Returns compiler-owned effect evidence for this closed primitive role.
    #[must_use]
    pub const fn effects(self) -> PrimitiveEffects {
        // Most current roles manipulate existing storage, expose runtime context, or terminate
        // execution. Generic destruction is conservative because its selected type-owned drop may
        // request storage. Keeping this decision on the closed role—not on source spelling—makes
        // future effectful primitives opt into the fact explicitly.
        PrimitiveEffects {
            may_allocate: matches!(self, Self::DropValueAtPointer),
        }
    }

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

    #[test]
    fn generic_destruction_is_the_only_conservative_allocation_effect() {
        let effectful = PrimitiveRole::ALL
            .iter()
            .copied()
            .filter(|role| role.effects().may_allocate())
            .collect::<Vec<_>>();
        assert_eq!(effectful, vec![PrimitiveRole::DropValueAtPointer]);
    }
}
