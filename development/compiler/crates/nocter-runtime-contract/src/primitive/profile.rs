use super::{
    PrimitiveContexts, PrimitiveExecutionFacts, PrimitiveProducedComputation, PrimitiveRole,
};

/// Complete implementation-owned runtime contract for one primitive role.
///
/// The exhaustive role projection below is deliberately separate from source declarations and
/// target signatures. Adding a role without assigning every execution and hidden-context fact is
/// therefore a compile-time error rather than an implicit allocation-free or nonblocking promise.
#[derive(Clone, Copy)]
struct PrimitiveRuntimeProfile {
    execution: PrimitiveExecutionFacts,
    contexts: PrimitiveContexts,
}

impl PrimitiveRuntimeProfile {
    const fn new(execution: PrimitiveExecutionFacts, contexts: PrimitiveContexts) -> Self {
        Self {
            execution,
            contexts,
        }
    }
}

const INERT_EXECUTION: PrimitiveExecutionFacts = PrimitiveExecutionFacts {
    may_allocate: false,
    may_block: false,
    produced_computation: PrimitiveProducedComputation::None,
};
const ALLOCATING_EXECUTION: PrimitiveExecutionFacts = PrimitiveExecutionFacts {
    may_allocate: true,
    ..INERT_EXECUTION
};
const BLOCKING_EXECUTION: PrimitiveExecutionFacts = PrimitiveExecutionFacts {
    may_block: true,
    ..INERT_EXECUTION
};
const ALLOCATING_FUTURE_EXECUTION: PrimitiveExecutionFacts = PrimitiveExecutionFacts {
    may_allocate: true,
    may_block: false,
    produced_computation: PrimitiveProducedComputation::DriveSafeFuture,
};
const PREALLOCATED_FUTURE_EXECUTION: PrimitiveExecutionFacts = PrimitiveExecutionFacts {
    produced_computation: PrimitiveProducedComputation::DriveSafeFuture,
    ..INERT_EXECUTION
};

const NO_CONTEXT: PrimitiveContexts = PrimitiveContexts {
    allocation: false,
    process: false,
};
const ALLOCATION_ONLY: PrimitiveContexts = PrimitiveContexts {
    allocation: true,
    process: false,
};
const PROCESS_ONLY: PrimitiveContexts = PrimitiveContexts {
    allocation: false,
    process: true,
};
const BOTH_CONTEXTS: PrimitiveContexts = PrimitiveContexts {
    allocation: true,
    process: true,
};

const PLAIN: PrimitiveRuntimeProfile = PrimitiveRuntimeProfile::new(INERT_EXECUTION, NO_CONTEXT);
const ALLOCATING: PrimitiveRuntimeProfile =
    PrimitiveRuntimeProfile::new(ALLOCATING_EXECUTION, NO_CONTEXT);
const BLOCKING: PrimitiveRuntimeProfile =
    PrimitiveRuntimeProfile::new(BLOCKING_EXECUTION, NO_CONTEXT);
const ALLOCATION_CONTEXT: PrimitiveRuntimeProfile =
    PrimitiveRuntimeProfile::new(INERT_EXECUTION, ALLOCATION_ONLY);
const PROCESS_CONTEXT: PrimitiveRuntimeProfile =
    PrimitiveRuntimeProfile::new(INERT_EXECUTION, PROCESS_ONLY);
const ALLOCATING_FUTURE_WITH_ALLOCATION_CONTEXT: PrimitiveRuntimeProfile =
    PrimitiveRuntimeProfile::new(ALLOCATING_FUTURE_EXECUTION, ALLOCATION_ONLY);
const ALLOCATING_FUTURE_WITH_BOTH_CONTEXTS: PrimitiveRuntimeProfile =
    PrimitiveRuntimeProfile::new(ALLOCATING_FUTURE_EXECUTION, BOTH_CONTEXTS);
const PREALLOCATED_FUTURE: PrimitiveRuntimeProfile =
    PrimitiveRuntimeProfile::new(PREALLOCATED_FUTURE_EXECUTION, NO_CONTEXT);

impl PrimitiveRole {
    /// Returns compiler-owned execution evidence for this closed primitive role.
    #[must_use]
    pub const fn execution_facts(self) -> PrimitiveExecutionFacts {
        self.runtime_profile().execution
    }

    /// Returns the hidden ambient capabilities consumed by this primitive's implementation.
    #[must_use]
    pub const fn contexts(self) -> PrimitiveContexts {
        self.runtime_profile().contexts
    }

    #[allow(
        clippy::too_many_lines,
        reason = "an exhaustive role projection makes omitted primitive contracts fail compilation"
    )]
    const fn runtime_profile(self) -> PrimitiveRuntimeProfile {
        match self {
            Self::DropValueAtPointer => ALLOCATING,
            Self::TimeoutWait
            | Self::NetworkConnectionReceiveEvent
            | Self::NetworkConnectionReleaseBarrier
            | Self::NetworkListenerReceiveEvent
            | Self::NetworkListenerReleaseBarrier
            | Self::Syscall0
            | Self::Syscall1
            | Self::Syscall2
            | Self::Syscall3
            | Self::Syscall3Signed
            | Self::Syscall4
            | Self::Syscall6 => BLOCKING,
            Self::CurrentAllocatorState | Self::CurrentAllocatorKind => ALLOCATION_CONTEXT,
            Self::ProcessArgumentCount
            | Self::ProcessArgument
            | Self::ProcessEnvironmentCount
            | Self::ProcessEnvironmentName
            | Self::ProcessEnvironmentValue
            | Self::ProcessTerminationDescriptor
            | Self::ProcessTerminationObserve => PROCESS_CONTEXT,
            Self::DescriptorReadiness
            | Self::DescriptorReadinessOrDeadline
            | Self::MonotonicDeadline
            | Self::ProcessCompletion
            | Self::TaskJoin
            | Self::TaskRace
            | Self::TaskGroupReady => ALLOCATING_FUTURE_WITH_ALLOCATION_CONTEXT,
            Self::FileOpenRead
            | Self::FileOpenCreate
            | Self::FileOpenCreateNew
            | Self::FileOpenAppend
            | Self::FileOpenCopyDestination
            | Self::DirectoryOpen
            | Self::FileRead
            | Self::DirectoryRead
            | Self::FileWrite
            | Self::FileFlush
            | Self::FileSeekStart
            | Self::FileSeekEnd
            | Self::FileSeekCurrent
            | Self::FileTruncate
            | Self::FileReadAt
            | Self::FileWriteAt
            | Self::FileIdentity
            | Self::FilesystemRemoveFile
            | Self::FilesystemRename
            | Self::FilesystemCreateDirectory
            | Self::FilesystemRemoveDirectory
            | Self::FilesystemMetadata
            | Self::FilesystemSymlinkMetadata
            | Self::FilesystemCreateSymlink
            | Self::FilesystemReadLink
            | Self::FilesystemCanonicalize => ALLOCATING_FUTURE_WITH_BOTH_CONTEXTS,
            Self::FileClose => PREALLOCATED_FUTURE,
            Self::NewError
            | Self::ErrorContext
            | Self::ErrorCode
            | Self::ErrorMessage
            | Self::AllocationFailureError
            | Self::AllocationAbort
            | Self::MemoryMap
            | Self::MemoryUnmap
            | Self::DescriptorClose
            | Self::DescriptorPipeCreate
            | Self::DescriptorDuplicateCloseOnExec
            | Self::DescriptorStatusFlags
            | Self::DescriptorSetStatusFlags
            | Self::DescriptorSuppressBrokenPipe
            | Self::DescriptorRead
            | Self::DescriptorWrite
            | Self::DatagramSocketOpen
            | Self::DatagramSocketConfigure
            | Self::DatagramBind
            | Self::DatagramConnect
            | Self::DatagramConnectStatus
            | Self::DatagramSend
            | Self::DatagramSendTo
            | Self::DatagramReceive
            | Self::DatagramLocalAddress
            | Self::DatagramPeerAddress
            | Self::EntropyFill
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
            | Self::TakeValueAtPointer
            | Self::TakeValueAtPointerFromOwner
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
            | Self::StringPointerAddress
            | Self::CharacterFromU32Unchecked
            | Self::CharacterCodePoint
            | Self::U8Truncate
            | Self::U16Truncate
            | Self::U32Truncate
            | Self::I8Truncate
            | Self::I16Truncate
            | Self::I32Truncate
            | Self::I8FromBits
            | Self::I8ToBits
            | Self::I16FromBits
            | Self::I16ToBits
            | Self::I32FromBits
            | Self::I32ToBits
            | Self::I64FromBits
            | Self::I64ToBits
            | Self::F32FromBits
            | Self::F32ToBits
            | Self::F64FromBits
            | Self::F64ToBits
            | Self::F32Floor
            | Self::F32Ceil
            | Self::F32Trunc
            | Self::F32RoundTiesEven
            | Self::F64Floor
            | Self::F64Ceil
            | Self::F64Trunc
            | Self::F64RoundTiesEven
            | Self::F64ToF32
            | Self::F64ToI64
            | Self::F64ToU64
            | Self::U64WrappingAdd
            | Self::U64WrappingSubtract
            | Self::U64WrappingMultiply
            | Self::U64MultiplyHigh
            | Self::U64BitwiseAnd
            | Self::U64BitwiseOr
            | Self::U64BitwiseXor
            | Self::U64RotateRight
            | Self::U64LeadingZeros
            | Self::ProcessExit
            | Self::ProcessFork
            | Self::ProcessOpenNull
            | Self::ProcessInstallDescriptor
            | Self::ProcessChangeDirectory
            | Self::ProcessExec
            | Self::MonotonicCounterRead
            | Self::MonotonicCounterFrequency
            | Self::MonotonicCounterDelta
            | Self::WallClockRead
            | Self::ProcessObserve
            | Self::ProcessTerminate
            | Self::ProcessKill
            | Self::ProcessAbandon
            | Self::FileOwnerDispose
            | Self::FileCompletionTakeOwner
            | Self::FileCompletionTransferredByteCount
            | Self::FileCompletionResultPosition
            | Self::FileCompletionMetadataKind
            | Self::FileCompletionMetadataLength
            | Self::FileCompletionMetadataModifiedSeconds
            | Self::FileCompletionMetadataModifiedNanoseconds
            | Self::FileCompletionIdentityDevice
            | Self::FileCompletionIdentityInode
            | Self::FileCompletionFailureKind
            | Self::FileCompletionFailureErrno
            | Self::FileCompletionDispose
            | Self::NetworkConnectionCreate
            | Self::NetworkConnectionCreateHost
            | Self::NetworkTlsConnectionCreate
            | Self::NetworkTlsConnectionCreateHost
            | Self::NetworkTlsConnectionMatchesApplicationProtocol
            | Self::NetworkConnectionStart
            | Self::NetworkConnectionEventDescriptor
            | Self::NetworkConnectionBeginReceive
            | Self::NetworkConnectionBeginSend
            | Self::NetworkConnectionTryReceiveEvent
            | Self::NetworkConnectionCopyLocalAddress
            | Self::NetworkConnectionCopyRemoteAddress
            | Self::NetworkConnectionRequestCancel
            | Self::NetworkConnectionRelease
            | Self::NetworkConnectionDispose
            | Self::NetworkListenerCreate
            | Self::NetworkListenerStart
            | Self::NetworkListenerEventDescriptor
            | Self::NetworkListenerTryReceiveEvent
            | Self::NetworkListenerPort
            | Self::NetworkListenerRequestCancel
            | Self::NetworkListenerRelease
            | Self::NetworkListenerDispose
            | Self::Trap
            | Self::Unreachable => PLAIN,
        }
    }
}
