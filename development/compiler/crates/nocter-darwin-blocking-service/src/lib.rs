//! Darwin fixed-worker adapter for the target-independent blocking-job lifecycle.

use std::fmt;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

use nocter_blocking_runtime::{
    BlockingJobService, Cancellation, CapacityEpoch, JobId, JobOutcome, JobStatus, ServiceCapacity,
    ServiceError, ServiceSnapshot, ShutdownCleanup, SubmitError,
};

mod retirement_service;

pub use retirement_service::{DarwinRetirementService, RetirementShutdownError};

#[derive(Default)]
struct WorkerControl {
    revision: u128,
    shutdown: bool,
}

struct WorkerSignal {
    state: Mutex<WorkerControl>,
    changed: Condvar,
}

impl WorkerSignal {
    fn new() -> Self {
        Self {
            state: Mutex::new(WorkerControl::default()),
            changed: Condvar::new(),
        }
    }

    fn revision(&self) -> u128 {
        lock(&self.state).revision
    }

    fn notify_one(&self) {
        let mut state = lock(&self.state);
        state.revision = state.revision.saturating_add(1);
        self.changed.notify_one();
    }

    fn notify_all(&self) {
        let mut state = lock(&self.state);
        state.revision = state.revision.saturating_add(1);
        self.changed.notify_all();
    }

    fn close(&self) {
        let mut state = lock(&self.state);
        state.shutdown = true;
        state.revision = state.revision.saturating_add(1);
        self.changed.notify_all();
    }

    fn wait_for_change(&self, observed: u128) -> bool {
        let mut state = lock(&self.state);
        while !state.shutdown && state.revision == observed {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        state.shutdown
    }
}

/// Fixed Darwin worker pool over one opaque operation.
pub struct DarwinBlockingService<I, O> {
    jobs: BlockingJobService<I, O>,
    workers: Vec<JoinHandle<()>>,
    worker_signal: Arc<WorkerSignal>,
    notification_reader: UnixStream,
    stopped: bool,
}

impl<I: Send + 'static, O: Send + 'static> DarwinBlockingService<I, O> {
    /// Creates every worker and the nonblocking wake channel as one owned service.
    ///
    /// # Errors
    ///
    /// Returns channel or thread construction failure after stopping and joining any worker that
    /// was already created.
    pub fn new<F>(capacity: ServiceCapacity, operation: F) -> Result<Self, BuildError>
    where
        F: Fn(&mut I) -> O + Send + Sync + 'static,
    {
        let (notification_reader, notification_writer) = notification_channel()?;

        let notifier = notification_writer
            .try_clone()
            .map_err(BuildError::Channel)?;
        let jobs = BlockingJobService::with_notifier(capacity, move || signal(&notifier));
        let worker_signal = Arc::new(WorkerSignal::new());
        let operation = Arc::new(operation);
        let mut workers = Vec::with_capacity(capacity.workers());
        for index in 0..capacity.workers() {
            let worker_jobs = jobs.clone();
            let worker_control = Arc::clone(&worker_signal);
            let operation = Arc::clone(&operation);
            match thread::Builder::new()
                .name(format!("nocter-blocking-{index}"))
                .spawn(move || worker_loop(&worker_jobs, &worker_control, &*operation))
            {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    stop_workers(&jobs, &worker_signal, &mut workers);
                    return Err(BuildError::Worker(error));
                }
            }
        }
        Ok(Self {
            jobs,
            workers,
            worker_signal,
            notification_reader,
            stopped: false,
        })
    }

    #[must_use]
    pub fn notification_descriptor(&self) -> RawFd {
        self.notification_reader.as_raw_fd()
    }

    #[must_use]
    pub fn snapshot(&self) -> ServiceSnapshot {
        self.jobs.snapshot()
    }

    #[must_use]
    pub fn status(&self, job: JobId) -> Option<JobStatus> {
        self.jobs.status(job)
    }

    /// Reports whether this service released capacity after the supplied observation.
    ///
    /// # Errors
    ///
    /// Rejects an observation issued by another lifecycle service.
    pub fn capacity_changed_since(&self, observed: CapacityEpoch) -> Result<bool, ServiceError> {
        self.jobs.capacity_changed_since(observed)
    }

    /// Transfers one input into the bounded service and wakes one worker.
    ///
    /// # Errors
    ///
    /// Returns the lifecycle admission error with input ownership preserved.
    pub fn submit(&self, input: I) -> Result<JobId, SubmitError<I>> {
        let job = self.jobs.submit(input)?;
        self.worker_signal.notify_one();
        Ok(job)
    }

    /// Detaches a waiter and wakes capacity observers when a slot became free immediately.
    ///
    /// # Errors
    ///
    /// Returns identity or lifecycle mismatch from the sole lifecycle authority.
    pub fn cancel(&self, job: JobId) -> Result<Cancellation<I, O>, ServiceError> {
        let cancellation = self.jobs.cancel(job)?;
        Ok(cancellation)
    }

    /// Consumes one exact outcome and wakes capacity observers.
    ///
    /// # Errors
    ///
    /// Returns identity or lifecycle mismatch from the sole lifecycle authority.
    pub fn consume(&self, job: JobId) -> Result<JobOutcome<O>, ServiceError> {
        let output = self.jobs.consume(job)?;
        Ok(output)
    }

    /// Drains every currently readable wake byte.
    ///
    /// # Errors
    ///
    /// Returns a channel read failure other than ordinary nonblocking exhaustion.
    pub fn drain_notifications(&mut self) -> Result<usize, io::Error> {
        drain_channel(&mut self.notification_reader)
    }

    /// Closes admission, joins every worker, and returns detached input and output ownership.
    ///
    /// # Errors
    ///
    /// Reports an unexpected worker panic outside the operation boundary.
    pub fn shutdown(&mut self) -> Result<ShutdownCleanup<I, O>, ShutdownError> {
        let cleanup = self.jobs.begin_shutdown();
        self.worker_signal.close();
        self.stopped = true;
        join_workers(&mut self.workers)?;
        Ok(cleanup)
    }
}

impl<I, O> Drop for DarwinBlockingService<I, O> {
    fn drop(&mut self) {
        if self.stopped {
            return;
        }
        let _cleanup = self.jobs.begin_shutdown();
        self.worker_signal.close();
        let _ = join_workers(&mut self.workers);
        self.stopped = true;
    }
}

fn worker_loop<I, O, F>(
    jobs: &BlockingJobService<I, O>,
    worker_signal: &WorkerSignal,
    operation: &F,
) where
    F: Fn(&mut I) -> O,
{
    loop {
        let observed = worker_signal.revision();
        if let Some(mut job) = jobs.claim() {
            let output = catch_unwind(AssertUnwindSafe(|| operation(job.input_mut())));
            match output {
                Ok(output) => {
                    let _ = job.complete(output);
                }
                Err(_) => drop(job),
            }
            continue;
        }
        if worker_signal.wait_for_change(observed) {
            return;
        }
    }
}

fn signal(stream: &UnixStream) {
    loop {
        match (&*stream).write(&[1]) {
            Ok(1) => return,
            Ok(_) => panic!("Darwin blocking-service wake channel made zero progress"),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return,
            Err(error) => panic!("Darwin blocking-service wake channel failed: {error}"),
        }
    }
}

fn notification_channel() -> Result<(UnixStream, UnixStream), BuildError> {
    let (reader, writer) = UnixStream::pair().map_err(BuildError::Channel)?;
    reader.set_nonblocking(true).map_err(BuildError::Channel)?;
    writer.set_nonblocking(true).map_err(BuildError::Channel)?;
    Ok((reader, writer))
}

fn drain_channel(reader: &mut UnixStream) -> Result<usize, io::Error> {
    let mut total = 0;
    let mut buffer = [0_u8; 256];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(total),
            Ok(count) => total += count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(total),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

fn stop_workers<I, O>(
    jobs: &BlockingJobService<I, O>,
    worker_signal: &WorkerSignal,
    workers: &mut Vec<JoinHandle<()>>,
) {
    let _cleanup = jobs.begin_shutdown();
    worker_signal.close();
    let _ = join_workers(workers);
}

fn join_workers(workers: &mut Vec<JoinHandle<()>>) -> Result<(), ShutdownError> {
    let mut failed = false;
    for worker in workers.drain(..) {
        failed |= worker.join().is_err();
    }
    if failed {
        Err(ShutdownError::WorkerPanicked)
    } else {
        Ok(())
    }
}

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Debug)]
pub enum BuildError {
    Channel(io::Error),
    Worker(io::Error),
}

impl fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Darwin blocking service construction failed: {self:?}"
        )
    }
}

impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Channel(error) | Self::Worker(error) => Some(error),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownError {
    WorkerPanicked,
}

impl fmt::Display for ShutdownError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Darwin blocking service shutdown failed: {self:?}"
        )
    }
}

impl std::error::Error for ShutdownError {}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod retirement_tests;
