use crate::{DarwinFileOperation, RuntimeAbiIdentity, RuntimeAsyncAbiSchema};

/// Fields of one generated Darwin file-operation computation.
///
/// The first five fields are the canonical opaque-future header. The fixed record is followed by
/// operation-owned bytes. No field points into a caller's path or transfer buffer: open and write
/// copy their input before admission, while read owns its output storage until consumption.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(usize)]
pub enum DarwinFileJobField {
    ResumeFunction,
    CancelFunction,
    ConsumeFunction,
    LifecycleState,
    AllocationContext,
    Service,
    Operation,
    RetirementRecord,
    AllocationSize,
    OwnedByteLength,
    PositionedOffset,
    TruncateLength,
    SeekDisplacement,
    Access,
    SeekOrigin,
    TransferredByteCount,
    ResultPosition,
    FailureKind,
    FailureErrno,
    Readiness,
    InterestKind,
    InterestSubject,
    InterestDetail,
    InterestReadinessPointer,
}

impl DarwinFileJobField {
    pub const ALL: &'static [Self] = &[
        Self::ResumeFunction,
        Self::CancelFunction,
        Self::ConsumeFunction,
        Self::LifecycleState,
        Self::AllocationContext,
        Self::Service,
        Self::Operation,
        Self::RetirementRecord,
        Self::AllocationSize,
        Self::OwnedByteLength,
        Self::PositionedOffset,
        Self::TruncateLength,
        Self::SeekDisplacement,
        Self::Access,
        Self::SeekOrigin,
        Self::TransferredByteCount,
        Self::ResultPosition,
        Self::FailureKind,
        Self::FailureErrno,
        Self::Readiness,
        Self::InterestKind,
        Self::InterestSubject,
        Self::InterestDetail,
        Self::InterestReadinessPointer,
    ];
}

/// Meaning of the bytes trailing one generated file-job record.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinFileJobOwnedBytes {
    None,
    /// A complete target path including its terminal NUL byte.
    NullTerminatedPath,
    /// Uninitialized capacity owned by the job and initialized only by the worker.
    ReadOutput,
    /// A complete copy of the caller's write input.
    WriteInput,
}

/// State required of the job's pre-reserved retirement record at admission.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinFileJobRetirementInput {
    Reserved,
    Live,
}

/// Operation-specific scalar input admitted by one generated file job.
///
/// Each variant maps to a dedicated field rather than to a generic payload word. The worker can
/// therefore consume the schema without a second convention for interpreting anonymous slots.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinFileJobOperand {
    None,
    Access,
    PositionedOffset,
    TruncateLength,
    Seek,
}

/// Scalar result published by the operation in addition to the retained retirement record.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinFileJobResult {
    None,
    TransferredByteCount,
    Position,
}

/// Complete operation-specific interpretation of the shared generated job layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinFileJobContract {
    owned_bytes: DarwinFileJobOwnedBytes,
    retirement_input: DarwinFileJobRetirementInput,
    operand: DarwinFileJobOperand,
    result: DarwinFileJobResult,
}

impl DarwinFileJobContract {
    #[must_use]
    pub const fn owned_bytes(self) -> DarwinFileJobOwnedBytes {
        self.owned_bytes
    }

    #[must_use]
    pub const fn retirement_input(self) -> DarwinFileJobRetirementInput {
        self.retirement_input
    }

    #[must_use]
    pub const fn operand(self) -> DarwinFileJobOperand {
        self.operand
    }

    #[must_use]
    pub const fn result(self) -> DarwinFileJobResult {
        self.result
    }
}

impl DarwinFileOperation {
    /// Returns the sole field interpretation for one generated file operation.
    #[must_use]
    pub const fn job_contract(self) -> DarwinFileJobContract {
        use DarwinFileJobOperand as Operand;
        use DarwinFileJobOwnedBytes as Bytes;
        use DarwinFileJobResult as Result;
        use DarwinFileJobRetirementInput as Retirement;

        let (owned_bytes, retirement_input, operand, result) = match self {
            Self::Open => (
                Bytes::NullTerminatedPath,
                Retirement::Reserved,
                Operand::Access,
                Result::None,
            ),
            Self::Read => (
                Bytes::ReadOutput,
                Retirement::Live,
                Operand::None,
                Result::TransferredByteCount,
            ),
            Self::Write => (
                Bytes::WriteInput,
                Retirement::Live,
                Operand::None,
                Result::TransferredByteCount,
            ),
            Self::Flush => (Bytes::None, Retirement::Live, Operand::None, Result::None),
            Self::Seek => (
                Bytes::None,
                Retirement::Live,
                Operand::Seek,
                Result::Position,
            ),
            Self::Truncate => (
                Bytes::None,
                Retirement::Live,
                Operand::TruncateLength,
                Result::None,
            ),
            Self::ReadAt => (
                Bytes::ReadOutput,
                Retirement::Live,
                Operand::PositionedOffset,
                Result::TransferredByteCount,
            ),
            Self::WriteAt => (
                Bytes::WriteInput,
                Retirement::Live,
                Operand::PositionedOffset,
                Result::TransferredByteCount,
            ),
        };
        DarwinFileJobContract {
            owned_bytes,
            retirement_input,
            operand,
            result,
        }
    }
}

/// Complete fixed and trailing-byte layout for one generated ARM64 Darwin file job.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinFileJobAbiSchema {
    asynchronous: RuntimeAsyncAbiSchema,
    field_offsets: [u64; DarwinFileJobField::ALL.len()],
    fixed_size: u64,
    alignment: u64,
}

impl DarwinFileJobAbiSchema {
    pub const ARM64_DARWIN: Self = Self {
        asynchronous: RuntimeAbiIdentity::Arm64DarwinV1.schema().asynchronous(),
        field_offsets: [
            0, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128, 136, 144, 152,
            160, 168, 176, 184,
        ],
        fixed_size: 192,
        alignment: 8,
    };

    #[must_use]
    pub const fn asynchronous(self) -> RuntimeAsyncAbiSchema {
        self.asynchronous
    }

    #[must_use]
    pub const fn offset(self, field: DarwinFileJobField) -> u64 {
        self.field_offsets[field as usize]
    }

    #[must_use]
    pub const fn fixed_size(self) -> u64 {
        self.fixed_size
    }

    #[must_use]
    pub const fn alignment(self) -> u64 {
        self.alignment
    }

    /// Offset of the operation-owned trailing bytes.
    #[must_use]
    pub const fn owned_bytes_offset(self) -> u64 {
        self.fixed_size
    }

    /// Computes the one allocation size used by construction and later frame release.
    #[must_use]
    pub const fn allocation_size(self, owned_byte_length: u64) -> Option<u64> {
        self.fixed_size.checked_add(owned_byte_length)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DarwinFileJobAbiSchema, DarwinFileJobField, DarwinFileJobOperand, DarwinFileJobOwnedBytes,
        DarwinFileJobResult, DarwinFileJobRetirementInput,
    };
    use crate::DarwinFileOperation;

    #[test]
    fn job_layout_extends_the_single_async_header_and_has_one_trailing_region() {
        let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
        let asynchronous = schema.asynchronous();
        assert_eq!(
            schema.offset(DarwinFileJobField::ResumeFunction),
            asynchronous.resume_function_offset()
        );
        assert_eq!(
            schema.offset(DarwinFileJobField::CancelFunction),
            asynchronous.cancel_function_offset()
        );
        assert_eq!(
            schema.offset(DarwinFileJobField::ConsumeFunction),
            asynchronous.consume_function_offset()
        );
        assert_eq!(
            schema.offset(DarwinFileJobField::LifecycleState),
            asynchronous.state_tag_offset()
        );
        assert_eq!(
            schema.offset(DarwinFileJobField::AllocationContext),
            asynchronous.allocation_context_offset()
        );
        assert_eq!(
            schema.offset(DarwinFileJobField::Service),
            asynchronous.fixed_header_size()
        );
        for (index, field) in DarwinFileJobField::ALL.iter().copied().enumerate() {
            assert_eq!(schema.offset(field), (index as u64) * 8);
        }
        assert_eq!(schema.fixed_size(), 192);
        assert_eq!(schema.alignment(), asynchronous.fixed_header_alignment());
        assert_eq!(schema.owned_bytes_offset(), schema.fixed_size());
        assert_eq!(schema.allocation_size(0), Some(192));
        assert_eq!(schema.allocation_size(31), Some(223));
        assert_eq!(schema.allocation_size(u64::MAX), None);
    }

    #[test]
    fn every_operation_has_one_closed_input_and_result_contract() {
        use DarwinFileJobOperand as Operand;
        use DarwinFileJobOwnedBytes as Bytes;
        use DarwinFileJobResult as Result;
        use DarwinFileJobRetirementInput as Retirement;

        let expected = [
            (
                DarwinFileOperation::Open,
                Bytes::NullTerminatedPath,
                Retirement::Reserved,
                Operand::Access,
                Result::None,
            ),
            (
                DarwinFileOperation::Read,
                Bytes::ReadOutput,
                Retirement::Live,
                Operand::None,
                Result::TransferredByteCount,
            ),
            (
                DarwinFileOperation::Write,
                Bytes::WriteInput,
                Retirement::Live,
                Operand::None,
                Result::TransferredByteCount,
            ),
            (
                DarwinFileOperation::Flush,
                Bytes::None,
                Retirement::Live,
                Operand::None,
                Result::None,
            ),
            (
                DarwinFileOperation::Seek,
                Bytes::None,
                Retirement::Live,
                Operand::Seek,
                Result::Position,
            ),
            (
                DarwinFileOperation::Truncate,
                Bytes::None,
                Retirement::Live,
                Operand::TruncateLength,
                Result::None,
            ),
            (
                DarwinFileOperation::ReadAt,
                Bytes::ReadOutput,
                Retirement::Live,
                Operand::PositionedOffset,
                Result::TransferredByteCount,
            ),
            (
                DarwinFileOperation::WriteAt,
                Bytes::WriteInput,
                Retirement::Live,
                Operand::PositionedOffset,
                Result::TransferredByteCount,
            ),
        ];

        assert_eq!(expected.len(), DarwinFileOperation::ALL.len());
        for (operation, bytes, retirement, operand, result) in expected {
            let contract = operation.job_contract();
            assert_eq!(contract.owned_bytes(), bytes);
            assert_eq!(contract.retirement_input(), retirement);
            assert_eq!(contract.operand(), operand);
            assert_eq!(contract.result(), result);
        }
    }
}
