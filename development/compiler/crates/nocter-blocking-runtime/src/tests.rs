use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use crate::{
    BlockingJobService, Cancellation, JobOutcome, JobStatus, ServiceCapacity, ServiceError,
    SubmitError,
};

fn service(workers: usize, maximum_jobs: usize) -> BlockingJobService<&'static str, usize> {
    BlockingJobService::new(ServiceCapacity::new(workers, maximum_jobs).unwrap())
}

#[test]
fn capacity_rejects_impossible_bounds() {
    assert_eq!(ServiceCapacity::new(0, 1), Err(ServiceError::ZeroWorkers));
    assert_eq!(
        ServiceCapacity::new(2, 1),
        Err(ServiceError::JobCapacityBelowWorkers {
            workers: 2,
            maximum_jobs: 1,
        })
    );
}

#[test]
fn saturation_returns_exact_input_and_observed_epoch() {
    let service = service(1, 1);
    let first = service.submit("first").unwrap();
    let SubmitError::Saturated(backpressure) = service.submit("second").unwrap_err() else {
        panic!("expected bounded saturation")
    };
    let observed = backpressure.observed_epoch();
    assert_eq!(
        backpressure.into_parts(),
        ("second", service.snapshot().capacity_epoch())
    );
    assert_eq!(
        service.cancel(first).unwrap(),
        Cancellation::Queued("first")
    );
    assert!(service.capacity_changed_since(observed).unwrap());
}

#[test]
fn identities_and_capacity_epochs_cannot_cross_services() {
    let first = service(1, 1);
    let second = service(1, 1);
    let first_job = first.submit("first").unwrap();
    let second_job = second.submit("second").unwrap();
    assert_eq!(first_job.get(), second_job.get());
    assert_ne!(first_job, second_job);
    assert_eq!(second.status(first_job), None);
    assert_eq!(
        second.cancel(first_job),
        Err(ServiceError::UnknownJob(first_job))
    );

    let SubmitError::Saturated(backpressure) = first.submit("observer").unwrap_err() else {
        panic!("expected bounded saturation")
    };
    assert!(matches!(
        second.capacity_changed_since(backpressure.observed_epoch()),
        Err(ServiceError::ForeignCapacityEpoch(_))
    ));
}

#[test]
fn workers_claim_fifo_without_exceeding_worker_capacity() {
    let service = service(2, 4);
    let first = service.submit("first").unwrap();
    let second = service.submit("second").unwrap();
    let third = service.submit("third").unwrap();
    let first_work = service.claim().unwrap();
    let second_work = service.claim().unwrap();
    assert_eq!(first_work.id(), first);
    assert_eq!(first_work.input(), &"first");
    assert_eq!(second_work.id(), second);
    assert!(service.claim().is_none());
    assert_eq!(service.snapshot().running(), 2);
    first_work.complete(1).unwrap();
    let third_work = service.claim().unwrap();
    assert_eq!(third_work.id(), third);
    drop(second_work);
    drop(third_work);
}

#[test]
fn completion_event_and_consumption_have_one_owner() {
    let service = service(1, 2);
    let job = service.submit("input").unwrap();
    service.claim().unwrap().complete(7).unwrap();
    assert_eq!(service.status(job), Some(JobStatus::Completed));
    assert_eq!(service.consume(job).unwrap(), JobOutcome::Completed(7));
    assert_eq!(service.consume(job), Err(ServiceError::UnknownJob(job)));
    assert!(service.is_drained());
}

#[test]
fn cancelling_running_work_detaches_waiter_but_not_worker_owner() {
    let service = service(1, 1);
    let job = service.submit("input").unwrap();
    let work = service.claim().unwrap();
    assert_eq!(service.cancel(job).unwrap(), Cancellation::Running);
    assert_eq!(service.status(job), None);
    assert!(!service.is_drained());
    assert_eq!(work.complete(9).unwrap(), Some(9));
    assert!(service.is_drained());
}

#[test]
fn dropping_a_worker_owner_cannot_strand_a_waiter() {
    let service = service(1, 1);
    let job = service.submit("input").unwrap();
    drop(service.claim().unwrap());
    assert_eq!(service.status(job), Some(JobStatus::Completed));
    assert_eq!(service.consume(job).unwrap(), JobOutcome::WorkerLost);
    assert!(service.is_drained());
}

#[test]
fn running_guard_closes_lifecycle_when_moved_to_another_thread() {
    let service = Arc::new(service(1, 1));
    let job = service.submit("input").unwrap();
    let work = service.claim().unwrap();
    thread::spawn(move || work.complete(42).unwrap())
        .join()
        .unwrap();
    assert_eq!(service.status(job), Some(JobStatus::Completed));
    assert_eq!(service.consume(job).unwrap(), JobOutcome::Completed(42));
}

#[test]
fn lifecycle_transitions_own_wake_publication() {
    let notifications = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&notifications);
    let service =
        BlockingJobService::with_notifier(ServiceCapacity::new(1, 2).unwrap(), move || {
            observed.fetch_add(1, Ordering::SeqCst);
        });
    let completed = service.submit("completed").unwrap();
    service.claim().unwrap().complete(1).unwrap();
    assert_eq!(notifications.load(Ordering::SeqCst), 1);
    service.consume(completed).unwrap();
    assert_eq!(notifications.load(Ordering::SeqCst), 2);

    let abandoned = service.submit("abandoned").unwrap();
    let work = service.claim().unwrap();
    service.cancel(abandoned).unwrap();
    assert_eq!(notifications.load(Ordering::SeqCst), 2);
    drop(work);
    assert_eq!(notifications.load(Ordering::SeqCst), 3);
}

#[test]
fn shutdown_extracts_idle_ownership_and_abandons_running_work() {
    let service = service(1, 3);
    let running = service.submit("running").unwrap();
    let work = service.claim().unwrap();
    let queued = service.submit("queued").unwrap();
    let completed = service.submit("completed").unwrap();
    // The single worker is occupied, so make the first job complete and claim the next two in
    // sequence to construct all three lifecycle categories without bypassing capacity.
    assert_eq!(work.complete(1).unwrap(), None);
    let queued_work = service.claim().unwrap();
    assert_eq!(queued_work.id(), queued);
    queued_work.complete(2).unwrap();
    let completed_work = service.claim().unwrap();
    assert_eq!(completed_work.id(), completed);

    let cleanup = service.begin_shutdown();
    let (queued_values, completed_values) = cleanup.into_parts();
    assert!(queued_values.is_empty());
    assert_eq!(
        completed_values,
        vec![JobOutcome::Completed(1), JobOutcome::Completed(2)]
    );
    assert!(!service.snapshot().accepting());
    assert_eq!(service.submit("later"), Err(SubmitError::Closed("later")));
    assert_eq!(completed_work.complete(3).unwrap(), Some(3));
    assert!(service.is_drained());
    assert_eq!(
        service.cancel(running),
        Err(ServiceError::UnknownJob(running))
    );
}
