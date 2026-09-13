use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use nocter_blocking_runtime::{RetirementCapacity, RetirementReserveError, RetirementStatus};

use crate::{DarwinRetirementService, RetirementShutdownError};

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "retirement worker did not finish"
        );
        std::thread::yield_now();
    }
}

#[test]
fn owner_drop_enters_the_reserved_worker_queue_and_releases_capacity() {
    let cleaned = Arc::new(AtomicUsize::new(0));
    let worker_cleaned = Arc::clone(&cleaned);
    let mut service = DarwinRetirementService::new(
        RetirementCapacity::new(1, 1).unwrap(),
        move |value: &mut String| {
            assert_eq!(value, "file");
            worker_cleaned.fetch_add(1, Ordering::SeqCst);
        },
    )
    .unwrap();
    let owner = service.reserve().unwrap().attach(String::from("file"));
    let Err(RetirementReserveError::Saturated(backpressure)) = service.reserve() else {
        panic!("expected bounded resource backpressure")
    };
    drop(owner);
    wait_until(|| service.snapshot().drained());
    assert_eq!(cleaned.load(Ordering::SeqCst), 1);
    assert!(
        service
            .capacity_changed_since(backpressure.observed_epoch())
            .unwrap()
    );
    assert!(service.drain_notifications().unwrap() > 0);
    service.shutdown().unwrap();
}

#[test]
fn shutdown_rejects_external_owners_then_drains_their_guaranteed_retirement() {
    let mut service =
        DarwinRetirementService::new(RetirementCapacity::new(1, 1).unwrap(), |_: &mut String| {})
            .unwrap();
    let owner = service.reserve().unwrap().attach(String::from("file"));
    assert_eq!(
        service.shutdown(),
        Err(RetirementShutdownError::OutstandingOwners(1))
    );
    assert!(matches!(
        service.reserve(),
        Err(RetirementReserveError::Closed)
    ));
    drop(owner);
    wait_until(|| service.snapshot().drained());
    service.shutdown().unwrap();
}

#[test]
fn explicit_retirement_publishes_exact_completion_before_releasing_capacity() {
    let mut service =
        DarwinRetirementService::new(RetirementCapacity::new(1, 1).unwrap(), |_: &mut String| {})
            .unwrap();
    let retirement = service
        .reserve()
        .unwrap()
        .attach(String::from("file"))
        .retire()
        .unwrap();
    wait_until(|| service.status(retirement) == Some(RetirementStatus::Completed));
    assert_eq!(service.snapshot().reserved(), 1);
    service.consume(retirement).unwrap();
    assert!(service.snapshot().drained());
    service.shutdown().unwrap();
}

#[test]
fn shutdown_detaches_waiters_and_joins_already_queued_cleanup() {
    let cleaned = Arc::new(AtomicUsize::new(0));
    let worker_cleaned = Arc::clone(&cleaned);
    let mut service = DarwinRetirementService::new(
        RetirementCapacity::new(1, 1).unwrap(),
        move |_: &mut String| {
            worker_cleaned.fetch_add(1, Ordering::SeqCst);
        },
    )
    .unwrap();
    let _retirement = service
        .reserve()
        .unwrap()
        .attach(String::from("file"))
        .retire()
        .unwrap();
    service.shutdown().unwrap();
    assert_eq!(cleaned.load(Ordering::SeqCst), 1);
}

#[test]
fn adapter_drop_detaches_but_does_not_abandon_an_issued_owner() {
    let cleaned = Arc::new(AtomicUsize::new(0));
    let worker_cleaned = Arc::clone(&cleaned);
    let owner = {
        let service = DarwinRetirementService::new(
            RetirementCapacity::new(1, 1).unwrap(),
            move |_: &mut String| {
                worker_cleaned.fetch_add(1, Ordering::SeqCst);
            },
        )
        .unwrap();
        service.reserve().unwrap().attach(String::from("file"))
    };
    assert_eq!(cleaned.load(Ordering::SeqCst), 0);
    drop(owner);
    wait_until(|| cleaned.load(Ordering::SeqCst) == 1);
}

#[test]
fn cleanup_panic_releases_capacity_and_the_fixed_worker_continues() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let worker_attempts = Arc::clone(&attempts);
    let mut service = DarwinRetirementService::new(
        RetirementCapacity::new(1, 1).unwrap(),
        move |_: &mut String| {
            assert!(
                worker_attempts.fetch_add(1, Ordering::SeqCst) != 0,
                "scripted cleanup panic"
            );
        },
    )
    .unwrap();
    drop(service.reserve().unwrap().attach(String::from("first")));
    wait_until(|| service.snapshot().drained());
    drop(service.reserve().unwrap().attach(String::from("second")));
    wait_until(|| service.snapshot().drained());
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    service.shutdown().unwrap();
}
