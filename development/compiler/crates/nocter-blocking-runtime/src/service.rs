use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::{CapacityEpoch, JobId};

/// Fixed worker and total-admission limits for one service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceCapacity {
    workers: usize,
    maximum_jobs: usize,
}

impl ServiceCapacity {
    /// Creates a capacity whose admitted-job bound includes queued, running, and completed jobs.
    ///
    /// # Errors
    ///
    /// Rejects zero workers or a total bound smaller than the worker count.
    pub const fn new(workers: usize, maximum_jobs: usize) -> Result<Self, ServiceError> {
        if workers == 0 {
            return Err(ServiceError::ZeroWorkers);
        }
        if maximum_jobs < workers {
            return Err(ServiceError::JobCapacityBelowWorkers {
                workers,
                maximum_jobs,
            });
        }
        Ok(Self {
            workers,
            maximum_jobs,
        })
    }

    #[must_use]
    pub const fn workers(self) -> usize {
        self.workers
    }

    #[must_use]
    pub const fn maximum_jobs(self) -> usize {
        self.maximum_jobs
    }
}

/// Read-only service counters at one instant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceSnapshot {
    accepting: bool,
    queued: usize,
    running: usize,
    completed: usize,
    abandoned_running: usize,
    capacity_epoch: CapacityEpoch,
}

impl ServiceSnapshot {
    #[must_use]
    pub const fn accepting(self) -> bool {
        self.accepting
    }

    #[must_use]
    pub const fn queued(self) -> usize {
        self.queued
    }

    #[must_use]
    pub const fn running(self) -> usize {
        self.running
    }

    #[must_use]
    pub const fn completed(self) -> usize {
        self.completed
    }

    #[must_use]
    pub const fn abandoned_running(self) -> usize {
        self.abandoned_running
    }

    #[must_use]
    pub const fn capacity_epoch(self) -> CapacityEpoch {
        self.capacity_epoch
    }

    #[must_use]
    pub const fn admitted(self) -> usize {
        self.queued + self.running + self.completed + self.abandoned_running
    }
}

/// Saturation evidence returned with input ownership intact.
#[derive(Debug, Eq, PartialEq)]
pub struct Backpressure<I> {
    input: I,
    observed_epoch: CapacityEpoch,
}

impl<I> Backpressure<I> {
    #[must_use]
    pub fn into_input(self) -> I {
        self.input
    }

    #[must_use]
    pub const fn observed_epoch(&self) -> CapacityEpoch {
        self.observed_epoch
    }

    #[must_use]
    pub fn into_parts(self) -> (I, CapacityEpoch) {
        (self.input, self.observed_epoch)
    }
}

/// Failure to admit a job without losing its input.
#[derive(Debug, Eq, PartialEq)]
pub enum SubmitError<I> {
    Closed(I),
    Saturated(Backpressure<I>),
    IdentityExhausted(I),
}

impl<I> SubmitError<I> {
    #[must_use]
    pub fn into_input(self) -> I {
        match self {
            Self::Closed(input) | Self::IdentityExhausted(input) => input,
            Self::Saturated(backpressure) => backpressure.into_input(),
        }
    }
}

/// Terminal outcome retained for a waiter.
#[derive(Debug, Eq, PartialEq)]
pub enum JobOutcome<O> {
    Completed(O),
    /// The running owner disappeared without publishing an output.
    WorkerLost,
}

/// Ownership returned when cancellation detaches a future from one job.
#[derive(Debug, Eq, PartialEq)]
pub enum Cancellation<I, O> {
    Queued(I),
    Running,
    Completed(JobOutcome<O>),
}

/// Values detached by orderly shutdown. Dropping this value performs their ordinary cleanup.
#[derive(Debug, Eq, PartialEq)]
pub struct ShutdownCleanup<I, O> {
    queued: Vec<I>,
    completed: Vec<JobOutcome<O>>,
}

impl<I, O> ShutdownCleanup<I, O> {
    #[must_use]
    pub fn into_parts(self) -> (Vec<I>, Vec<JobOutcome<O>>) {
        (self.queued, self.completed)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceError {
    ZeroWorkers,
    JobCapacityBelowWorkers { workers: usize, maximum_jobs: usize },
    UnknownJob(JobId),
    InvalidJobState(JobId),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "blocking-job lifecycle failed: {self:?}")
    }
}

impl std::error::Error for ServiceError {}

enum JobState<I, O> {
    Queued(I),
    Running,
    AbandonedRunning,
    Completed(JobOutcome<O>),
}

struct Inner<I, O> {
    capacity: ServiceCapacity,
    accepting: bool,
    next_job: u64,
    capacity_epoch: CapacityEpoch,
    states: BTreeMap<JobId, JobState<I, O>>,
    queue: VecDeque<JobId>,
    running: usize,
    completions: VecDeque<JobId>,
}

impl<I, O> Inner<I, O> {
    fn snapshot(&self) -> ServiceSnapshot {
        let mut queued = 0;
        let mut completed = 0;
        let mut abandoned_running = 0;
        for state in self.states.values() {
            match state {
                JobState::Queued(_) => queued += 1,
                JobState::Completed(_) => completed += 1,
                JobState::AbandonedRunning => abandoned_running += 1,
                JobState::Running => {}
            }
        }
        ServiceSnapshot {
            accepting: self.accepting,
            queued,
            running: self.running.saturating_sub(abandoned_running),
            completed,
            abandoned_running,
            capacity_epoch: self.capacity_epoch,
        }
    }

    fn release_capacity(&mut self) {
        self.capacity_epoch.advance();
    }
}

/// Bounded owner of opaque blocking-job lifecycle state.
pub struct BlockingJobService<I, O> {
    inner: Arc<Mutex<Inner<I, O>>>,
}

impl<I, O> BlockingJobService<I, O> {
    #[must_use]
    pub fn new(capacity: ServiceCapacity) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                capacity,
                accepting: true,
                next_job: 0,
                capacity_epoch: CapacityEpoch::INITIAL,
                states: BTreeMap::new(),
                queue: VecDeque::new(),
                running: 0,
                completions: VecDeque::new(),
            })),
        }
    }

    #[must_use]
    pub fn capacity(&self) -> ServiceCapacity {
        lock(&self.inner).capacity
    }

    #[must_use]
    pub fn snapshot(&self) -> ServiceSnapshot {
        lock(&self.inner).snapshot()
    }

    /// Whether capacity has changed since a rejected admission observed its epoch.
    #[must_use]
    pub fn capacity_changed_since(&self, observed: CapacityEpoch) -> bool {
        lock(&self.inner).capacity_epoch != observed
    }

    /// Transfers one input into bounded queue ownership.
    ///
    /// # Errors
    ///
    /// Returns closed, saturation, or identity-exhaustion evidence with the exact input retained.
    pub fn submit(&self, input: I) -> Result<JobId, SubmitError<I>> {
        let mut inner = lock(&self.inner);
        if !inner.accepting {
            return Err(SubmitError::Closed(input));
        }
        if inner.states.len() == inner.capacity.maximum_jobs {
            return Err(SubmitError::Saturated(Backpressure {
                input,
                observed_epoch: inner.capacity_epoch,
            }));
        }
        let Some(next_job) = inner.next_job.checked_add(1) else {
            return Err(SubmitError::IdentityExhausted(input));
        };
        let job = JobId::new(inner.next_job);
        inner.next_job = next_job;
        let previous = inner.states.insert(job, JobState::Queued(input));
        debug_assert!(previous.is_none());
        inner.queue.push_back(job);
        Ok(job)
    }

    /// Transfers the next queued input to one worker-owned guard when a worker is available.
    #[must_use]
    pub fn claim(&self) -> Option<RunningJob<I, O>> {
        let mut inner = lock(&self.inner);
        if inner.running == inner.capacity.workers {
            return None;
        }
        while let Some(job) = inner.queue.pop_front() {
            let Some(state) = inner.states.remove(&job) else {
                continue;
            };
            let JobState::Queued(input) = state else {
                inner.states.insert(job, state);
                continue;
            };
            inner.states.insert(job, JobState::Running);
            inner.running += 1;
            return Some(RunningJob {
                job,
                input,
                inner: Arc::clone(&self.inner),
                retired: false,
            });
        }
        None
    }

    /// Detaches one future owner and returns every value immediately available for cleanup.
    ///
    /// # Errors
    ///
    /// Rejects an unknown or already abandoned identity.
    pub fn cancel(&self, job: JobId) -> Result<Cancellation<I, O>, ServiceError> {
        let mut inner = lock(&self.inner);
        let state = inner
            .states
            .remove(&job)
            .ok_or(ServiceError::UnknownJob(job))?;
        match state {
            JobState::Queued(input) => {
                inner.release_capacity();
                Ok(Cancellation::Queued(input))
            }
            JobState::Running => {
                inner.states.insert(job, JobState::AbandonedRunning);
                Ok(Cancellation::Running)
            }
            JobState::Completed(output) => {
                inner.release_capacity();
                Ok(Cancellation::Completed(output))
            }
            JobState::AbandonedRunning => {
                inner.states.insert(job, JobState::AbandonedRunning);
                Err(ServiceError::InvalidJobState(job))
            }
        }
    }

    /// Transfers one completed outcome to its waiter and retires the admission slot.
    ///
    /// # Errors
    ///
    /// Rejects an unknown or non-completed identity.
    pub fn consume(&self, job: JobId) -> Result<JobOutcome<O>, ServiceError> {
        let mut inner = lock(&self.inner);
        let state = inner
            .states
            .remove(&job)
            .ok_or(ServiceError::UnknownJob(job))?;
        let JobState::Completed(output) = state else {
            inner.states.insert(job, state);
            return Err(ServiceError::InvalidJobState(job));
        };
        inner.release_capacity();
        Ok(output)
    }

    /// Drains completion identities in worker-publication order.
    ///
    /// An identity may already have been cancelled. Consumers validate it with `consume`; job IDs
    /// are never reused, so a stale event cannot refer to another job.
    #[must_use]
    pub fn completion_events(&self) -> Box<[JobId]> {
        lock(&self.inner)
            .completions
            .drain(..)
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    /// Closes admission and detaches every value not owned by a running worker.
    ///
    /// Running jobs become abandoned and keep the shared service state alive until their guards
    /// complete or drop.
    #[must_use]
    pub fn begin_shutdown(&self) -> ShutdownCleanup<I, O> {
        let mut inner = lock(&self.inner);
        inner.accepting = false;
        let jobs = inner.states.keys().copied().collect::<Vec<_>>();
        let mut queued = Vec::new();
        let mut completed = Vec::new();
        for job in jobs {
            let Some(state) = inner.states.remove(&job) else {
                continue;
            };
            match state {
                JobState::Queued(input) => {
                    queued.push(input);
                    inner.release_capacity();
                }
                JobState::Completed(output) => {
                    completed.push(output);
                    inner.release_capacity();
                }
                JobState::Running | JobState::AbandonedRunning => {
                    inner.states.insert(job, JobState::AbandonedRunning);
                }
            }
        }
        inner.queue.clear();
        inner.completions.clear();
        ShutdownCleanup { queued, completed }
    }

    #[must_use]
    pub fn is_drained(&self) -> bool {
        lock(&self.inner).states.is_empty()
    }
}

/// Sole owner of one claimed input and its required terminal lifecycle transition.
pub struct RunningJob<I, O> {
    job: JobId,
    input: I,
    inner: Arc<Mutex<Inner<I, O>>>,
    retired: bool,
}

impl<I, O> RunningJob<I, O> {
    #[must_use]
    pub const fn id(&self) -> JobId {
        self.job
    }

    #[must_use]
    pub const fn input(&self) -> &I {
        &self.input
    }

    #[must_use]
    pub fn input_mut(&mut self) -> &mut I {
        &mut self.input
    }

    /// Publishes one outcome or returns it when the waiter abandoned the running job.
    ///
    /// # Errors
    ///
    /// Returns an invalid-state error only when service and worker ownership disagree.
    pub fn complete(mut self, output: O) -> Result<Option<O>, ServiceError> {
        let mut inner = lock(&self.inner);
        let state = inner
            .states
            .remove(&self.job)
            .ok_or(ServiceError::UnknownJob(self.job))?;
        let abandoned = match state {
            JobState::Running => false,
            JobState::AbandonedRunning => true,
            state @ (JobState::Queued(_) | JobState::Completed(_)) => {
                inner.states.insert(self.job, state);
                return Err(ServiceError::InvalidJobState(self.job));
            }
        };
        inner.running -= 1;
        self.retired = true;
        if abandoned {
            inner.release_capacity();
            Ok(Some(output))
        } else {
            inner
                .states
                .insert(self.job, JobState::Completed(JobOutcome::Completed(output)));
            inner.completions.push_back(self.job);
            Ok(None)
        }
    }
}

impl<I, O> Drop for RunningJob<I, O> {
    fn drop(&mut self) {
        if self.retired {
            return;
        }
        let mut inner = lock(&self.inner);
        let Some(state) = inner.states.remove(&self.job) else {
            return;
        };
        match state {
            JobState::Running => {
                inner.running -= 1;
                inner
                    .states
                    .insert(self.job, JobState::Completed(JobOutcome::WorkerLost));
                inner.completions.push_back(self.job);
            }
            JobState::AbandonedRunning => {
                inner.running -= 1;
                inner.release_capacity();
            }
            state @ (JobState::Queued(_) | JobState::Completed(_)) => {
                inner.states.insert(self.job, state);
            }
        }
    }
}

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
