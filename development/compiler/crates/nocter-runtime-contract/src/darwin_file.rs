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
    use super::{DarwinFileAccess, DarwinFileOperation, DarwinFileSeekOrigin};

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
}
