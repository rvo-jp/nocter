use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::identity::ServiceIdentity;
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

/// Waiter-visible lifecycle state of one current job identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
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
    ForeignCapacityEpoch(CapacityEpoch),
    UnknownJob(JobId),
    InvalidJobState(JobId),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "blocking-job lifecycle failed: {self:?}")
    }
}

impl std::error::Error for ServiceError {}

enum ActiveJobState<O> {
    Running,
    AbandonedRunning,
    Completed(JobOutcome<O>),
}

struct Inner<I, O> {
    identity: ServiceIdentity,
    capacity: ServiceCapacity,
    accepting: bool,
    next_job: u64,
    capacity_epoch: CapacityEpoch,
    queue: VecDeque<(JobId, I)>,
    active: BTreeMap<JobId, ActiveJobState<O>>,
    notify: Arc<dyn Fn() + Send + Sync>,
}

impl<I, O> Inner<I, O> {
    fn snapshot(&self) -> ServiceSnapshot {
        let mut completed = 0;
        let mut abandoned_running = 0;
        for state in self.active.values() {
            match state {
                ActiveJobState::Completed(_) => completed += 1,
                ActiveJobState::AbandonedRunning => abandoned_running += 1,
                ActiveJobState::Running => {}
            }
        }
        let running = self
            .active
            .values()
            .filter(|state| matches!(state, ActiveJobState::Running))
            .count();
        ServiceSnapshot {
            accepting: self.accepting,
            queued: self.queue.len(),
            running,
            completed,
            abandoned_running,
            capacity_epoch: self.capacity_epoch,
        }
    }

    fn release_capacity(&mut self) {
        self.capacity_epoch.advance();
    }

    fn running_count(&self) -> usize {
        self.active
            .values()
            .filter(|state| {
                matches!(
                    state,
                    ActiveJobState::Running | ActiveJobState::AbandonedRunning
                )
            })
            .count()
    }

    fn admitted_count(&self) -> usize {
        self.queue.len() + self.active.len()
    }
}

/// Bounded owner of opaque blocking-job lifecycle state.
pub struct BlockingJobService<I, O> {
    inner: Arc<Mutex<Inner<I, O>>>,
}

impl<I, O> Clone for BlockingJobService<I, O> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<I, O> BlockingJobService<I, O> {
    #[must_use]
    pub fn new(capacity: ServiceCapacity) -> Self {
        Self::with_notifier(capacity, || {})
    }

    /// Creates a service whose lifecycle transitions invoke one wake-only notifier.
    ///
    /// The notifier runs after the state lock is released. It must not assign meaning to a wake;
    /// observers query their exact job or capacity epoch after resuming.
    #[must_use]
    pub fn with_notifier<N>(capacity: ServiceCapacity, notify: N) -> Self
    where
        N: Fn() + Send + Sync + 'static,
    {
        let identity = ServiceIdentity::allocate();
        Self {
            inner: Arc::new(Mutex::new(Inner {
                identity,
                capacity,
                accepting: true,
                next_job: 0,
                capacity_epoch: CapacityEpoch::initial(identity),
                queue: VecDeque::new(),
                active: BTreeMap::new(),
                notify: Arc::new(notify),
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

    /// Returns the waiter-visible state only while this exact identity remains current.
    #[must_use]
    pub fn status(&self, job: JobId) -> Option<JobStatus> {
        let inner = lock(&self.inner);
        if inner.queue.iter().any(|(queued, _)| *queued == job) {
            return Some(JobStatus::Queued);
        }
        inner.active.get(&job).and_then(|state| match state {
            ActiveJobState::Running => Some(JobStatus::Running),
            ActiveJobState::Completed(_) => Some(JobStatus::Completed),
            ActiveJobState::AbandonedRunning => None,
        })
    }

    /// Whether capacity has changed since a rejected admission observed its epoch.
    ///
    /// # Errors
    ///
    /// Rejects an observation issued by another lifecycle service.
    pub fn capacity_changed_since(&self, observed: CapacityEpoch) -> Result<bool, ServiceError> {
        let inner = lock(&self.inner);
        if !observed.belongs_to(inner.identity) {
            return Err(ServiceError::ForeignCapacityEpoch(observed));
        }
        Ok(inner.capacity_epoch != observed)
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
        if inner.admitted_count() >= inner.capacity.maximum_jobs {
            return Err(SubmitError::Saturated(Backpressure {
                input,
                observed_epoch: inner.capacity_epoch,
            }));
        }
        let Some(next_job) = inner.next_job.checked_add(1) else {
            return Err(SubmitError::IdentityExhausted(input));
        };
        let job = JobId::new(inner.identity, inner.next_job);
        inner.next_job = next_job;
        inner.queue.push_back((job, input));
        Ok(job)
    }

    /// Transfers the next queued input to one worker-owned guard when a worker is available.
    #[must_use]
    pub fn claim(&self) -> Option<RunningJob<I, O>> {
        let mut inner = lock(&self.inner);
        if inner.running_count() >= inner.capacity.workers {
            return None;
        }
        if let Some((job, input)) = inner.queue.pop_front() {
            let previous = inner.active.insert(job, ActiveJobState::Running);
            debug_assert!(previous.is_none());
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
        if let Some(position) = inner.queue.iter().position(|(queued, _)| *queued == job) {
            let input = inner
                .queue
                .remove(position)
                .map(|(_, input)| input)
                .ok_or(ServiceError::UnknownJob(job))?;
            inner.release_capacity();
            let notify = Arc::clone(&inner.notify);
            drop(inner);
            notify();
            return Ok(Cancellation::Queued(input));
        }
        let state = inner
            .active
            .remove(&job)
            .ok_or(ServiceError::UnknownJob(job))?;
        match state {
            ActiveJobState::Running => {
                inner.active.insert(job, ActiveJobState::AbandonedRunning);
                Ok(Cancellation::Running)
            }
            ActiveJobState::Completed(output) => {
                inner.release_capacity();
                let notify = Arc::clone(&inner.notify);
                drop(inner);
                notify();
                Ok(Cancellation::Completed(output))
            }
            ActiveJobState::AbandonedRunning => {
                inner.active.insert(job, ActiveJobState::AbandonedRunning);
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
            .active
            .remove(&job)
            .ok_or(ServiceError::UnknownJob(job))?;
        let ActiveJobState::Completed(output) = state else {
            inner.active.insert(job, state);
            return Err(ServiceError::InvalidJobState(job));
        };
        inner.release_capacity();
        let notify = Arc::clone(&inner.notify);
        drop(inner);
        notify();
        Ok(output)
    }

    /// Closes admission and detaches every value not owned by a running worker.
    ///
    /// Running jobs become abandoned and keep the shared service state alive until their guards
    /// complete or drop.
    #[must_use]
    pub fn begin_shutdown(&self) -> ShutdownCleanup<I, O> {
        let mut inner = lock(&self.inner);
        inner.accepting = false;
        let queued = inner
            .queue
            .drain(..)
            .map(|(_, input)| input)
            .collect::<Vec<_>>();
        for _ in &queued {
            inner.release_capacity();
        }
        let jobs = inner.active.keys().copied().collect::<Vec<_>>();
        let mut completed = Vec::new();
        for job in jobs {
            let Some(state) = inner.active.remove(&job) else {
                continue;
            };
            match state {
                ActiveJobState::Completed(output) => {
                    completed.push(output);
                    inner.release_capacity();
                }
                ActiveJobState::Running | ActiveJobState::AbandonedRunning => {
                    inner.active.insert(job, ActiveJobState::AbandonedRunning);
                }
            }
        }
        let notify = Arc::clone(&inner.notify);
        drop(inner);
        notify();
        ShutdownCleanup { queued, completed }
    }

    #[must_use]
    pub fn is_drained(&self) -> bool {
        let inner = lock(&self.inner);
        inner.queue.is_empty() && inner.active.is_empty()
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
            .active
            .remove(&self.job)
            .ok_or(ServiceError::UnknownJob(self.job))?;
        let abandoned = match state {
            ActiveJobState::Running => false,
            ActiveJobState::AbandonedRunning => true,
            state @ ActiveJobState::Completed(_) => {
                inner.active.insert(self.job, state);
                return Err(ServiceError::InvalidJobState(self.job));
            }
        };
        self.retired = true;
        if abandoned {
            inner.release_capacity();
            let notify = Arc::clone(&inner.notify);
            drop(inner);
            notify();
            Ok(Some(output))
        } else {
            inner.active.insert(
                self.job,
                ActiveJobState::Completed(JobOutcome::Completed(output)),
            );
            let notify = Arc::clone(&inner.notify);
            drop(inner);
            notify();
            Ok(None)
        }
    }
}

impl<I, O> Drop for RunningJob<I, O> {
    fn drop(&mut self) {
        if self.retired {
            return;
        }
        let notify = {
            let mut inner = lock(&self.inner);
            let Some(state) = inner.active.remove(&self.job) else {
                return;
            };
            match state {
                ActiveJobState::Running => {
                    inner
                        .active
                        .insert(self.job, ActiveJobState::Completed(JobOutcome::WorkerLost));
                    Some(Arc::clone(&inner.notify))
                }
                ActiveJobState::AbandonedRunning => {
                    inner.release_capacity();
                    Some(Arc::clone(&inner.notify))
                }
                state @ ActiveJobState::Completed(_) => {
                    inner.active.insert(self.job, state);
                    None
                }
            }
        };
        if let Some(notify) = notify {
            notify();
        }
    }
}

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
