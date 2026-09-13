use std::io::Write;
use std::time::{Duration, Instant};

use nocter_blocking_runtime::{
    JobOutcome, JobStatus, RetirementCapacity, RetirementStatus, ServiceCapacity,
};
use tempfile::NamedTempFile;

use crate::{
    DarwinFileJob, DarwinFileOutcome, DarwinFileOwner, DarwinFileService, FileAccess,
    FileCancellation, FilePosition,
};

fn service() -> DarwinFileService {
    DarwinFileService::new(
        ServiceCapacity::new(2, 4).unwrap(),
        RetirementCapacity::new(1, 4).unwrap(),
    )
    .unwrap()
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
    assert_eq!(fact.transferred(), 6);
    assert!(fact.error().is_none());

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
