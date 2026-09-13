/// One blocking file-operation family admitted by the generated Darwin service.
///
/// This is the source-independent operation vocabulary shared by host conformance and native
/// target generation. It describes what a worker executes, not public `std/fs` naming, queue
/// state, or source-level error policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinFileOperation {
    Open,
    Read,
    Write,
    Flush,
    Seek,
    Truncate,
    ReadAt,
    WriteAt,
}

/// Bounded generated-service capacity selected by the runtime contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinFileServiceConfiguration {
    operation_workers: usize,
    maximum_operations: usize,
    retirement_workers: usize,
    maximum_retirements: usize,
}

impl DarwinFileServiceConfiguration {
    /// Capacity for the supported generated ARM64 Darwin runtime.
    pub const ARM64_DARWIN: Self = Self {
        operation_workers: 4,
        maximum_operations: 64,
        retirement_workers: 1,
        maximum_retirements: 64,
    };

    #[must_use]
    pub const fn operation_workers(self) -> usize {
        self.operation_workers
    }

    #[must_use]
    pub const fn maximum_operations(self) -> usize {
        self.maximum_operations
    }

    #[must_use]
    pub const fn retirement_workers(self) -> usize {
        self.retirement_workers
    }

    #[must_use]
    pub const fn maximum_retirements(self) -> usize {
        self.maximum_retirements
    }
}

impl DarwinFileOperation {
    /// Every file operation in stable target-contract order.
    pub const ALL: &'static [Self] = &[
        Self::Open,
        Self::Read,
        Self::Write,
        Self::Flush,
        Self::Seek,
        Self::Truncate,
        Self::ReadAt,
        Self::WriteAt,
    ];

    /// Returns the compact target-service tag for this operation.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Open => 0,
            Self::Read => 1,
            Self::Write => 2,
            Self::Flush => 3,
            Self::Seek => 4,
            Self::Truncate => 5,
            Self::ReadAt => 6,
            Self::WriteAt => 7,
        }
    }

    /// Decodes one target-service operation tag.
    #[must_use]
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Open),
            1 => Some(Self::Read),
            2 => Some(Self::Write),
            3 => Some(Self::Flush),
            4 => Some(Self::Seek),
            5 => Some(Self::Truncate),
            6 => Some(Self::ReadAt),
            7 => Some(Self::WriteAt),
            _ => None,
        }
    }
}

/// Open behavior selected before a file job crosses into target execution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinFileAccess {
    Read,
    Create,
    Append,
}

impl DarwinFileAccess {
    pub const ALL: &'static [Self] = &[Self::Read, Self::Create, Self::Append];

    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Read => 0,
            Self::Create => 1,
            Self::Append => 2,
        }
    }

    #[must_use]
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Read),
            1 => Some(Self::Create),
            2 => Some(Self::Append),
            _ => None,
        }
    }
}

/// Base used by one cursor-changing seek operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinFileSeekOrigin {
    Start,
    End,
    Current,
}

/// Closed classification of one target-level file-operation failure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinFileFailureKind {
    Allocation,
    Target,
    OffsetOverflow,
    ZeroProgress,
    InvalidProgress,
    Unclassified,
}

/// One validated file-operation failure before standard-library error policy is applied.
///
/// Darwin errno remains an opaque positive target code. Private representation prevents an
/// adapter from constructing the otherwise ambiguous `Target(0)` state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DarwinFileFailure {
    kind: DarwinFileFailureKind,
    target_errno: i32,
}

impl DarwinFileFailure {
    pub const ALLOCATION: Self = Self::without_errno(DarwinFileFailureKind::Allocation);
    pub const OFFSET_OVERFLOW: Self = Self::without_errno(DarwinFileFailureKind::OffsetOverflow);
    pub const ZERO_PROGRESS: Self = Self::without_errno(DarwinFileFailureKind::ZeroProgress);
    pub const INVALID_PROGRESS: Self = Self::without_errno(DarwinFileFailureKind::InvalidProgress);
    /// A target adapter reported failure without a classifiable Darwin errno.
    pub const UNCLASSIFIED: Self = Self::without_errno(DarwinFileFailureKind::Unclassified);

    const fn without_errno(kind: DarwinFileFailureKind) -> Self {
        Self {
            kind,
            target_errno: 0,
        }
    }

    /// Creates one raw target failure only for a positive errno value.
    #[must_use]
    pub const fn target(errno: i32) -> Option<Self> {
        if errno <= 0 {
            None
        } else {
            Some(Self {
                kind: DarwinFileFailureKind::Target,
                target_errno: errno,
            })
        }
    }

    #[must_use]
    pub const fn kind(self) -> DarwinFileFailureKind {
        self.kind
    }

    #[must_use]
    pub const fn target_errno(self) -> Option<i32> {
        if matches!(self.kind, DarwinFileFailureKind::Target) {
            Some(self.target_errno)
        } else {
            None
        }
    }
}

/// Exact prefix progress from one complete-write attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinFileWriteFact {
    attempted: usize,
    transferred: usize,
    failure: Option<DarwinFileFailure>,
}

impl DarwinFileWriteFact {
    #[must_use]
    pub const fn complete(attempted: usize) -> Self {
        Self {
            attempted,
            transferred: attempted,
            failure: None,
        }
    }

    #[must_use]
    pub const fn failed(attempted: usize, transferred: usize, failure: DarwinFileFailure) -> Self {
        if transferred <= attempted {
            Self {
                attempted,
                transferred,
                failure: Some(failure),
            }
        } else {
            Self {
                attempted,
                transferred: attempted,
                failure: Some(DarwinFileFailure::INVALID_PROGRESS),
            }
        }
    }

    #[must_use]
    pub const fn attempted(self) -> usize {
        self.attempted
    }

    #[must_use]
    pub const fn transferred(self) -> usize {
        self.transferred
    }

    #[must_use]
    pub const fn failure(self) -> Option<DarwinFileFailure> {
        self.failure
    }
}

impl DarwinFileSeekOrigin {
    pub const ALL: &'static [Self] = &[Self::Start, Self::End, Self::Current];

    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Start => 0,
            Self::End => 1,
            Self::Current => 2,
        }
    }

    #[must_use]
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Start),
            1 => Some(Self::End),
            2 => Some(Self::Current),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DarwinFileAccess, DarwinFileFailure, DarwinFileFailureKind, DarwinFileOperation,
        DarwinFileSeekOrigin, DarwinFileServiceConfiguration, DarwinFileWriteFact,
    };

    #[test]
    fn closed_file_service_tags_are_unique_and_round_trip() {
        let operation_codes = DarwinFileOperation::ALL
            .iter()
            .copied()
            .map(DarwinFileOperation::code)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(operation_codes.len(), DarwinFileOperation::ALL.len());
        for operation in DarwinFileOperation::ALL.iter().copied() {
            assert_eq!(
                DarwinFileOperation::from_code(operation.code()),
                Some(operation)
            );
        }
        assert_eq!(DarwinFileOperation::from_code(u8::MAX), None);

        for access in DarwinFileAccess::ALL.iter().copied() {
            assert_eq!(DarwinFileAccess::from_code(access.code()), Some(access));
        }
        assert_eq!(DarwinFileAccess::from_code(u8::MAX), None);

        for origin in DarwinFileSeekOrigin::ALL.iter().copied() {
            assert_eq!(DarwinFileSeekOrigin::from_code(origin.code()), Some(origin));
        }
        assert_eq!(DarwinFileSeekOrigin::from_code(u8::MAX), None);
    }

    #[test]
    fn raw_failures_and_write_progress_do_not_depend_on_host_error_objects() {
        assert_eq!(DarwinFileFailure::target(0), None);
        assert_eq!(DarwinFileFailure::target(-1), None);
        let target = DarwinFileFailure::target(5).unwrap();
        assert_eq!(target.kind(), DarwinFileFailureKind::Target);
        assert_eq!(target.target_errno(), Some(5));
        assert_eq!(DarwinFileFailure::ALLOCATION.target_errno(), None);

        let complete = DarwinFileWriteFact::complete(7);
        assert_eq!(complete.attempted(), 7);
        assert_eq!(complete.transferred(), 7);
        assert_eq!(complete.failure(), None);
        let failed = DarwinFileWriteFact::failed(7, 3, DarwinFileFailure::ZERO_PROGRESS);
        assert_eq!(failed.attempted(), 7);
        assert_eq!(failed.transferred(), 3);
        assert_eq!(failed.failure(), Some(DarwinFileFailure::ZERO_PROGRESS));
        let malformed = DarwinFileWriteFact::failed(7, 9, DarwinFileFailure::ZERO_PROGRESS);
        assert_eq!(malformed.transferred(), 7);
        assert_eq!(
            malformed.failure(),
            Some(DarwinFileFailure::INVALID_PROGRESS)
        );
    }

    #[test]
    fn generated_capacity_is_finite_and_keeps_retirement_independent() {
        let capacity = DarwinFileServiceConfiguration::ARM64_DARWIN;
        assert!(capacity.operation_workers() > 0);
        assert!(capacity.maximum_operations() >= capacity.operation_workers());
        assert!(capacity.retirement_workers() > 0);
        assert!(capacity.maximum_retirements() >= capacity.retirement_workers());
    }
}
