use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_model::CallableId;

macro_rules! closed_role_enum {
    (
        $(#[$enum_attribute:meta])*
        pub enum $name:ident {
            $($(#[$variant_attribute:meta])* $variant:ident),+ $(,)?
        }
    ) => {
        $(#[$enum_attribute])*
        pub enum $name {
            $($(#[$variant_attribute])* $variant),+
        }

        impl $name {
            /// Every member of this closed role vocabulary in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
        }
    };
}

/// Runtime effects certified by the compiler for one closed primitive role.
///
/// This is positive implementation evidence, not source syntax. New primitive roles must state
/// their behavior here before an authored guarantee can rely on them.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PrimitiveEffects {
    may_allocate: bool,
    may_block: bool,
    returns_drive_safe_future: bool,
}

impl PrimitiveEffects {
    #[must_use]
    pub const fn may_allocate(self) -> bool {
        self.may_allocate
    }

    /// Whether invocation may synchronously wait for progress outside the current thread.
    #[must_use]
    pub const fn may_block(self) -> bool {
        self.may_block
    }

    /// Whether a primitive returning `future T` certifies every drive and cancellation entry as
    /// nonblocking.
    #[must_use]
    pub const fn returns_drive_safe_future(self) -> bool {
        self.returns_drive_safe_future
    }
}

closed_role_enum! {
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
        /// Requests one private anonymous page mapping through a target-owned ABI operation.
        MemoryMap,
        /// Releases one page mapping through a target-owned ABI operation.
        MemoryUnmap,
        /// Closes one Darwin descriptor without exposing the raw syscall-number boundary.
        DescriptorClose,
        /// Fills one 64-bit hash seed from the target entropy source.
        EntropySeedFill,
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
        CharacterFromU32Unchecked,
        CharacterCodePoint,
        U8Truncate,
        U16Truncate,
        U32Truncate,
        I8Truncate,
        I16Truncate,
        I32Truncate,
        F32FromBits,
        F32ToBits,
        F64FromBits,
        F64ToBits,
        F32Floor,
        F32Ceil,
        F32Trunc,
        F32RoundTiesEven,
        F64Floor,
        F64Ceil,
        F64Trunc,
        F64RoundTiesEven,
        F64ToF32,
        F64ToI64,
        F64ToU64,
        U64WrappingAdd,
        U64WrappingSubtract,
        U64WrappingMultiply,
        U64MultiplyHigh,
        U64BitwiseAnd,
        U64BitwiseOr,
        U64BitwiseXor,
        U64RotateRight,
        U64LeadingZeros,
        ProcessExit,
        ProcessArgumentCount,
        ProcessArgument,
        ProcessEnvironmentCount,
        ProcessEnvironmentName,
        ProcessEnvironmentValue,
        /// Performs one instruction-ordered observation of the current 64-bit value from a process
        /// monotonic-counter domain.
        MonotonicCounterRead,
        /// Reads that domain's fixed, non-zero ticks-per-second value, which must fit in `u32`.
        MonotonicCounterFrequency,
        /// Computes `later - earlier` in the counter's wrapping 64-bit domain.
        MonotonicCounterDelta,
        /// Creates one lazy computation that becomes completable after descriptor readiness.
        DescriptorReadiness,
        /// Creates one lazy computation that becomes completable after descriptor readiness or a
        /// monotonic deadline, whichever the reactor observes first.
        DescriptorReadinessOrDeadline,
        /// Creates one lazy computation that becomes completable at a monotonic deadline.
        MonotonicDeadline,
        /// Takes ownership of two lazy computations and produces both outputs concurrently.
        TaskJoin,
        NetworkConnectionCreate,
        NetworkTlsConnectionCreate,
        NetworkTlsConnectionMatchesApplicationProtocol,
        NetworkConnectionStart,
        NetworkConnectionEventDescriptor,
        NetworkConnectionBeginReceive,
        NetworkConnectionBeginSend,
        NetworkConnectionReceiveEvent,
        NetworkConnectionCopyLocalAddress,
        NetworkConnectionCopyRemoteAddress,
        NetworkConnectionRequestCancel,
        NetworkConnectionReleaseBarrier,
        NetworkConnectionRelease,
        NetworkListenerCreate,
        NetworkListenerStart,
        NetworkListenerEventDescriptor,
        NetworkListenerReceiveEvent,
        NetworkListenerPort,
        NetworkListenerRequestCancel,
        NetworkListenerReleaseBarrier,
        NetworkListenerRelease,
        Syscall0,
        /// Preserves both successful result words of one zero-argument target syscall.
        SyscallPair0,
        Syscall1,
        Syscall2,
        Syscall3,
        Syscall4,
        Syscall5,
        Syscall6,
        Trap,
        Unreachable,
    }
}

impl PrimitiveRole {
    /// Returns the stable compiler-contract name of this primitive role.
    #[must_use]
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive closed mapping keeps primitive role names as a single authority"
    )]
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
            Self::MemoryMap => "memory_map",
            Self::MemoryUnmap => "memory_unmap",
            Self::DescriptorClose => "descriptor_close",
            Self::EntropySeedFill => "entropy_seed_fill",
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
            Self::CharacterFromU32Unchecked => "character_from_u32_unchecked",
            Self::CharacterCodePoint => "character_code_point",
            Self::U8Truncate => "u8_truncate",
            Self::U16Truncate => "u16_truncate",
            Self::U32Truncate => "u32_truncate",
            Self::I8Truncate => "i8_truncate",
            Self::I16Truncate => "i16_truncate",
            Self::I32Truncate => "i32_truncate",
            Self::F32FromBits => "f32_from_bits",
            Self::F32ToBits => "f32_to_bits",
            Self::F64FromBits => "f64_from_bits",
            Self::F64ToBits => "f64_to_bits",
            Self::F32Floor => "f32_floor",
            Self::F32Ceil => "f32_ceil",
            Self::F32Trunc => "f32_trunc",
            Self::F32RoundTiesEven => "f32_round_ties_even",
            Self::F64Floor => "f64_floor",
            Self::F64Ceil => "f64_ceil",
            Self::F64Trunc => "f64_trunc",
            Self::F64RoundTiesEven => "f64_round_ties_even",
            Self::F64ToF32 => "f64_to_f32",
            Self::F64ToI64 => "f64_to_i64",
            Self::F64ToU64 => "f64_to_u64",
            Self::U64WrappingAdd => "u64_wrapping_add",
            Self::U64WrappingSubtract => "u64_wrapping_subtract",
            Self::U64WrappingMultiply => "u64_wrapping_multiply",
            Self::U64MultiplyHigh => "u64_multiply_high",
            Self::U64BitwiseAnd => "u64_bitwise_and",
            Self::U64BitwiseOr => "u64_bitwise_or",
            Self::U64BitwiseXor => "u64_bitwise_xor",
            Self::U64RotateRight => "u64_rotate_right",
            Self::U64LeadingZeros => "u64_leading_zeros",
            Self::ProcessExit => "process_exit",
            Self::ProcessArgumentCount => "process_argument_count",
            Self::ProcessArgument => "process_argument",
            Self::ProcessEnvironmentCount => "process_environment_count",
            Self::ProcessEnvironmentName => "process_environment_name",
            Self::ProcessEnvironmentValue => "process_environment_value",
            Self::MonotonicCounterRead => "monotonic_counter_read",
            Self::MonotonicCounterFrequency => "monotonic_counter_frequency",
            Self::MonotonicCounterDelta => "monotonic_counter_delta",
            Self::DescriptorReadiness => "descriptor_readiness",
            Self::DescriptorReadinessOrDeadline => "descriptor_readiness_or_deadline",
            Self::MonotonicDeadline => "monotonic_deadline",
            Self::TaskJoin => "task_join",
            Self::NetworkConnectionCreate
            | Self::NetworkTlsConnectionCreate
            | Self::NetworkTlsConnectionMatchesApplicationProtocol
            | Self::NetworkConnectionStart
            | Self::NetworkConnectionEventDescriptor
            | Self::NetworkConnectionBeginReceive
            | Self::NetworkConnectionBeginSend
            | Self::NetworkConnectionReceiveEvent
            | Self::NetworkConnectionCopyLocalAddress
            | Self::NetworkConnectionCopyRemoteAddress
            | Self::NetworkConnectionRequestCancel
            | Self::NetworkConnectionReleaseBarrier
            | Self::NetworkConnectionRelease => self.network_connection_name(),
            Self::NetworkListenerCreate
            | Self::NetworkListenerStart
            | Self::NetworkListenerEventDescriptor
            | Self::NetworkListenerReceiveEvent
            | Self::NetworkListenerPort
            | Self::NetworkListenerRequestCancel
            | Self::NetworkListenerReleaseBarrier
            | Self::NetworkListenerRelease => self.network_listener_name(),
            Self::Syscall0 => "syscall_0",
            Self::SyscallPair0 => "syscall_pair_0",
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

    const fn network_listener_name(self) -> &'static str {
        match self {
            Self::NetworkListenerCreate => "network_listener_create",
            Self::NetworkListenerStart => "network_listener_start",
            Self::NetworkListenerEventDescriptor => "network_listener_event_descriptor",
            Self::NetworkListenerReceiveEvent => "network_listener_receive_event",
            Self::NetworkListenerPort => "network_listener_port",
            Self::NetworkListenerRequestCancel => "network_listener_request_cancel",
            Self::NetworkListenerReleaseBarrier => "network_listener_release_barrier",
            Self::NetworkListenerRelease => "network_listener_release",
            _ => panic!("only listener roles use listener names"),
        }
    }

    const fn network_connection_name(self) -> &'static str {
        match self {
            Self::NetworkConnectionCreate => "network_connection_create",
            Self::NetworkTlsConnectionCreate => "network_tls_connection_create",
            Self::NetworkTlsConnectionMatchesApplicationProtocol => {
                "network_tls_connection_matches_application_protocol"
            }
            Self::NetworkConnectionStart => "network_connection_start",
            Self::NetworkConnectionEventDescriptor => "network_connection_event_descriptor",
            Self::NetworkConnectionBeginReceive => "network_connection_begin_receive",
            Self::NetworkConnectionBeginSend => "network_connection_begin_send",
            Self::NetworkConnectionReceiveEvent => "network_connection_receive_event",
            Self::NetworkConnectionCopyLocalAddress => "network_connection_copy_local_address",
            Self::NetworkConnectionCopyRemoteAddress => "network_connection_copy_remote_address",
            Self::NetworkConnectionRequestCancel => "network_connection_request_cancel",
            Self::NetworkConnectionReleaseBarrier => "network_connection_release_barrier",
            Self::NetworkConnectionRelease => "network_connection_release",
            _ => panic!("only connection roles use connection names"),
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
            may_allocate: matches!(
                self,
                Self::DropValueAtPointer
                    | Self::DescriptorReadiness
                    | Self::DescriptorReadinessOrDeadline
                    | Self::MonotonicDeadline
                    | Self::TaskJoin
            ),
            may_block: matches!(
                self,
                Self::NetworkConnectionReceiveEvent
                    | Self::NetworkConnectionReleaseBarrier
                    | Self::NetworkListenerReceiveEvent
                    | Self::NetworkListenerReleaseBarrier
                    | Self::Syscall0
                    | Self::SyscallPair0
                    | Self::Syscall1
                    | Self::Syscall2
                    | Self::Syscall3
                    | Self::Syscall4
                    | Self::Syscall5
                    | Self::Syscall6
            ),
            returns_drive_safe_future: matches!(
                self,
                Self::DescriptorReadiness
                    | Self::DescriptorReadinessOrDeadline
                    | Self::MonotonicDeadline
                    | Self::TaskJoin
            ),
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
    fn allocation_effects_are_owned_by_the_closed_primitive_roles() {
        let effectful = PrimitiveRole::ALL
            .iter()
            .copied()
            .filter(|role| role.effects().may_allocate())
            .collect::<Vec<_>>();
        assert_eq!(
            effectful,
            vec![
                PrimitiveRole::DropValueAtPointer,
                PrimitiveRole::DescriptorReadiness,
                PrimitiveRole::DescriptorReadinessOrDeadline,
                PrimitiveRole::MonotonicDeadline,
                PrimitiveRole::TaskJoin,
            ]
        );
    }

    #[test]
    fn blocking_effects_are_owned_by_the_closed_primitive_roles() {
        let effectful = PrimitiveRole::ALL
            .iter()
            .copied()
            .filter(|role| role.effects().may_block())
            .collect::<Vec<_>>();
        assert_eq!(
            effectful,
            vec![
                PrimitiveRole::NetworkConnectionReceiveEvent,
                PrimitiveRole::NetworkConnectionReleaseBarrier,
                PrimitiveRole::NetworkListenerReceiveEvent,
                PrimitiveRole::NetworkListenerReleaseBarrier,
                PrimitiveRole::Syscall0,
                PrimitiveRole::SyscallPair0,
                PrimitiveRole::Syscall1,
                PrimitiveRole::Syscall2,
                PrimitiveRole::Syscall3,
                PrimitiveRole::Syscall4,
                PrimitiveRole::Syscall5,
                PrimitiveRole::Syscall6,
            ]
        );
    }

    #[test]
    fn drive_safe_future_constructors_are_explicitly_certified() {
        let certified = PrimitiveRole::ALL
            .iter()
            .copied()
            .filter(|role| role.effects().returns_drive_safe_future())
            .collect::<Vec<_>>();
        assert_eq!(
            certified,
            vec![
                PrimitiveRole::DescriptorReadiness,
                PrimitiveRole::DescriptorReadinessOrDeadline,
                PrimitiveRole::MonotonicDeadline,
                PrimitiveRole::TaskJoin,
            ]
        );
    }
}
