//! Typed host conformance for executor-safe Darwin file operations.

use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

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
pub use nocter_runtime_contract::{
    DarwinCanonicalPathAbi, DarwinFileAccess as FileAccess,
    DarwinFileFailure as FileOperationError, DarwinFileMetadataKind as FileMetadataKind,
    DarwinFileOperation as FileJobKind, DarwinFileSeekOrigin, DarwinFileWriteFact as FileWriteFact,
};

const DARWIN_OPEN_NOFOLLOW: i32 = 0x100;

/// Target-neutral metadata facts returned by the host conformance service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileMetadataFact {
    pub kind: FileMetadataKind,
    pub length: u64,
    pub modified_seconds: i64,
    pub modified_nanoseconds: u64,
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

impl FilePosition {
    /// Returns the closed target seek base without exposing host `SeekFrom` representation.
    #[must_use]
    pub const fn origin(self) -> DarwinFileSeekOrigin {
        match self {
            Self::Start(_) => DarwinFileSeekOrigin::Start,
            Self::End(_) => DarwinFileSeekOrigin::End,
            Self::Current(_) => DarwinFileSeekOrigin::Current,
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
    LockExclusive(DarwinFileOwner),
    Seek {
        owner: DarwinFileOwner,
        position: FilePosition,
    },
    Truncate {
        owner: DarwinFileOwner,
        length: u64,
    },
    ReadAt {
        owner: DarwinFileOwner,
        maximum: usize,
        offset: u64,
    },
    WriteAt {
        owner: DarwinFileOwner,
        bytes: Box<[u8]>,
        offset: u64,
    },
    Identity(DarwinFileOwner),
    RemoveFile(PathBuf),
    Rename {
        source: PathBuf,
        destination: PathBuf,
    },
    CreateDirectory(PathBuf),
    RemoveDirectory(PathBuf),
    Metadata(PathBuf),
    SymlinkMetadata(PathBuf),
    CreateSymlink {
        target: PathBuf,
        link: PathBuf,
    },
    ReadLink {
        path: PathBuf,
        maximum: usize,
    },
    Canonicalize {
        path: PathBuf,
        maximum: usize,
    },
}

/// One complete owned file-operation input.
pub struct DarwinFileJob {
    payload: FileJobPayload,
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
            payload: FileJobPayload::Open {
                permit,
                path,
                access,
            },
        }
    }

    #[must_use]
    pub fn read(owner: DarwinFileOwner, maximum: usize) -> Self {
        Self {
            payload: FileJobPayload::Read { owner, maximum },
        }
    }

    #[must_use]
    pub fn write(owner: DarwinFileOwner, bytes: impl Into<Box<[u8]>>) -> Self {
        Self {
            payload: FileJobPayload::Write {
                owner,
                bytes: bytes.into(),
            },
        }
    }

    #[must_use]
    pub fn flush(owner: DarwinFileOwner) -> Self {
        Self {
            payload: FileJobPayload::Flush(owner),
        }
    }

    #[must_use]
    pub fn lock_exclusive(owner: DarwinFileOwner) -> Self {
        Self {
            payload: FileJobPayload::LockExclusive(owner),
        }
    }

    #[must_use]
    pub fn seek(owner: DarwinFileOwner, position: FilePosition) -> Self {
        Self {
            payload: FileJobPayload::Seek { owner, position },
        }
    }

    #[must_use]
    pub fn truncate(owner: DarwinFileOwner, length: u64) -> Self {
        Self {
            payload: FileJobPayload::Truncate { owner, length },
        }
    }

    /// Creates one positioned read that does not change the shared file cursor.
    #[must_use]
    pub fn read_at(owner: DarwinFileOwner, maximum: usize, offset: u64) -> Self {
        Self {
            payload: FileJobPayload::ReadAt {
                owner,
                maximum,
                offset,
            },
        }
    }

    /// Creates one positioned complete-write attempt that does not change the shared file cursor.
    #[must_use]
    pub fn write_at(owner: DarwinFileOwner, bytes: impl Into<Box<[u8]>>, offset: u64) -> Self {
        Self {
            payload: FileJobPayload::WriteAt {
                owner,
                bytes: bytes.into(),
                offset,
            },
        }
    }

    #[must_use]
    pub fn remove_file(path: PathBuf) -> Self {
        Self {
            payload: FileJobPayload::RemoveFile(path),
        }
    }

    #[must_use]
    pub fn rename(source: PathBuf, destination: PathBuf) -> Self {
        Self {
            payload: FileJobPayload::Rename {
                source,
                destination,
            },
        }
    }

    #[must_use]
    pub fn create_directory(path: PathBuf) -> Self {
        Self {
            payload: FileJobPayload::CreateDirectory(path),
        }
    }

    #[must_use]
    pub fn remove_directory(path: PathBuf) -> Self {
        Self {
            payload: FileJobPayload::RemoveDirectory(path),
        }
    }

    #[must_use]
    pub fn metadata(path: PathBuf) -> Self {
        Self {
            payload: FileJobPayload::Metadata(path),
        }
    }

    #[must_use]
    pub fn symlink_metadata(path: PathBuf) -> Self {
        Self {
            payload: FileJobPayload::SymlinkMetadata(path),
        }
    }

    #[must_use]
    pub fn create_symlink(target: PathBuf, link: PathBuf) -> Self {
        Self {
            payload: FileJobPayload::CreateSymlink { target, link },
        }
    }

    #[must_use]
    pub fn read_link(path: PathBuf, maximum: usize) -> Self {
        Self {
            payload: FileJobPayload::ReadLink { path, maximum },
        }
    }

    #[must_use]
    pub fn identity(owner: DarwinFileOwner) -> Self {
        Self {
            payload: FileJobPayload::Identity(owner),
        }
    }

    #[must_use]
    pub fn canonicalize(path: PathBuf, maximum: usize) -> Self {
        Self {
            payload: FileJobPayload::Canonicalize { path, maximum },
        }
    }

    /// Returns the operation family projected from the owned payload variant.
    #[must_use]
    pub fn kind(&self) -> FileJobKind {
        match &self.payload {
            FileJobPayload::Open { .. } => FileJobKind::Open,
            FileJobPayload::Read { .. } => FileJobKind::Read,
            FileJobPayload::Write { .. } => FileJobKind::Write,
            FileJobPayload::Flush(_) => FileJobKind::Flush,
            FileJobPayload::LockExclusive(_) => FileJobKind::LockExclusive,
            FileJobPayload::Seek { .. } => FileJobKind::Seek,
            FileJobPayload::Truncate { .. } => FileJobKind::Truncate,
            FileJobPayload::ReadAt { .. } => FileJobKind::ReadAt,
            FileJobPayload::WriteAt { .. } => FileJobKind::WriteAt,
            FileJobPayload::Identity(_) => FileJobKind::Identity,
            FileJobPayload::RemoveFile(_) => FileJobKind::RemoveFile,
            FileJobPayload::Rename { .. } => FileJobKind::Rename,
            FileJobPayload::CreateDirectory(_) => FileJobKind::CreateDirectory,
            FileJobPayload::RemoveDirectory(_) => FileJobKind::RemoveDirectory,
            FileJobPayload::Metadata(_) => FileJobKind::Metadata,
            FileJobPayload::SymlinkMetadata(_) => FileJobKind::SymlinkMetadata,
            FileJobPayload::CreateSymlink { .. } => FileJobKind::CreateSymlink,
            FileJobPayload::ReadLink { .. } => FileJobKind::ReadLink,
            FileJobPayload::Canonicalize { .. } => FileJobKind::Canonicalize,
        }
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
    LockExclusive {
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
    ReadAt {
        owner: DarwinFileOwner,
        result: Result<Box<[u8]>, FileOperationError>,
    },
    WriteAt {
        owner: DarwinFileOwner,
        fact: FileWriteFact,
    },
    Identity {
        owner: DarwinFileOwner,
        result: Result<(u64, u64), FileOperationError>,
    },
    RemoveFile(Result<(), FileOperationError>),
    Rename(Result<(), FileOperationError>),
    CreateDirectory(Result<(), FileOperationError>),
    RemoveDirectory(Result<(), FileOperationError>),
    Metadata(Result<FileMetadataFact, FileOperationError>),
    SymlinkMetadata(Result<FileMetadataFact, FileOperationError>),
    CreateSymlink(Result<(), FileOperationError>),
    ReadLink(Result<Box<[u8]>, FileOperationError>),
    Canonicalize(Result<Box<[u8]>, FileOperationError>),
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
        let retirements = DarwinRetirementService::new(retirement_capacity, retire_file)?;
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

fn execute_job(job: DarwinFileJob) -> DarwinFileOutcome {
    match job.payload {
        FileJobPayload::Open {
            permit,
            path,
            access,
        } => DarwinFileOutcome::Open(
            open_file(&path, access)
                .map(|file| DarwinFileOwner::new(permit.attach(file)))
                .map_err(|error| file_failure(&error)),
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
                .sync_all()
                .map_err(|error| file_failure(&error));
            DarwinFileOutcome::Flush { owner, result }
        }
        FileJobPayload::LockExclusive(mut owner) => {
            let result = lock_file_exclusive(owner.0.resource_mut());
            DarwinFileOutcome::LockExclusive { owner, result }
        }
        FileJobPayload::Seek {
            mut owner,
            position,
        } => {
            let result = owner
                .0
                .resource_mut()
                .seek(position.into())
                .map_err(|error| file_failure(&error));
            DarwinFileOutcome::Seek { owner, result }
        }
        FileJobPayload::Truncate { mut owner, length } => {
            let result = owner
                .0
                .resource_mut()
                .set_len(length)
                .map_err(|error| file_failure(&error));
            DarwinFileOutcome::Truncate { owner, result }
        }
        FileJobPayload::ReadAt {
            mut owner,
            maximum,
            offset,
        } => {
            let result = read_file_at(owner.0.resource_mut(), maximum, offset);
            DarwinFileOutcome::ReadAt { owner, result }
        }
        FileJobPayload::WriteAt {
            mut owner,
            bytes,
            offset,
        } => {
            let fact = write_file_at(owner.0.resource_mut(), &bytes, offset);
            DarwinFileOutcome::WriteAt { owner, fact }
        }
        FileJobPayload::Identity(owner) => identity_outcome(owner),
        FileJobPayload::RemoveFile(path) => DarwinFileOutcome::RemoveFile(
            std::fs::remove_file(path).map_err(|error| file_failure(&error)),
        ),
        FileJobPayload::Rename {
            source,
            destination,
        } => DarwinFileOutcome::Rename(
            std::fs::rename(source, destination).map_err(|error| file_failure(&error)),
        ),
        FileJobPayload::CreateDirectory(path) => DarwinFileOutcome::CreateDirectory(
            std::fs::create_dir(path).map_err(|error| file_failure(&error)),
        ),
        FileJobPayload::RemoveDirectory(path) => DarwinFileOutcome::RemoveDirectory(
            std::fs::remove_dir(path).map_err(|error| file_failure(&error)),
        ),
        FileJobPayload::Metadata(path) => {
            DarwinFileOutcome::Metadata(metadata_fact(&path, MetadataQuery::FollowFinalLink))
        }
        FileJobPayload::SymlinkMetadata(path) => DarwinFileOutcome::SymlinkMetadata(metadata_fact(
            &path,
            MetadataQuery::InspectFinalLink,
        )),
        FileJobPayload::CreateSymlink { target, link } => DarwinFileOutcome::CreateSymlink(
            std::os::unix::fs::symlink(target, link).map_err(|error| file_failure(&error)),
        ),
        FileJobPayload::ReadLink { path, maximum } => {
            DarwinFileOutcome::ReadLink(read_link_path(path, maximum))
        }
        FileJobPayload::Canonicalize { path, maximum } => {
            DarwinFileOutcome::Canonicalize(canonical_path(path, maximum))
        }
    }
}

fn lock_file_exclusive(file: &mut File) -> Result<(), FileOperationError> {
    match file.try_lock() {
        Ok(()) => Ok(()),
        Err(TryLockError::Error(error)) => Err(file_failure(&error)),
        Err(TryLockError::WouldBlock) => Err(FileOperationError::LOCK_CONTENDED),
    }
}

/// Releases advisory lock state before descriptor destruction.
///
/// A descriptor duplicated by `fork` can outlive the retiring owner until the child calls
/// `exec`. Relying on `File::drop` alone would therefore let completed retirement remain
/// transiently locked by that duplicate. Explicit unlock makes retirement completion the exact
/// lock-release boundary while descriptor destruction remains the final resource cleanup.
fn retire_file(file: &mut File) {
    let _ = file.unlock();
}

fn identity_outcome(mut owner: DarwinFileOwner) -> DarwinFileOutcome {
    let result = owner
        .0
        .resource_mut()
        .metadata()
        .map(|metadata| (metadata.dev(), metadata.ino()))
        .map_err(|error| file_failure(&error));
    DarwinFileOutcome::Identity { owner, result }
}

fn read_link_path(path: PathBuf, maximum: usize) -> Result<Box<[u8]>, FileOperationError> {
    std::fs::read_link(path)
        .map_err(|error| file_failure(&error))
        .map(|target| {
            let mut bytes = std::os::unix::ffi::OsStringExt::into_vec(target.into_os_string());
            bytes.truncate(maximum);
            bytes.into_boxed_slice()
        })
}

fn canonical_path(path: PathBuf, maximum: usize) -> Result<Box<[u8]>, FileOperationError> {
    if maximum < DarwinCanonicalPathAbi::OUTPUT_SIZE {
        return Err(FileOperationError::INVALID_PROGRESS);
    }
    let canonical = std::fs::canonicalize(path).map_err(|error| file_failure(&error))?;
    let bytes = std::os::unix::ffi::OsStringExt::into_vec(canonical.into_os_string());
    if bytes.len() > maximum {
        Err(FileOperationError::INVALID_PROGRESS)
    } else {
        Ok(bytes.into_boxed_slice())
    }
}

#[derive(Clone, Copy)]
enum MetadataQuery {
    FollowFinalLink,
    InspectFinalLink,
}

fn metadata_fact(
    path: &PathBuf,
    query: MetadataQuery,
) -> Result<FileMetadataFact, FileOperationError> {
    let metadata = match query {
        MetadataQuery::FollowFinalLink => std::fs::metadata(path),
        MetadataQuery::InspectFinalLink => std::fs::symlink_metadata(path),
    }
    .map_err(|error| file_failure(&error))?;
    let file_type = metadata.file_type();
    let kind = if file_type.is_file() {
        FileMetadataKind::Regular
    } else if file_type.is_dir() {
        FileMetadataKind::Directory
    } else if file_type.is_symlink() {
        FileMetadataKind::SymbolicLink
    } else {
        FileMetadataKind::Other
    };
    let (modified_seconds, modified_nanoseconds) =
        unix_time_parts(metadata.modified().map_err(|error| file_failure(&error))?)?;
    Ok(FileMetadataFact {
        kind,
        length: metadata.len(),
        modified_seconds,
        modified_nanoseconds,
    })
}

fn unix_time_parts(time: std::time::SystemTime) -> Result<(i64, u64), FileOperationError> {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => Ok((
            i64::try_from(duration.as_secs()).map_err(|_| FileOperationError::UNCLASSIFIED)?,
            u64::from(duration.subsec_nanos()),
        )),
        Err(error) => {
            let duration = error.duration();
            let seconds =
                i64::try_from(duration.as_secs()).map_err(|_| FileOperationError::UNCLASSIFIED)?;
            let nanoseconds = u64::from(duration.subsec_nanos());
            if nanoseconds == 0 {
                Ok((-seconds, 0))
            } else {
                Ok((
                    seconds
                        .checked_add(1)
                        .and_then(i64::checked_neg)
                        .ok_or(FileOperationError::UNCLASSIFIED)?,
                    1_000_000_000 - nanoseconds,
                ))
            }
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
        FileAccess::CreateNew => OpenOptions::new().write(true).create_new(true).open(path),
        FileAccess::Append => OpenOptions::new().append(true).create(true).open(path),
        FileAccess::CopyDestination => OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path),
        FileAccess::Directory => {
            let directory = OpenOptions::new()
                .read(true)
                .custom_flags(DARWIN_OPEN_NOFOLLOW)
                .open(path)?;
            if directory.metadata()?.is_dir() {
                Ok(directory)
            } else {
                Err(io::Error::from(io::ErrorKind::NotADirectory))
            }
        }
    }
}

fn read_file(file: &mut File, maximum: usize) -> Result<Box<[u8]>, FileOperationError> {
    read_into_owned(maximum, |bytes| file.read(bytes))
}

fn read_file_at(file: &File, maximum: usize, offset: u64) -> Result<Box<[u8]>, FileOperationError> {
    read_into_owned(maximum, |bytes| file.read_at(bytes, offset))
}

fn read_into_owned(
    maximum: usize,
    read: impl FnOnce(&mut [u8]) -> io::Result<usize>,
) -> Result<Box<[u8]>, FileOperationError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(maximum)
        .map_err(|_| FileOperationError::ALLOCATION)?;
    bytes.resize(maximum, 0);
    let received = read(&mut bytes).map_err(|error| file_failure(&error))?;
    bytes.truncate(received);
    Ok(bytes.into_boxed_slice())
}

fn write_file(file: &mut File, bytes: &[u8]) -> FileWriteFact {
    write_complete(bytes, |remaining, _| {
        file.write(remaining)
            .map_err(|error| file_write_failure(&error))
    })
}

fn write_file_at(file: &File, bytes: &[u8], offset: u64) -> FileWriteFact {
    write_complete(bytes, |remaining, transferred| {
        let position =
            offset
                .checked_add(transferred as u64)
                .ok_or(FileWriteAttemptError::Failure(
                    FileOperationError::OFFSET_OVERFLOW,
                ))?;
        file.write_at(remaining, position)
            .map_err(|error| file_write_failure(&error))
    })
}

fn write_complete(
    bytes: &[u8],
    mut write: impl FnMut(&[u8], usize) -> Result<usize, FileWriteAttemptError>,
) -> FileWriteFact {
    let mut transferred = 0;
    while transferred < bytes.len() {
        match write(&bytes[transferred..], transferred) {
            Ok(0) => {
                return FileWriteFact::failed(
                    bytes.len(),
                    transferred,
                    FileOperationError::ZERO_PROGRESS,
                );
            }
            Ok(count) if count > bytes.len() - transferred => {
                return FileWriteFact::failed(
                    bytes.len(),
                    bytes.len(),
                    FileOperationError::INVALID_PROGRESS,
                );
            }
            Ok(count) => transferred += count,
            Err(FileWriteAttemptError::Interrupted) => {}
            Err(FileWriteAttemptError::Failure(error)) => {
                return FileWriteFact::failed(bytes.len(), transferred, error);
            }
        }
    }
    FileWriteFact::complete(transferred)
}

fn file_failure(error: &io::Error) -> FileOperationError {
    error
        .raw_os_error()
        .and_then(FileOperationError::target)
        .unwrap_or(FileOperationError::UNCLASSIFIED)
}

enum FileWriteAttemptError {
    Interrupted,
    Failure(FileOperationError),
}

fn file_write_failure(error: &io::Error) -> FileWriteAttemptError {
    if error.kind() == io::ErrorKind::Interrupted {
        FileWriteAttemptError::Interrupted
    } else {
        FileWriteAttemptError::Failure(file_failure(error))
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
