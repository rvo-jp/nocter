use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use nocter_blocking_runtime::{Cancellation, JobOutcome, JobStatus, ServiceCapacity, SubmitError};
use nocter_darwin_reactor::DarwinReactor;
use nocter_task_runtime::{
    ReactorInterest, ReadinessDirection, Scheduler, SchedulerProgress, TaskState,
};

use crate::DarwinBlockingService;

fn wait_for_completion<I: Send + 'static, O: Send + 'static>(
    service: &DarwinBlockingService<I, O>,
) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while service.snapshot().completed() == 0 {
        assert!(Instant::now() < deadline, "worker did not complete in time");
        std::thread::yield_now();
    }
}

#[test]
fn fixed_workers_execute_owned_jobs_and_publish_exact_completion() {
    let mut service =
        DarwinBlockingService::new(ServiceCapacity::new(2, 4).unwrap(), |value: &mut String| {
            value.len()
        })
        .unwrap();
    let first = service.submit(String::from("one")).unwrap();
    let second = service.submit(String::from("second")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while service.snapshot().completed() < 2 {
        assert!(
            Instant::now() < deadline,
            "workers did not complete in time"
        );
        std::thread::yield_now();
    }
    assert_eq!(service.status(first), Some(JobStatus::Completed));
    assert_eq!(service.status(second), Some(JobStatus::Completed));
    assert_eq!(service.consume(first).unwrap(), JobOutcome::Completed(3));
    assert_eq!(service.consume(second).unwrap(), JobOutcome::Completed(6));
    service.shutdown().unwrap();
}

#[test]
fn wake_descriptor_composes_with_the_existing_reactor_interest() {
    let mut service =
        DarwinBlockingService::new(ServiceCapacity::new(1, 1).unwrap(), |value: &mut String| {
            value.len()
        })
        .unwrap();
    let mut scheduler = Scheduler::new(DarwinReactor::new().unwrap());
    let task = scheduler.spawn().unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );
    scheduler
        .suspend(
            task,
            [ReactorInterest::Descriptor {
                descriptor: u64::try_from(service.notification_descriptor()).unwrap(),
                direction: ReadinessDirection::Readable,
            }],
        )
        .unwrap();
    let job = service.submit(String::from("ready")).unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );
    assert_eq!(scheduler.state(task), Some(TaskState::Running));
    assert!(service.drain_notifications().unwrap() > 0);
    assert_eq!(service.status(job), Some(JobStatus::Completed));
    assert_eq!(service.consume(job).unwrap(), JobOutcome::Completed(5));
    scheduler.complete(task).unwrap();
    scheduler.consume_completed(task).unwrap();
    service.shutdown().unwrap();
}

#[test]
fn cancellation_during_execution_never_publishes_the_abandoned_output() {
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let worker_entered = Arc::clone(&entered);
    let worker_release = Arc::clone(&release);
    let mut service =
        DarwinBlockingService::new(ServiceCapacity::new(1, 1).unwrap(), move |&mut ()| {
            worker_entered.wait();
            worker_release.wait();
            7
        })
        .unwrap();
    let job = service.submit(()).unwrap();
    entered.wait();
    assert_eq!(service.cancel(job).unwrap(), Cancellation::Running);
    release.wait();
    let deadline = Instant::now() + Duration::from_secs(2);
    while service.snapshot().admitted() != 0 {
        assert!(Instant::now() < deadline, "abandoned worker did not retire");
        std::thread::yield_now();
    }
    assert_eq!(service.status(job), None);
    service.shutdown().unwrap();
}

#[test]
fn saturation_is_backpressure_and_capacity_release_emits_a_wake() {
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let worker_entered = Arc::clone(&entered);
    let worker_release = Arc::clone(&release);
    let mut service =
        DarwinBlockingService::new(ServiceCapacity::new(1, 1).unwrap(), move |_: &mut &str| {
            worker_entered.wait();
            worker_release.wait();
            1
        })
        .unwrap();
    let first = service.submit("first").unwrap();
    entered.wait();
    let SubmitError::Saturated(backpressure) = service.submit("second").unwrap_err() else {
        panic!("expected bounded backpressure")
    };
    let (second, observed) = backpressure.into_parts();
    release.wait();
    wait_for_completion(&service);
    assert_eq!(service.consume(first).unwrap(), JobOutcome::Completed(1));
    assert!(service.capacity_changed_since(observed).unwrap());
    assert_eq!(second, "second");
    assert!(service.drain_notifications().unwrap() > 0);
    service.shutdown().unwrap();
}

#[test]
fn operation_panic_becomes_worker_loss_without_reducing_pool_capacity() {
    let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let worker_attempts = Arc::clone(&attempts);
    let mut service =
        DarwinBlockingService::new(ServiceCapacity::new(1, 2).unwrap(), move |&mut ()| {
            assert!(
                worker_attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst) != 0,
                "scripted operation panic"
            );
            9
        })
        .unwrap();
    let failed = service.submit(()).unwrap();
    wait_for_completion(&service);
    assert_eq!(service.consume(failed).unwrap(), JobOutcome::WorkerLost);
    let succeeded = service.submit(()).unwrap();
    wait_for_completion(&service);
    assert_eq!(
        service.consume(succeeded).unwrap(),
        JobOutcome::Completed(9)
    );
    service.shutdown().unwrap();
}
