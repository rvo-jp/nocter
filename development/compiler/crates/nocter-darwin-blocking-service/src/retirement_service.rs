use std::fmt;
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use nocter_blocking_runtime::{
    ResourcePermit, RetirementCapacity, RetirementEpoch, RetirementError, RetirementId,
    RetirementReserveError, RetirementService, RetirementSnapshot, RetirementStatus,
};

use crate::{BuildError, WorkerSignal, drain_channel, join_workers, notification_channel, signal};

/// Fixed Darwin cleanup workers over one opaque retirement-aware resource type.
pub struct DarwinRetirementService<R> {
    resources: RetirementService<R>,
    workers: Vec<JoinHandle<()>>,
    worker_signal: Arc<WorkerSignal>,
    notification_reader: UnixStream,
    stopped: bool,
}

impl<R: Send + 'static> DarwinRetirementService<R> {
    /// Creates every cleanup worker and the nonblocking capacity-notification channel.
    ///
    /// # Errors
    ///
    /// Returns channel or thread construction failure after joining every partially created
    /// worker.
    pub fn new<F>(capacity: RetirementCapacity, cleanup: F) -> Result<Self, BuildError>
    where
        F: Fn(&mut R) + Send + Sync + 'static,
    {
        let (notification_reader, notification_writer) = notification_channel()?;
        // The lifecycle may outlive this adapter while an already issued owner is still live. Its
        // notifier therefore retains one read endpoint so late retirement never writes to a pipe
        // with no reader. It does not consume bytes or create a second observation authority.
        let notification_reader_guard = notification_reader
            .try_clone()
            .map_err(BuildError::Channel)?;
        let worker_signal = Arc::new(WorkerSignal::new());
        let notify_workers = Arc::clone(&worker_signal);
        let resources = RetirementService::with_notifier(capacity, move || {
            notify_workers.notify_all();
            signal(&notification_writer);
            let _keep_reader_alive = &notification_reader_guard;
        });
        let cleanup = Arc::new(cleanup);
        let mut workers = Vec::with_capacity(capacity.workers());
        for index in 0..capacity.workers() {
            let worker_resources = resources.clone();
            let worker_control = Arc::clone(&worker_signal);
            let cleanup = Arc::clone(&cleanup);
            match thread::Builder::new()
                .name(format!("nocter-retirement-{index}"))
                .spawn(move || retirement_loop(&worker_resources, &worker_control, &*cleanup))
            {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    resources.close_admission();
                    worker_signal.close();
                    let _ = join_workers(&mut workers);
                    return Err(BuildError::Worker(error));
                }
            }
        }
        Ok(Self {
            resources,
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
    pub fn snapshot(&self) -> RetirementSnapshot {
        self.resources.snapshot()
    }

    /// Reserves infallible cleanup admission before a native resource is created.
    ///
    /// # Errors
    ///
    /// Reports closed admission, bounded saturation, or permanent observation-identity exhaustion.
    pub fn reserve(&self) -> Result<ResourcePermit<R>, RetirementReserveError> {
        self.resources.reserve()
    }

    /// Reports whether this service released retirement capacity after the supplied observation.
    ///
    /// # Errors
    ///
    /// Rejects an observation issued by another retirement service.
    pub fn capacity_changed_since(
        &self,
        observed: RetirementEpoch,
    ) -> Result<bool, RetirementError> {
        self.resources.capacity_changed_since(observed)
    }

    #[must_use]
    pub fn status(&self, retirement: RetirementId) -> Option<RetirementStatus> {
        self.resources.status(retirement)
    }

    /// Consumes one exact completed retirement.
    ///
    /// # Errors
    ///
    /// Rejects an unknown, detached, or incomplete retirement.
    pub fn consume(&self, retirement: RetirementId) -> Result<(), RetirementError> {
        self.resources.consume(retirement)
    }

    /// Detaches one waiter without cancelling cleanup.
    ///
    /// # Errors
    ///
    /// Rejects an unknown or already detached retirement.
    pub fn detach(&self, retirement: RetirementId) -> Result<(), RetirementError> {
        self.resources.detach(retirement)
    }

    /// Drains every currently readable wake byte.
    ///
    /// # Errors
    ///
    /// Returns a channel read failure other than ordinary nonblocking exhaustion.
    pub fn drain_notifications(&mut self) -> Result<usize, io::Error> {
        drain_channel(&mut self.notification_reader)
    }

    /// Closes new permits and joins workers after every issued permit or owner has entered cleanup.
    ///
    /// # Errors
    ///
    /// Reports externally held permits or owners without stopping workers, or a worker panic after
    /// all externally owned resources have been surrendered.
    pub fn shutdown(&mut self) -> Result<(), RetirementShutdownError> {
        self.resources.close_admission();
        self.resources.detach_all();
        let snapshot = self.resources.snapshot();
        let external = snapshot
            .reserved()
            .saturating_sub(snapshot.queued() + snapshot.running());
        if external != 0 {
            return Err(RetirementShutdownError::OutstandingOwners(external));
        }
        join_workers(&mut self.workers).map_err(|_| RetirementShutdownError::WorkerPanicked)?;
        self.worker_signal.close();
        self.stopped = true;
        Ok(())
    }
}

impl<R> Drop for DarwinRetirementService<R> {
    fn drop(&mut self) {
        if self.stopped {
            return;
        }
        self.resources.close_admission();
        self.resources.detach_all();
        let snapshot = self.resources.snapshot();
        let external = snapshot
            .reserved()
            .saturating_sub(snapshot.queued() + snapshot.running());
        if external == 0 {
            let _ = join_workers(&mut self.workers);
            self.worker_signal.close();
        } else {
            // Existing owners retain the lifecycle notifier and worker control. Detached join
            // handles finish after the final owner enters the pre-reserved queue and all cleanup
            // workers observe the closed, drained state.
            self.workers.clear();
        }
        self.stopped = true;
    }
}

fn retirement_loop<R, F>(
    resources: &RetirementService<R>,
    worker_signal: &WorkerSignal,
    cleanup: &F,
) where
    F: Fn(&mut R),
{
    loop {
        let observed = worker_signal.revision();
        if let Some(mut resource) = resources.claim() {
            let _ = catch_unwind(AssertUnwindSafe(|| cleanup(resource.resource_mut())));
            resource.finish();
            continue;
        }
        let snapshot = resources.snapshot();
        if !snapshot.accepting() && snapshot.drained() {
            return;
        }
        if worker_signal.wait_for_change(observed) {
            return;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetirementShutdownError {
    OutstandingOwners(usize),
    WorkerPanicked,
}

impl fmt::Display for RetirementShutdownError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Darwin retirement service shutdown failed: {self:?}"
        )
    }
}

impl std::error::Error for RetirementShutdownError {}
