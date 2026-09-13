//! Typed host conformance for executor-safe Darwin file operations.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use nocter_blocking_runtime::{
    Cancellation, CapacityEpoch, JobId, JobOutcome, JobStatus, ResourceOwner, ResourcePermit,
    RetirementCapacity, RetirementEpoch, RetirementError, RetirementId, RetirementReserveError,
    RetirementSnapshot, RetirementStatus, ServiceCapacity, ServiceError, ServiceSnapshot,
    SubmitError,
};
use nocter_darwin_blocking_service::{
    BuildError, DarwinBlockingService, DarwinRetirementService, RetirementShutdownError,
    ShutdownError,
};

/// File access semantics selected before a target open job begins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileAccess {
    Read,
    Create,
    Append,
}

/// Portable positioning input retained by one owned seek job.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilePosition {
    Start(u64),
    End(i64),
    Current(i64),
}

impl From<FilePosition> for SeekFrom {
    fn from(position: FilePosition) -> Self {
        match position {
            FilePosition::Start(offset) => Self::Start(offset),
            FilePosition::End(offset) => Self::End(offset),
            FilePosition::Current(offset) => Self::Current(offset),
        }
    }
}

/// One file descriptor owner with an infallible pre-reserved retirement path.
pub struct DarwinFileOwner(ResourceOwner<File>);

impl DarwinFileOwner {
    fn new(owner: ResourceOwner<File>) -> Self {
        Self(owner)
    }

    /// Starts exact asynchronous close observation after transferring the file to cleanup.
    ///
    /// # Errors
    ///
    /// Reports permanent retirement-identity exhaustion after cleanup has still been enqueued.
    pub fn retire(self) -> Result<RetirementId, RetirementError> {
        self.0.retire()
    }
}

/// Exact operation family retained beside a job identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileJobKind {
    Open,
    Read,
    Write,
    Flush,
    Seek,
    Truncate,
}

enum FileJobPayload {
    Open {
        permit: ResourcePermit<File>,
        path: PathBuf,
        access: FileAccess,
    },
    Read {
        owner: DarwinFileOwner,
        maximum: usize,
    },
    Write {
        owner: DarwinFileOwner,
        bytes: Box<[u8]>,
    },
    Flush(DarwinFileOwner),
    Seek {
        owner: DarwinFileOwner,
        position: FilePosition,
    },
    Truncate {
        owner: DarwinFileOwner,
        length: u64,
    },
}

/// One complete owned file-operation input.
pub struct DarwinFileJob {
    payload: Option<FileJobPayload>,
}

impl fmt::Debug for DarwinFileJob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DarwinFileJob")
            .field("kind", &self.kind())
            .finish_non_exhaustive()
    }
}

impl DarwinFileJob {
    #[must_use]
    pub fn open(permit: ResourcePermit<File>, path: PathBuf, access: FileAccess) -> Self {
        Self {
            payload: Some(FileJobPayload::Open {
                permit,
                path,
                access,
            }),
        }
    }

    #[must_use]
    pub fn read(owner: DarwinFileOwner, maximum: usize) -> Self {
        Self {
            payload: Some(FileJobPayload::Read { owner, maximum }),
        }
    }

    #[must_use]
    pub fn write(owner: DarwinFileOwner, bytes: impl Into<Box<[u8]>>) -> Self {
        Self {
            payload: Some(FileJobPayload::Write {
                owner,
                bytes: bytes.into(),
            }),
        }
    }

    #[must_use]
    pub fn flush(owner: DarwinFileOwner) -> Self {
        Self {
            payload: Some(FileJobPayload::Flush(owner)),
        }
    }

    #[must_use]
    pub fn seek(owner: DarwinFileOwner, position: FilePosition) -> Self {
        Self {
            payload: Some(FileJobPayload::Seek { owner, position }),
        }
    }

    #[must_use]
    pub fn truncate(owner: DarwinFileOwner, length: u64) -> Self {
        Self {
            payload: Some(FileJobPayload::Truncate { owner, length }),
        }
    }

    /// Returns the operation family projected from the owned payload variant.
    ///
    /// # Panics
    ///
    /// Panics only after an internal worker invokes this method on the already consumed payload;
    /// submitted jobs are never returned to public callers.
    #[must_use]
    pub fn kind(&self) -> FileJobKind {
        match self
            .payload
            .as_ref()
            .expect("an admitted file job is not returned to its submitter")
        {
            FileJobPayload::Open { .. } => FileJobKind::Open,
            FileJobPayload::Read { .. } => FileJobKind::Read,
            FileJobPayload::Write { .. } => FileJobKind::Write,
            FileJobPayload::Flush(_) => FileJobKind::Flush,
            FileJobPayload::Seek { .. } => FileJobKind::Seek,
            FileJobPayload::Truncate { .. } => FileJobKind::Truncate,
        }
    }
}

/// Allocation or host-I/O failure produced on a blocking worker.
#[derive(Debug)]
pub enum FileOperationError {
    Allocation,
    Io(io::Error),
}

impl fmt::Display for FileOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Darwin file operation failed: {self:?}")
    }
}

impl std::error::Error for FileOperationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Allocation => None,
            Self::Io(error) => Some(error),
        }
    }
}

impl From<io::Error> for FileOperationError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Exact write progress retained even when the terminal operation result is an error.
#[derive(Debug)]
pub struct FileWriteFact {
    transferred: usize,
    error: Option<io::Error>,
}

impl FileWriteFact {
    #[must_use]
    pub const fn transferred(&self) -> usize {
        self.transferred
    }

    #[must_use]
    pub const fn error(&self) -> Option<&io::Error> {
        self.error.as_ref()
    }
}

/// Typed worker output. Every non-open operation returns the same file owner.
pub enum DarwinFileOutcome {
    Open(Result<DarwinFileOwner, FileOperationError>),
    Read {
        owner: DarwinFileOwner,
        result: Result<Box<[u8]>, FileOperationError>,
    },
    Write {
        owner: DarwinFileOwner,
        fact: FileWriteFact,
    },
    Flush {
        owner: DarwinFileOwner,
        result: Result<(), FileOperationError>,
    },
    Seek {
        owner: DarwinFileOwner,
        result: Result<u64, FileOperationError>,
    },
    Truncate {
        owner: DarwinFileOwner,
        result: Result<(), FileOperationError>,
    },
}

/// Cancellation result without exposing generic queue payloads to file policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileCancellation {
    Queued,
    Running,
    Completed,
    WorkerLost,
}

/// Host conformance composition of ordinary file jobs and guaranteed retirement.
pub struct DarwinFileService {
    jobs: DarwinBlockingService<DarwinFileJob, DarwinFileOutcome>,
    retirements: DarwinRetirementService<File>,
}

impl DarwinFileService {
    /// Creates both bounded worker domains as one file service.
    ///
    /// # Errors
    ///
    /// Returns target adapter construction failure before publishing a partial service.
    pub fn new(
        job_capacity: ServiceCapacity,
        retirement_capacity: RetirementCapacity,
    ) -> Result<Self, BuildError> {
        let retirements = DarwinRetirementService::new(retirement_capacity, |_: &mut File| {})?;
        let jobs = DarwinBlockingService::new(job_capacity, execute_job)?;
        Ok(Self { jobs, retirements })
    }

    /// Reserves cleanup capacity before an open job can create a file owner.
    ///
    /// # Errors
    ///
    /// Reports retirement admission failure without creating an operating-system resource.
    pub fn reserve_file(&self) -> Result<ResourcePermit<File>, RetirementReserveError> {
        self.retirements.reserve()
    }

    /// Transfers one complete operation into bounded worker ownership.
    ///
    /// # Errors
    ///
    /// Returns the exact job on closed, saturated, or exhausted admission.
    pub fn submit(&self, job: DarwinFileJob) -> Result<JobId, SubmitError<DarwinFileJob>> {
        self.jobs.submit(job)
    }

    #[must_use]
    pub fn job_notification_descriptor(&self) -> std::os::fd::RawFd {
        self.jobs.notification_descriptor()
    }

    #[must_use]
    pub fn retirement_notification_descriptor(&self) -> std::os::fd::RawFd {
        self.retirements.notification_descriptor()
    }

    #[must_use]
    pub fn status(&self, job: JobId) -> Option<JobStatus> {
        self.jobs.status(job)
    }

    #[must_use]
    pub fn job_snapshot(&self) -> ServiceSnapshot {
        self.jobs.snapshot()
    }

    /// Reports whether ordinary job capacity changed after the supplied observation.
    ///
    /// # Errors
    ///
    /// Rejects an observation issued by another job service.
    pub fn capacity_changed_since(&self, observed: CapacityEpoch) -> Result<bool, ServiceError> {
        self.jobs.capacity_changed_since(observed)
    }

    /// Consumes one exact file-operation outcome.
    ///
    /// # Errors
    ///
    /// Rejects an unknown, detached, or incomplete job.
    pub fn consume(&self, job: JobId) -> Result<JobOutcome<DarwinFileOutcome>, ServiceError> {
        self.jobs.consume(job)
    }

    /// Detaches one operation waiter and immediately destroys any returned input or output.
    ///
    /// # Errors
    ///
    /// Rejects an unknown or already detached job.
    pub fn cancel(&self, job: JobId) -> Result<FileCancellation, ServiceError> {
        Ok(match self.jobs.cancel(job)? {
            Cancellation::Queued(input) => {
                drop(input);
                FileCancellation::Queued
            }
            Cancellation::Running => FileCancellation::Running,
            Cancellation::Completed(JobOutcome::Completed(output)) => {
                drop(output);
                FileCancellation::Completed
            }
            Cancellation::Completed(JobOutcome::WorkerLost) => FileCancellation::WorkerLost,
        })
    }

    #[must_use]
    pub fn retirement_status(&self, retirement: RetirementId) -> Option<RetirementStatus> {
        self.retirements.status(retirement)
    }

    #[must_use]
    pub fn retirement_snapshot(&self) -> RetirementSnapshot {
        self.retirements.snapshot()
    }

    /// Reports whether file-retirement capacity changed after the supplied observation.
    ///
    /// # Errors
    ///
    /// Rejects an observation issued by another retirement service.
    pub fn retirement_capacity_changed_since(
        &self,
        observed: RetirementEpoch,
    ) -> Result<bool, RetirementError> {
        self.retirements.capacity_changed_since(observed)
    }

    /// Consumes one exact completed file retirement.
    ///
    /// # Errors
    ///
    /// Rejects an unknown, detached, or incomplete retirement.
    pub fn consume_retirement(&self, retirement: RetirementId) -> Result<(), RetirementError> {
        self.retirements.consume(retirement)
    }

    /// Detaches one explicit close waiter without cancelling file cleanup.
    ///
    /// # Errors
    ///
    /// Rejects an unknown or already detached retirement.
    pub fn detach_retirement(&self, retirement: RetirementId) -> Result<(), RetirementError> {
        self.retirements.detach(retirement)
    }

    /// Drains both independent wake-only channels.
    ///
    /// # Errors
    ///
    /// Returns the first channel read failure other than nonblocking exhaustion.
    pub fn drain_notifications(&mut self) -> Result<(usize, usize), io::Error> {
        Ok((
            self.jobs.drain_notifications()?,
            self.retirements.drain_notifications()?,
        ))
    }

    /// Stops operation admission before requiring complete resource retirement.
    ///
    /// # Errors
    ///
    /// Reports operation-worker failure or outstanding file owners and close waiters.
    pub fn shutdown(&mut self) -> Result<(), FileServiceShutdownError> {
        drop(
            self.jobs
                .shutdown()
                .map_err(FileServiceShutdownError::Jobs)?,
        );
        self.retirements
            .shutdown()
            .map_err(FileServiceShutdownError::Retirement)
    }
}

fn execute_job(job: &mut DarwinFileJob) -> DarwinFileOutcome {
    let payload = job
        .payload
        .take()
        .expect("one file job is executed at most once");
    match payload {
        FileJobPayload::Open {
            permit,
            path,
            access,
        } => DarwinFileOutcome::Open(
            open_file(&path, access)
                .map(|file| DarwinFileOwner::new(permit.attach(file)))
                .map_err(FileOperationError::from),
        ),
        FileJobPayload::Read { mut owner, maximum } => {
            let result = read_file(owner.0.resource_mut(), maximum);
            DarwinFileOutcome::Read { owner, result }
        }
        FileJobPayload::Write { mut owner, bytes } => {
            let fact = write_file(owner.0.resource_mut(), &bytes);
            DarwinFileOutcome::Write { owner, fact }
        }
        FileJobPayload::Flush(mut owner) => {
            let result = owner
                .0
                .resource_mut()
                .flush()
                .map_err(FileOperationError::from);
            DarwinFileOutcome::Flush { owner, result }
        }
        FileJobPayload::Seek {
            mut owner,
            position,
        } => {
            let result = owner
                .0
                .resource_mut()
                .seek(position.into())
                .map_err(FileOperationError::from);
            DarwinFileOutcome::Seek { owner, result }
        }
        FileJobPayload::Truncate { mut owner, length } => {
            let result = owner
                .0
                .resource_mut()
                .set_len(length)
                .map_err(FileOperationError::from);
            DarwinFileOutcome::Truncate { owner, result }
        }
    }
}

fn open_file(path: &PathBuf, access: FileAccess) -> io::Result<File> {
    match access {
        FileAccess::Read => File::open(path),
        FileAccess::Create => OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path),
        FileAccess::Append => OpenOptions::new().append(true).create(true).open(path),
    }
}

fn read_file(file: &mut File, maximum: usize) -> Result<Box<[u8]>, FileOperationError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(maximum)
        .map_err(|_| FileOperationError::Allocation)?;
    bytes.resize(maximum, 0);
    let received = file.read(&mut bytes).map_err(FileOperationError::from)?;
    bytes.truncate(received);
    Ok(bytes.into_boxed_slice())
}

fn write_file(file: &mut File, bytes: &[u8]) -> FileWriteFact {
    let mut transferred = 0;
    while transferred < bytes.len() {
        match file.write(&bytes[transferred..]) {
            Ok(0) => {
                return FileWriteFact {
                    transferred,
                    error: Some(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "write made no progress",
                    )),
                };
            }
            Ok(count) => transferred += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                return FileWriteFact {
                    transferred,
                    error: Some(error),
                };
            }
        }
    }
    FileWriteFact {
        transferred,
        error: None,
    }
}

#[derive(Debug)]
pub enum FileServiceShutdownError {
    Jobs(ShutdownError),
    Retirement(RetirementShutdownError),
}

impl fmt::Display for FileServiceShutdownError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Darwin file service shutdown failed: {self:?}")
    }
}

impl std::error::Error for FileServiceShutdownError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Jobs(error) => Some(error),
            Self::Retirement(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests;
