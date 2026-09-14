use std::io::Write;
use std::time::{Duration, Instant};

use nocter_blocking_runtime::{
    JobOutcome, JobStatus, RetirementCapacity, RetirementStatus, ServiceCapacity,
};
use nocter_runtime_contract::DarwinFileServiceConfiguration;
use tempfile::NamedTempFile;

use crate::{
    DarwinFileJob, DarwinFileOutcome, DarwinFileOwner, DarwinFileService, FileAccess,
    FileCancellation, FileJobKind, FilePosition,
};

fn service() -> DarwinFileService {
    DarwinFileService::new(
        ServiceCapacity::new(2, 4).unwrap(),
        RetirementCapacity::new(1, 4).unwrap(),
    )
    .unwrap()
}

#[test]
fn generated_capacity_is_accepted_by_both_lifecycle_authorities() {
    let capacity = DarwinFileServiceConfiguration::ARM64_DARWIN;
    assert!(
        ServiceCapacity::new(capacity.operation_workers(), capacity.maximum_operations(),).is_ok()
    );
    assert!(
        RetirementCapacity::new(
            capacity.retirement_workers(),
            capacity.maximum_retirements(),
        )
        .is_ok()
    );
}

fn wait_for_job(service: &DarwinFileService, job: nocter_blocking_runtime::JobId) {
    wait_until(|| service.status(job) == Some(JobStatus::Completed));
}

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !condition() {
        assert!(Instant::now() < deadline, "file service did not finish");
        std::thread::yield_now();
    }
}

fn open_for_read(service: &DarwinFileService, path: &std::path::Path) -> DarwinFileOwner {
    let job = service
        .submit(DarwinFileJob::open(
            service.reserve_file().unwrap(),
            path.to_path_buf(),
            FileAccess::Read,
        ))
        .unwrap();
    wait_for_job(service, job);
    let JobOutcome::Completed(DarwinFileOutcome::Open(result)) = service.consume(job).unwrap()
    else {
        panic!("open job returned the wrong outcome")
    };
    result.unwrap()
}

#[test]
fn read_seek_and_explicit_close_preserve_one_owned_file() {
    let mut temporary = NamedTempFile::new().unwrap();
    temporary.write_all(b"hello").unwrap();
    temporary.flush().unwrap();
    let mut service = service();
    let owner = open_for_read(&service, temporary.path());

    let read = service.submit(DarwinFileJob::read(owner, 3)).unwrap();
    wait_for_job(&service, read);
    let JobOutcome::Completed(DarwinFileOutcome::Read { owner, result }) =
        service.consume(read).unwrap()
    else {
        panic!("read job returned the wrong outcome")
    };
    assert_eq!(&*result.unwrap(), b"hel");

    let seek = service
        .submit(DarwinFileJob::seek(owner, FilePosition::Start(0)))
        .unwrap();
    wait_for_job(&service, seek);
    let JobOutcome::Completed(DarwinFileOutcome::Seek { owner, result }) =
        service.consume(seek).unwrap()
    else {
        panic!("seek job returned the wrong outcome")
    };
    assert_eq!(result.unwrap(), 0);

    let read = service.submit(DarwinFileJob::read(owner, 16)).unwrap();
    wait_for_job(&service, read);
    let JobOutcome::Completed(DarwinFileOutcome::Read { owner, result }) =
        service.consume(read).unwrap()
    else {
        panic!("second read job returned the wrong outcome")
    };
    assert_eq!(&*result.unwrap(), b"hello");

    let retirement = owner.retire().unwrap();
    wait_until(|| service.retirement_status(retirement) == Some(RetirementStatus::Completed));
    service.consume_retirement(retirement).unwrap();
    service.shutdown().unwrap();
}

#[test]
fn write_flush_and_truncate_publish_complete_operation_facts() {
    let temporary = NamedTempFile::new().unwrap();
    let path = temporary.path().to_path_buf();
    drop(temporary);
    let mut service = service();
    let open = service
        .submit(DarwinFileJob::open(
            service.reserve_file().unwrap(),
            path.clone(),
            FileAccess::Create,
        ))
        .unwrap();
    wait_for_job(&service, open);
    let JobOutcome::Completed(DarwinFileOutcome::Open(result)) = service.consume(open).unwrap()
    else {
        panic!("create job returned the wrong outcome")
    };

    let write = service
        .submit(DarwinFileJob::write(result.unwrap(), &b"abcdef"[..]))
        .unwrap();
    wait_for_job(&service, write);
    let JobOutcome::Completed(DarwinFileOutcome::Write { owner, fact }) =
        service.consume(write).unwrap()
    else {
        panic!("write job returned the wrong outcome")
    };
    assert_eq!(fact.attempted(), 6);
    assert_eq!(fact.transferred(), 6);
    assert!(fact.failure().is_none());

    let truncate = service.submit(DarwinFileJob::truncate(owner, 3)).unwrap();
    wait_for_job(&service, truncate);
    let JobOutcome::Completed(DarwinFileOutcome::Truncate { owner, result }) =
        service.consume(truncate).unwrap()
    else {
        panic!("truncate job returned the wrong outcome")
    };
    result.unwrap();

    let flush = service.submit(DarwinFileJob::flush(owner)).unwrap();
    wait_for_job(&service, flush);
    let JobOutcome::Completed(DarwinFileOutcome::Flush { owner, result }) =
        service.consume(flush).unwrap()
    else {
        panic!("flush job returned the wrong outcome")
    };
    result.unwrap();
    drop(owner);
    wait_until(|| service.retirement_snapshot().drained());
    service.shutdown().unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"abc");
}

#[test]
fn cancelling_completed_open_destroys_its_unpublished_owner_on_retirement_worker() {
    let temporary = NamedTempFile::new().unwrap();
    let mut service = service();
    let open = service
        .submit(DarwinFileJob::open(
            service.reserve_file().unwrap(),
            temporary.path().to_path_buf(),
            FileAccess::Read,
        ))
        .unwrap();
    wait_for_job(&service, open);
    assert_eq!(service.cancel(open).unwrap(), FileCancellation::Completed);
    wait_until(|| service.retirement_snapshot().drained());
    service.shutdown().unwrap();
}

#[test]
fn positioned_transfers_preserve_cursor_and_exact_progress() {
    let mut temporary = NamedTempFile::new().unwrap();
    temporary.write_all(b"abcdef").unwrap();
    temporary.flush().unwrap();
    let path = temporary.path().to_path_buf();
    {
        let mut service = service();
        let owner = open_for_read(&service, &path);

        let seek = DarwinFileJob::seek(owner, FilePosition::Start(4));
        assert_eq!(seek.kind(), FileJobKind::Seek);
        let seek = service.submit(seek).unwrap();
        wait_for_job(&service, seek);
        let JobOutcome::Completed(DarwinFileOutcome::Seek { owner, result }) =
            service.consume(seek).unwrap()
        else {
            panic!("seek job returned the wrong outcome")
        };
        assert_eq!(result.unwrap(), 4);

        let positioned = DarwinFileJob::read_at(owner, 3, 1);
        assert_eq!(positioned.kind(), FileJobKind::ReadAt);
        let positioned = service.submit(positioned).unwrap();
        wait_for_job(&service, positioned);
        let JobOutcome::Completed(DarwinFileOutcome::ReadAt { owner, result }) =
            service.consume(positioned).unwrap()
        else {
            panic!("positioned read returned the wrong outcome")
        };
        assert_eq!(&*result.unwrap(), b"bcd");

        let sequential = service.submit(DarwinFileJob::read(owner, 2)).unwrap();
        wait_for_job(&service, sequential);
        let JobOutcome::Completed(DarwinFileOutcome::Read { owner, result }) =
            service.consume(sequential).unwrap()
        else {
            panic!("sequential read returned the wrong outcome")
        };
        assert_eq!(&*result.unwrap(), b"ef");
        drop(owner);
        wait_until(|| service.retirement_snapshot().drained());
        service.shutdown().unwrap();
    }

    let mut service = service();
    let open = service
        .submit(DarwinFileJob::open(
            service.reserve_file().unwrap(),
            path.clone(),
            FileAccess::Create,
        ))
        .unwrap();
    wait_for_job(&service, open);
    let JobOutcome::Completed(DarwinFileOutcome::Open(result)) = service.consume(open).unwrap()
    else {
        panic!("create open returned the wrong outcome")
    };
    let initial = service
        .submit(DarwinFileJob::write(result.unwrap(), &b"abcdef"[..]))
        .unwrap();
    wait_for_job(&service, initial);
    let JobOutcome::Completed(DarwinFileOutcome::Write { owner, fact }) =
        service.consume(initial).unwrap()
    else {
        panic!("initial write returned the wrong outcome")
    };
    assert_eq!(fact.attempted(), 6);
    assert_eq!(fact.transferred(), 6);
    assert!(fact.failure().is_none());

    let positioned = DarwinFileJob::write_at(owner, &b"XY"[..], 1);
    assert_eq!(positioned.kind(), FileJobKind::WriteAt);
    let positioned = service.submit(positioned).unwrap();
    wait_for_job(&service, positioned);
    let JobOutcome::Completed(DarwinFileOutcome::WriteAt { owner, fact }) =
        service.consume(positioned).unwrap()
    else {
        panic!("positioned write returned the wrong outcome")
    };
    assert_eq!(fact.attempted(), 2);
    assert_eq!(fact.transferred(), 2);
    assert!(fact.failure().is_none());

    let sequential = service
        .submit(DarwinFileJob::write(owner, &b"Z"[..]))
        .unwrap();
    wait_for_job(&service, sequential);
    let JobOutcome::Completed(DarwinFileOutcome::Write { owner, fact }) =
        service.consume(sequential).unwrap()
    else {
        panic!("sequential write returned the wrong outcome")
    };
    assert_eq!(fact.attempted(), 1);
    assert_eq!(fact.transferred(), 1);
    assert!(fact.failure().is_none());
    drop(owner);
    wait_until(|| service.retirement_snapshot().drained());
    service.shutdown().unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"aXYdefZ");
}

#[test]
fn path_mutations_share_bounded_job_admission_without_resource_owners() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("created");
    let source = directory.join("source.txt");
    let destination = directory.join("destination.txt");
    let mut service = service();

    let create = DarwinFileJob::create_directory(directory.clone());
    assert_eq!(create.kind(), FileJobKind::CreateDirectory);
    let create = service.submit(create).unwrap();
    wait_for_job(&service, create);
    let JobOutcome::Completed(DarwinFileOutcome::CreateDirectory(result)) =
        service.consume(create).unwrap()
    else {
        panic!("create-directory job returned the wrong outcome")
    };
    result.unwrap();

    std::fs::write(&source, b"owned input").unwrap();
    let rename = DarwinFileJob::rename(source.clone(), destination.clone());
    assert_eq!(rename.kind(), FileJobKind::Rename);
    let rename = service.submit(rename).unwrap();
    wait_for_job(&service, rename);
    let JobOutcome::Completed(DarwinFileOutcome::Rename(result)) = service.consume(rename).unwrap()
    else {
        panic!("rename job returned the wrong outcome")
    };
    result.unwrap();
    assert_eq!(std::fs::read(&destination).unwrap(), b"owned input");

    let remove_file = DarwinFileJob::remove_file(destination);
    assert_eq!(remove_file.kind(), FileJobKind::RemoveFile);
    let remove_file = service.submit(remove_file).unwrap();
    wait_for_job(&service, remove_file);
    let JobOutcome::Completed(DarwinFileOutcome::RemoveFile(result)) =
        service.consume(remove_file).unwrap()
    else {
        panic!("remove-file job returned the wrong outcome")
    };
    result.unwrap();

    let remove_directory = DarwinFileJob::remove_directory(directory.clone());
    assert_eq!(remove_directory.kind(), FileJobKind::RemoveDirectory);
    let remove_directory = service.submit(remove_directory).unwrap();
    wait_for_job(&service, remove_directory);
    let JobOutcome::Completed(DarwinFileOutcome::RemoveDirectory(result)) =
        service.consume(remove_directory).unwrap()
    else {
        panic!("remove-directory job returned the wrong outcome")
    };
    result.unwrap();
    assert!(!directory.exists());

    service.shutdown().unwrap();
}
