use std::collections::{BTreeMap, btree_map::Entry};
use std::fmt;

use crate::{Reactor, ReactorInterest, Scheduler, SchedulerError, SchedulerProgress, TaskId};

/// A non-empty wait set returned by one deferred computation poll.
///
/// The type makes the scheduler's non-empty suspension precondition structural. Executor clients
/// cannot publish a pending state that has no possible wakeup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaitSet {
    interests: Box<[ReactorInterest]>,
}

impl WaitSet {
    #[must_use]
    pub fn one(interest: ReactorInterest) -> Self {
        Self {
            interests: Box::new([interest]),
        }
    }

    #[must_use]
    pub fn new(
        first: ReactorInterest,
        remaining: impl IntoIterator<Item = ReactorInterest>,
    ) -> Self {
        let interests = std::iter::once(first)
            .chain(remaining)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self { interests }
    }

    #[must_use]
    pub const fn interests(&self) -> &[ReactorInterest] {
        &self.interests
    }
}

/// One result from polling an executor-owned computation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComputationPoll {
    Pending(WaitSet),
    Completed,
}

/// Opaque computation payload owned by [`Executor`].
///
/// Implementations own their frame representation and lifecycle entry mechanism. The executor
/// observes only poll status, wait interests, cancellation, and completed-output consumption.
pub trait Computation {
    type Output;

    fn resume(&mut self) -> ComputationPoll;
    fn cancel(self);
    fn consume(self) -> Self::Output;
}

/// Observable result of one executor scheduling step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutorProgress {
    Resumed(TaskId),
    Idle,
}

/// Single-threaded owner of opaque computation payloads and their scheduler identities.
///
/// The embedded scheduler remains the only lifecycle and wakeup authority. This layer pairs each
/// live task identity with exactly one payload and invokes payload lifecycle operations only after
/// the scheduler has published the corresponding transition.
pub struct Executor<R, C> {
    scheduler: Scheduler<R>,
    computations: BTreeMap<TaskId, C>,
}

impl<R: Reactor, C: Computation> Executor<R, C> {
    #[must_use]
    pub fn new(reactor: R) -> Self {
        Self {
            scheduler: Scheduler::new(reactor),
            computations: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn scheduler(&self) -> &Scheduler<R> {
        &self.scheduler
    }

    /// Transfers one lazy computation into the executor and schedules its first poll.
    ///
    /// # Errors
    ///
    /// Returns scheduler identity exhaustion or an internal identity/payload inconsistency.
    pub fn spawn(&mut self, computation: C) -> Result<TaskId, ExecutorError<R::Error>> {
        let task = self.scheduler.spawn()?;
        match self.computations.entry(task) {
            Entry::Vacant(entry) => {
                entry.insert(computation);
            }
            Entry::Occupied(_) => return Err(ExecutorError::DuplicateComputation(task)),
        }
        Ok(task)
    }

    /// Polls the next runnable computation, blocking in the reactor only when required.
    ///
    /// A failed reactor registration restores the task to the runnable queue before the error is
    /// returned, so the executor never leaks a task in its transient running state.
    ///
    /// # Errors
    ///
    /// Returns a scheduler/reactor failure or an internal identity/payload inconsistency.
    pub fn step(&mut self) -> Result<ExecutorProgress, ExecutorError<R::Error>> {
        let SchedulerProgress::Resume(task) = self.scheduler.next_progress()? else {
            return Ok(ExecutorProgress::Idle);
        };
        let Some(computation) = self.computations.get_mut(&task) else {
            self.scheduler
                .yield_now(task)
                .map_err(ExecutorError::Recovery)?;
            return Err(ExecutorError::MissingComputation(task));
        };
        let poll = computation.resume();
        match poll {
            ComputationPoll::Pending(wait) => {
                if let Err(error) = self
                    .scheduler
                    .suspend(task, wait.interests().iter().copied())
                {
                    self.scheduler
                        .yield_now(task)
                        .map_err(ExecutorError::Recovery)?;
                    return Err(ExecutorError::Scheduler(error));
                }
            }
            ComputationPoll::Completed => self.scheduler.complete(task)?,
        }
        Ok(ExecutorProgress::Resumed(task))
    }

    /// Runs scheduling steps until no runnable task or active wait registration remains.
    ///
    /// # Errors
    ///
    /// Returns the first error from [`Self::step`].
    pub fn run_until_idle(&mut self) -> Result<(), ExecutorError<R::Error>> {
        loop {
            let progress = self.step()?;
            if matches!(progress, ExecutorProgress::Idle) && !self.scheduler.has_progress_source() {
                break;
            }
        }
        Ok(())
    }

    /// Cancels one pending or completed task after detaching every wakeup.
    ///
    /// # Errors
    ///
    /// Rejects an unknown/non-cancellable task or an internal identity/payload inconsistency.
    pub fn cancel(&mut self, task: TaskId) -> Result<(), ExecutorError<R::Error>> {
        if !self.computations.contains_key(&task) {
            return Err(ExecutorError::MissingComputation(task));
        }
        self.scheduler.cancel(task)?;
        let computation = self
            .computations
            .remove(&task)
            .ok_or(ExecutorError::MissingComputation(task))?;
        computation.cancel();
        self.scheduler.finish_cancellation(task)?;
        Ok(())
    }

    /// Consumes one completed task and releases its generation-qualified identity.
    ///
    /// # Errors
    ///
    /// Rejects an unknown/incomplete task or an internal identity/payload inconsistency.
    pub fn consume(&mut self, task: TaskId) -> Result<C::Output, ExecutorError<R::Error>> {
        if !self.computations.contains_key(&task) {
            return Err(ExecutorError::MissingComputation(task));
        }
        self.scheduler.consume_completed(task)?;
        self.computations
            .remove(&task)
            .map(Computation::consume)
            .ok_or(ExecutorError::MissingComputation(task))
    }

    /// Cancels every retained computation through the scheduler's orderly shutdown transition.
    ///
    /// # Errors
    ///
    /// Rejects shutdown during an active poll, a scheduler/reactor failure, or an internal
    /// identity/payload inconsistency.
    pub fn shutdown(&mut self) -> Result<(), ExecutorError<R::Error>> {
        for cancellation in self.scheduler.begin_shutdown()?.iter().copied() {
            let task = cancellation.task();
            let computation = self
                .computations
                .remove(&task)
                .ok_or(ExecutorError::MissingComputation(task))?;
            computation.cancel();
            self.scheduler.finish_cancellation(task)?;
        }
        Ok(())
    }
}

/// Executor orchestration failure.
#[derive(Debug)]
pub enum ExecutorError<E> {
    Scheduler(SchedulerError<E>),
    Recovery(SchedulerError<E>),
    MissingComputation(TaskId),
    DuplicateComputation(TaskId),
}

impl<E: fmt::Debug> fmt::Display for ExecutorError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "asynchronous executor transition failed: {self:?}"
        )
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ExecutorError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Scheduler(error) | Self::Recovery(error) => Some(error),
            Self::MissingComputation(_) | Self::DuplicateComputation(_) => None,
        }
    }
}

impl<E> From<SchedulerError<E>> for ExecutorError<E> {
    fn from(error: SchedulerError<E>) -> Self {
        Self::Scheduler(error)
    }
}
