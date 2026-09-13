/// Shared state of one generated file-operation computation.
///
/// The computation frame is also the worker-owned job record. There is no second queue record or
/// result table whose lifetime could disagree with the future handle.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinFileJobState {
    /// The frame owns complete input but has not consumed bounded service capacity.
    Prepared,
    /// A worker owns the operation and the source computation still owns its waiter.
    RunningAttached,
    /// The source computation was cancelled; the worker remains the sole frame owner.
    RunningDetached,
    /// The worker published complete output for the attached waiter.
    Completed,
    /// Terminal logical state immediately before the frame is released.
    Released,
}

impl DarwinFileJobState {
    pub const ALL: &'static [Self] = &[
        Self::Prepared,
        Self::RunningAttached,
        Self::RunningDetached,
        Self::Completed,
        Self::Released,
    ];

    #[must_use]
    pub const fn code(self) -> u64 {
        match self {
            Self::Prepared => 0,
            Self::RunningAttached => 1,
            Self::RunningDetached => 2,
            Self::Completed => 3,
            Self::Released => 4,
        }
    }

    #[must_use]
    pub const fn from_code(code: u64) -> Option<Self> {
        match code {
            0 => Some(Self::Prepared),
            1 => Some(Self::RunningAttached),
            2 => Some(Self::RunningDetached),
            3 => Some(Self::Completed),
            4 => Some(Self::Released),
            _ => None,
        }
    }
}

/// One owner transition attempted on a generated file-operation computation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinFileJobEvent {
    Admit,
    Reject,
    Cancel,
    Publish,
    Consume,
}

/// Complete ownership work authorized by one valid file-job transition.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinFileJobAction {
    /// Consume one service-capacity unit and dispatch the owned frame exactly once.
    Dispatch,
    /// Retain a terminal admission failure for the still-attached waiter.
    RetainRejection,
    /// Destroy unsubmitted input and release the frame without touching service capacity.
    ReleasePrepared,
    /// Detach the waiter while preserving worker and frame ownership.
    Detach,
    /// Retain completed output in the frame and notify the executor.
    RetainCompletion,
    /// Destroy abandoned output, release capacity, notify capacity waiters, and release the frame.
    ReleaseDetachedCompletion,
    /// Destroy unconsumed output, release capacity, and release the frame.
    ReleaseCompletion,
    /// Move output to the caller, release capacity, and release the frame.
    ConsumeCompletion,
}

/// One validated generated file-job transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinFileJobTransition {
    next: DarwinFileJobState,
    action: DarwinFileJobAction,
}

impl DarwinFileJobTransition {
    #[must_use]
    pub const fn next(self) -> DarwinFileJobState {
        self.next
    }

    #[must_use]
    pub const fn action(self) -> DarwinFileJobAction {
        self.action
    }
}

impl DarwinFileJobState {
    /// Applies one lifecycle event without inventing recovery for an invalid owner transition.
    #[must_use]
    pub const fn apply(self, event: DarwinFileJobEvent) -> Option<DarwinFileJobTransition> {
        use DarwinFileJobAction as Action;
        use DarwinFileJobEvent as Event;
        use DarwinFileJobState as State;

        let (next, action) = match (self, event) {
            (State::Prepared, Event::Admit) => (State::RunningAttached, Action::Dispatch),
            (State::Prepared, Event::Reject) => (State::Completed, Action::RetainRejection),
            (State::Prepared, Event::Cancel) => (State::Released, Action::ReleasePrepared),
            (State::RunningAttached, Event::Cancel) => (State::RunningDetached, Action::Detach),
            (State::RunningAttached, Event::Publish) => {
                (State::Completed, Action::RetainCompletion)
            }
            (State::RunningDetached, Event::Publish) => {
                (State::Released, Action::ReleaseDetachedCompletion)
            }
            (State::Completed, Event::Cancel) => (State::Released, Action::ReleaseCompletion),
            (State::Completed, Event::Consume) => (State::Released, Action::ConsumeCompletion),
            _ => return None,
        };
        Some(DarwinFileJobTransition { next, action })
    }
}

/// Shared state of one pre-reserved generated file-retirement record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinFileRetirementState {
    Available,
    Reserved,
    Live,
    ClosingAttached,
    ClosingDetached,
    Completed,
}

impl DarwinFileRetirementState {
    pub const ALL: &'static [Self] = &[
        Self::Available,
        Self::Reserved,
        Self::Live,
        Self::ClosingAttached,
        Self::ClosingDetached,
        Self::Completed,
    ];

    #[must_use]
    pub const fn code(self) -> u64 {
        match self {
            Self::Available => 0,
            Self::Reserved => 1,
            Self::Live => 2,
            Self::ClosingAttached => 3,
            Self::ClosingDetached => 4,
            Self::Completed => 5,
        }
    }

    #[must_use]
    pub const fn from_code(code: u64) -> Option<Self> {
        match code {
            0 => Some(Self::Available),
            1 => Some(Self::Reserved),
            2 => Some(Self::Live),
            3 => Some(Self::ClosingAttached),
            4 => Some(Self::ClosingDetached),
            5 => Some(Self::Completed),
            _ => None,
        }
    }
}

/// One ownership transition attempted on a generated retirement record.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinFileRetirementEvent {
    Reserve,
    PublishOwner,
    AbandonReservation,
    BeginClose,
    DropOwner,
    Cancel,
    PublishClose,
    Consume,
}

/// Complete ownership work authorized by one valid retirement transition.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinFileRetirementAction {
    RetainReservation,
    PublishOwner,
    ReleaseReservation,
    EnqueueAttachedClose,
    EnqueueDetachedClose,
    DetachClose,
    RetainCloseCompletion,
    ReleaseDetachedClose,
    ReleaseCloseCompletion,
    ConsumeCloseCompletion,
}

/// One validated generated retirement transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinFileRetirementTransition {
    next: DarwinFileRetirementState,
    action: DarwinFileRetirementAction,
}

impl DarwinFileRetirementTransition {
    #[must_use]
    pub const fn next(self) -> DarwinFileRetirementState {
        self.next
    }

    #[must_use]
    pub const fn action(self) -> DarwinFileRetirementAction {
        self.action
    }
}

impl DarwinFileRetirementState {
    /// Applies one retirement event while keeping descriptor ownership in exactly one state.
    #[must_use]
    pub const fn apply(
        self,
        event: DarwinFileRetirementEvent,
    ) -> Option<DarwinFileRetirementTransition> {
        use DarwinFileRetirementAction as Action;
        use DarwinFileRetirementEvent as Event;
        use DarwinFileRetirementState as State;

        let (next, action) = match (self, event) {
            (State::Available, Event::Reserve) => (State::Reserved, Action::RetainReservation),
            (State::Reserved, Event::PublishOwner) => (State::Live, Action::PublishOwner),
            (State::Reserved, Event::AbandonReservation) => {
                (State::Available, Action::ReleaseReservation)
            }
            (State::Live, Event::BeginClose) => {
                (State::ClosingAttached, Action::EnqueueAttachedClose)
            }
            (State::Live, Event::DropOwner) => {
                (State::ClosingDetached, Action::EnqueueDetachedClose)
            }
            (State::ClosingAttached, Event::Cancel) => {
                (State::ClosingDetached, Action::DetachClose)
            }
            (State::ClosingAttached, Event::PublishClose) => {
                (State::Completed, Action::RetainCloseCompletion)
            }
            (State::ClosingDetached, Event::PublishClose) => {
                (State::Available, Action::ReleaseDetachedClose)
            }
            (State::Completed, Event::Cancel) => (State::Available, Action::ReleaseCloseCompletion),
            (State::Completed, Event::Consume) => {
                (State::Available, Action::ConsumeCloseCompletion)
            }
            _ => return None,
        };
        Some(DarwinFileRetirementTransition { next, action })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DarwinFileJobAction as JobAction, DarwinFileJobEvent as JobEvent,
        DarwinFileJobState as JobState, DarwinFileRetirementAction as RetirementAction,
        DarwinFileRetirementEvent as RetirementEvent, DarwinFileRetirementState as RetirementState,
    };

    #[test]
    fn job_state_codes_are_closed_unique_and_round_trip() {
        let codes = JobState::ALL
            .iter()
            .copied()
            .map(JobState::code)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(codes.len(), JobState::ALL.len());
        for state in JobState::ALL.iter().copied() {
            assert_eq!(JobState::from_code(state.code()), Some(state));
        }
        assert_eq!(JobState::from_code(u64::MAX), None);
    }

    #[test]
    fn attached_and_detached_job_completion_have_disjoint_owners() {
        let attached = JobState::Prepared.apply(JobEvent::Admit).unwrap();
        assert_eq!(attached.next(), JobState::RunningAttached);
        assert_eq!(attached.action(), JobAction::Dispatch);
        let completed = attached.next().apply(JobEvent::Publish).unwrap();
        assert_eq!(completed.next(), JobState::Completed);
        assert_eq!(completed.action(), JobAction::RetainCompletion);
        let consumed = completed.next().apply(JobEvent::Consume).unwrap();
        assert_eq!(consumed.next(), JobState::Released);
        assert_eq!(consumed.action(), JobAction::ConsumeCompletion);

        let detached = attached.next().apply(JobEvent::Cancel).unwrap();
        assert_eq!(detached.next(), JobState::RunningDetached);
        assert_eq!(detached.action(), JobAction::Detach);
        let released = detached.next().apply(JobEvent::Publish).unwrap();
        assert_eq!(released.next(), JobState::Released);
        assert_eq!(released.action(), JobAction::ReleaseDetachedCompletion);
    }

    #[test]
    fn closed_admission_has_one_prepared_to_completed_transition() {
        let rejected = JobState::Prepared.apply(JobEvent::Reject).unwrap();
        assert_eq!(rejected.next(), JobState::Completed);
        assert_eq!(rejected.action(), JobAction::RetainRejection);
        assert_eq!(JobState::RunningAttached.apply(JobEvent::Reject), None);
    }

    #[test]
    fn invalid_job_events_cannot_authorize_cleanup() {
        for (state, event) in [
            (JobState::Prepared, JobEvent::Publish),
            (JobState::RunningAttached, JobEvent::Consume),
            (JobState::RunningDetached, JobEvent::Consume),
            (JobState::Completed, JobEvent::Publish),
            (JobState::Released, JobEvent::Cancel),
        ] {
            assert_eq!(state.apply(event), None);
        }
    }

    #[test]
    fn retirement_state_codes_are_closed_unique_and_round_trip() {
        let codes = RetirementState::ALL
            .iter()
            .copied()
            .map(RetirementState::code)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(codes.len(), RetirementState::ALL.len());
        for state in RetirementState::ALL.iter().copied() {
            assert_eq!(RetirementState::from_code(state.code()), Some(state));
        }
        assert_eq!(RetirementState::from_code(u64::MAX), None);
    }

    #[test]
    fn retirement_reservation_closes_every_owner_path() {
        let reserved = RetirementState::Available
            .apply(RetirementEvent::Reserve)
            .unwrap();
        assert_eq!(reserved.action(), RetirementAction::RetainReservation);
        let live = reserved
            .next()
            .apply(RetirementEvent::PublishOwner)
            .unwrap();
        assert_eq!(live.next(), RetirementState::Live);
        let closing = live.next().apply(RetirementEvent::BeginClose).unwrap();
        assert_eq!(closing.action(), RetirementAction::EnqueueAttachedClose);
        let completed = closing.next().apply(RetirementEvent::PublishClose).unwrap();
        assert_eq!(completed.action(), RetirementAction::RetainCloseCompletion);
        let available = completed.next().apply(RetirementEvent::Consume).unwrap();
        assert_eq!(available.next(), RetirementState::Available);
        assert_eq!(available.action(), RetirementAction::ConsumeCloseCompletion);

        let dropped = live.next().apply(RetirementEvent::DropOwner).unwrap();
        assert_eq!(dropped.action(), RetirementAction::EnqueueDetachedClose);
        let released = dropped.next().apply(RetirementEvent::PublishClose).unwrap();
        assert_eq!(released.next(), RetirementState::Available);
        assert_eq!(released.action(), RetirementAction::ReleaseDetachedClose);
    }

    #[test]
    fn attached_close_cancellation_only_detaches_the_waiter() {
        let detached = RetirementState::ClosingAttached
            .apply(RetirementEvent::Cancel)
            .unwrap();
        assert_eq!(detached.next(), RetirementState::ClosingDetached);
        assert_eq!(detached.action(), RetirementAction::DetachClose);
        assert_eq!(
            detached.next().apply(RetirementEvent::Consume),
            None,
            "detached cleanup cannot be consumed by the former waiter"
        );

        let cancelled_completion = RetirementState::Completed
            .apply(RetirementEvent::Cancel)
            .unwrap();
        assert_eq!(cancelled_completion.next(), RetirementState::Available);
        assert_eq!(
            cancelled_completion.action(),
            RetirementAction::ReleaseCloseCompletion
        );
    }
}
