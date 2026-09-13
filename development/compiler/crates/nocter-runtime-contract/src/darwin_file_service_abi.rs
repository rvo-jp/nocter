use crate::{DarwinFileServiceConfiguration, RuntimeAbiIdentity, RuntimeAsyncAbiSchema};

/// Process-owned fields of the generated Darwin file service.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(usize)]
pub enum DarwinFileServiceField {
    OperationQueueZero,
    OperationQueueOne,
    OperationQueueTwo,
    OperationQueueThree,
    RetirementQueue,
    WorkerGroup,
    NotificationReader,
    NotificationWriter,
    ActiveOperationCount,
    NextOperationQueue,
    AdmissionState,
}

impl DarwinFileServiceField {
    pub const ALL: &'static [Self] = &[
        Self::OperationQueueZero,
        Self::OperationQueueOne,
        Self::OperationQueueTwo,
        Self::OperationQueueThree,
        Self::RetirementQueue,
        Self::WorkerGroup,
        Self::NotificationReader,
        Self::NotificationWriter,
        Self::ActiveOperationCount,
        Self::NextOperationQueue,
        Self::AdmissionState,
    ];

    pub const OPERATION_QUEUES: &'static [Self] = &[
        Self::OperationQueueZero,
        Self::OperationQueueOne,
        Self::OperationQueueTwo,
        Self::OperationQueueThree,
    ];
}

/// Fields of one pre-reserved descriptor-retirement record.
///
/// The first five fields are the ordinary opaque future header. A live `FileOwner` points to this
/// same record; explicit close turns it into a future without allocating or moving descriptor
/// ownership into a second record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(usize)]
pub enum DarwinFileRetirementField {
    ResumeFunction,
    CancelFunction,
    ConsumeFunction,
    LifecycleState,
    AllocationContext,
    Service,
    Descriptor,
    FailureKind,
    FailureErrno,
    Readiness,
    InterestKind,
    InterestSubject,
    InterestDetail,
    InterestReadinessPointer,
}

impl DarwinFileRetirementField {
    pub const ALL: &'static [Self] = &[
        Self::ResumeFunction,
        Self::CancelFunction,
        Self::ConsumeFunction,
        Self::LifecycleState,
        Self::AllocationContext,
        Self::Service,
        Self::Descriptor,
        Self::FailureKind,
        Self::FailureErrno,
        Self::Readiness,
        Self::InterestKind,
        Self::InterestSubject,
        Self::InterestDetail,
        Self::InterestReadinessPointer,
    ];
}

/// Root lifecycle of the process-owned generated file service.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinFileServiceState {
    Accepting,
    Draining,
    Released,
}

/// Result of one generated operation or retirement-capacity admission attempt.
///
/// `Saturated` is transient and leaves the complete input owned by its future. `Closed` is
/// terminal and is observable while the service root remains alive during orderly drain.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinFileServiceAdmission {
    Ready,
    Saturated,
    Closed,
}

impl DarwinFileServiceAdmission {
    pub const ALL: &'static [Self] = &[Self::Ready, Self::Saturated, Self::Closed];

    #[must_use]
    pub const fn code(self) -> u64 {
        match self {
            Self::Ready => 0,
            Self::Saturated => 1,
            Self::Closed => 2,
        }
    }

    #[must_use]
    pub const fn from_code(code: u64) -> Option<Self> {
        match code {
            0 => Some(Self::Ready),
            1 => Some(Self::Saturated),
            2 => Some(Self::Closed),
            _ => None,
        }
    }
}

impl DarwinFileServiceState {
    pub const ALL: &'static [Self] = &[Self::Accepting, Self::Draining, Self::Released];

    #[must_use]
    pub const fn code(self) -> u64 {
        match self {
            Self::Accepting => 0,
            Self::Draining => 1,
            Self::Released => 2,
        }
    }

    #[must_use]
    pub const fn from_code(code: u64) -> Option<Self> {
        match code {
            0 => Some(Self::Accepting),
            1 => Some(Self::Draining),
            2 => Some(Self::Released),
            _ => None,
        }
    }

    /// Applies one root lifecycle event without allowing service storage to precede its drain.
    #[must_use]
    pub const fn apply(self, event: DarwinFileServiceEvent) -> Option<DarwinFileServiceTransition> {
        let (next, action) = match (self, event) {
            (Self::Accepting, DarwinFileServiceEvent::BeginDrain) => {
                (Self::Draining, DarwinFileServiceAction::DrainWorkers)
            }
            (Self::Draining, DarwinFileServiceEvent::FinishDrain) => {
                (Self::Released, DarwinFileServiceAction::ReleaseResources)
            }
            _ => return None,
        };
        Some(DarwinFileServiceTransition { next, action })
    }
}

/// One root lifecycle transition attempted by generated shutdown code.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinFileServiceEvent {
    BeginDrain,
    FinishDrain,
}

/// Work authorized by one valid root lifecycle transition.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinFileServiceAction {
    DrainWorkers,
    ReleaseResources,
}

/// One validated generated file-service root transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinFileServiceTransition {
    next: DarwinFileServiceState,
    action: DarwinFileServiceAction,
}

impl DarwinFileServiceTransition {
    #[must_use]
    pub const fn next(self) -> DarwinFileServiceState {
        self.next
    }

    #[must_use]
    pub const fn action(self) -> DarwinFileServiceAction {
        self.action
    }
}

/// Complete generated service and retirement-record layout for ARM64 Darwin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinFileServiceAbiSchema {
    field_offsets: [u64; DarwinFileServiceField::ALL.len()],
    retirement_records_offset: u64,
    retirement_record: DarwinFileRetirementAbiSchema,
    retirement_record_count: usize,
    size: u64,
    alignment: u64,
}

impl DarwinFileServiceAbiSchema {
    pub const ARM64_DARWIN: Self = Self {
        field_offsets: [0, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80],
        retirement_records_offset: 88,
        retirement_record: DarwinFileRetirementAbiSchema::ARM64_DARWIN,
        retirement_record_count: DarwinFileServiceConfiguration::ARM64_DARWIN.maximum_retirements(),
        size: 7_256,
        alignment: 8,
    };

    #[must_use]
    pub const fn offset(self, field: DarwinFileServiceField) -> u64 {
        self.field_offsets[field as usize]
    }

    #[must_use]
    pub fn operation_queue_offset(self, index: usize) -> Option<u64> {
        DarwinFileServiceField::OPERATION_QUEUES
            .get(index)
            .map(|field| self.offset(*field))
    }

    #[must_use]
    pub const fn retirement_record(self) -> DarwinFileRetirementAbiSchema {
        self.retirement_record
    }

    #[must_use]
    pub const fn retirement_record_count(self) -> usize {
        self.retirement_record_count
    }

    #[must_use]
    pub const fn retirement_record_offset(self, index: usize) -> Option<u64> {
        if index >= self.retirement_record_count {
            return None;
        }
        let Some(index) = (index as u64).checked_mul(self.retirement_record.size()) else {
            return None;
        };
        self.retirement_records_offset.checked_add(index)
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

/// Exact layout of one pre-reserved close record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinFileRetirementAbiSchema {
    asynchronous: RuntimeAsyncAbiSchema,
    field_offsets: [u64; DarwinFileRetirementField::ALL.len()],
    size: u64,
    alignment: u64,
}

impl DarwinFileRetirementAbiSchema {
    pub const ARM64_DARWIN: Self = Self {
        asynchronous: RuntimeAbiIdentity::Arm64DarwinV1.schema().asynchronous(),
        field_offsets: [0, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104],
        size: 112,
        alignment: 8,
    };

    #[must_use]
    pub const fn asynchronous(self) -> RuntimeAsyncAbiSchema {
        self.asynchronous
    }

    #[must_use]
    pub const fn offset(self, field: DarwinFileRetirementField) -> u64 {
        self.field_offsets[field as usize]
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        DarwinFileRetirementAbiSchema, DarwinFileRetirementField, DarwinFileServiceAbiSchema,
        DarwinFileServiceAction, DarwinFileServiceAdmission, DarwinFileServiceEvent,
        DarwinFileServiceField, DarwinFileServiceState,
    };
    use crate::DarwinFileServiceConfiguration;

    #[test]
    fn service_layout_is_dense_bounded_and_contains_every_retirement_record() {
        let schema = DarwinFileServiceAbiSchema::ARM64_DARWIN;
        for (index, field) in DarwinFileServiceField::ALL.iter().copied().enumerate() {
            assert_eq!(schema.offset(field), (index as u64) * 8);
        }
        assert_eq!(
            DarwinFileServiceField::OPERATION_QUEUES.len(),
            DarwinFileServiceConfiguration::ARM64_DARWIN.operation_workers()
        );
        assert_eq!(schema.retirement_record_offset(0), Some(88));
        assert_eq!(schema.retirement_record_offset(63), Some(7_144));
        assert_eq!(schema.retirement_record_offset(64), None);
        assert_eq!(schema.size(), 7_256);
        assert_eq!(schema.alignment(), 8);
    }

    #[test]
    fn retirement_record_extends_the_single_async_header_without_redeclaring_it() {
        let schema = DarwinFileRetirementAbiSchema::ARM64_DARWIN;
        let asynchronous = schema.asynchronous();
        assert_eq!(
            schema.offset(DarwinFileRetirementField::ResumeFunction),
            asynchronous.resume_function_offset()
        );
        assert_eq!(
            schema.offset(DarwinFileRetirementField::CancelFunction),
            asynchronous.cancel_function_offset()
        );
        assert_eq!(
            schema.offset(DarwinFileRetirementField::ConsumeFunction),
            asynchronous.consume_function_offset()
        );
        assert_eq!(
            schema.offset(DarwinFileRetirementField::LifecycleState),
            asynchronous.state_tag_offset()
        );
        assert_eq!(
            schema.offset(DarwinFileRetirementField::AllocationContext),
            asynchronous.allocation_context_offset()
        );
        assert_eq!(
            schema.offset(DarwinFileRetirementField::Service),
            asynchronous.fixed_header_size()
        );
        for (index, field) in DarwinFileRetirementField::ALL.iter().copied().enumerate() {
            assert_eq!(schema.offset(field), (index as u64) * 8);
        }
        assert_eq!(schema.size(), 112);
        assert_eq!(schema.alignment(), asynchronous.fixed_header_alignment());
    }

    #[test]
    fn service_state_tags_are_closed_unique_and_round_trip() {
        let codes = DarwinFileServiceState::ALL
            .iter()
            .copied()
            .map(DarwinFileServiceState::code)
            .collect::<BTreeSet<_>>();
        assert_eq!(codes.len(), DarwinFileServiceState::ALL.len());
        for state in DarwinFileServiceState::ALL.iter().copied() {
            assert_eq!(DarwinFileServiceState::from_code(state.code()), Some(state));
        }
        assert_eq!(DarwinFileServiceState::from_code(u64::MAX), None);

        let drain = DarwinFileServiceState::Accepting
            .apply(DarwinFileServiceEvent::BeginDrain)
            .unwrap();
        assert_eq!(drain.next(), DarwinFileServiceState::Draining);
        assert_eq!(drain.action(), DarwinFileServiceAction::DrainWorkers);
        let release = drain
            .next()
            .apply(DarwinFileServiceEvent::FinishDrain)
            .unwrap();
        assert_eq!(release.next(), DarwinFileServiceState::Released);
        assert_eq!(release.action(), DarwinFileServiceAction::ReleaseResources);
        assert_eq!(
            DarwinFileServiceState::Accepting.apply(DarwinFileServiceEvent::FinishDrain),
            None
        );
        assert_eq!(
            DarwinFileServiceState::Released.apply(DarwinFileServiceEvent::BeginDrain),
            None
        );
    }

    #[test]
    fn service_admission_results_are_closed_unique_and_round_trip() {
        let codes = DarwinFileServiceAdmission::ALL
            .iter()
            .copied()
            .map(DarwinFileServiceAdmission::code)
            .collect::<BTreeSet<_>>();
        assert_eq!(codes.len(), DarwinFileServiceAdmission::ALL.len());
        for result in DarwinFileServiceAdmission::ALL.iter().copied() {
            assert_eq!(
                DarwinFileServiceAdmission::from_code(result.code()),
                Some(result)
            );
        }
        assert_eq!(DarwinFileServiceAdmission::from_code(u64::MAX), None);
    }
}
