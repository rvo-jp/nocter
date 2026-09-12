use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::time::Duration;

use nocter_task_runtime::{ReactorInterest, ReadinessDirection, Scheduler, SchedulerProgress};

use super::DarwinReactor;

fn readable(stream: &UnixStream) -> ReactorInterest {
    ReactorInterest::Descriptor {
        descriptor: u64::try_from(stream.as_raw_fd()).unwrap(),
        direction: ReadinessDirection::Readable,
    }
}

fn process_exit(process: u32) -> ReactorInterest {
    ReactorInterest::ProcessExit {
        process: process.into(),
    }
}

#[test]
fn two_native_descriptors_wake_two_tasks_in_registration_order() {
    let (first_reader, mut first_writer) = UnixStream::pair().unwrap();
    let (second_reader, mut second_writer) = UnixStream::pair().unwrap();
    first_reader.set_nonblocking(true).unwrap();
    second_reader.set_nonblocking(true).unwrap();
    let mut scheduler = Scheduler::new(DarwinReactor::new().unwrap());
    let first = scheduler.spawn().unwrap();
    let second = scheduler.spawn().unwrap();

    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(first)
    );
    scheduler.suspend(first, [readable(&first_reader)]).unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(second)
    );
    scheduler
        .suspend(second, [readable(&second_reader)])
        .unwrap();
    first_writer.write_all(b"a").unwrap();
    second_writer.write_all(b"b").unwrap();

    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(first)
    );
    scheduler.complete(first).unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(second)
    );
}

#[test]
fn a_native_timer_uses_one_fixed_monotonic_deadline() {
    let reactor = DarwinReactor::new().unwrap();
    let deadline = reactor.deadline_after(Duration::from_millis(2));
    let mut scheduler = Scheduler::new(reactor);
    let task = scheduler.spawn().unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );
    scheduler
        .suspend(task, [ReactorInterest::Timer { deadline }])
        .unwrap();

    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );
}

#[test]
fn removing_one_logical_waiter_preserves_another_on_the_same_descriptor() {
    let (reader, mut writer) = UnixStream::pair().unwrap();
    reader.set_nonblocking(true).unwrap();
    let mut scheduler = Scheduler::new(DarwinReactor::new().unwrap());
    let cancelled = scheduler.spawn().unwrap();
    let retained = scheduler.spawn().unwrap();

    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(cancelled)
    );
    scheduler.suspend(cancelled, [readable(&reader)]).unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(retained)
    );
    scheduler.suspend(retained, [readable(&reader)]).unwrap();
    scheduler.cancel(cancelled).unwrap();
    scheduler.finish_cancellation(cancelled).unwrap();
    writer.write_all(b"ready").unwrap();

    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(retained)
    );
}

#[test]
fn cancelling_the_last_waiter_does_not_take_descriptor_ownership() {
    let (reader, mut writer) = UnixStream::pair().unwrap();
    reader.set_nonblocking(true).unwrap();
    let mut scheduler = Scheduler::new(DarwinReactor::new().unwrap());
    let cancelled = scheduler.spawn().unwrap();

    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(cancelled)
    );
    scheduler.suspend(cancelled, [readable(&reader)]).unwrap();
    scheduler.cancel(cancelled).unwrap();
    scheduler.finish_cancellation(cancelled).unwrap();

    let replacement = scheduler.spawn().unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(replacement)
    );
    scheduler.suspend(replacement, [readable(&reader)]).unwrap();
    writer.write_all(b"still-open").unwrap();

    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(replacement)
    );
}

#[test]
fn process_exit_wakes_every_retained_logical_waiter() {
    let mut child = Command::new("/bin/sh")
        .args(["-c", "sleep 0.02"])
        .spawn()
        .unwrap();
    let interest = process_exit(child.id());
    let mut scheduler = Scheduler::new(DarwinReactor::new().unwrap());
    let cancelled = scheduler.spawn().unwrap();
    let retained = scheduler.spawn().unwrap();

    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(cancelled)
    );
    scheduler.suspend(cancelled, [interest]).unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(retained)
    );
    scheduler.suspend(retained, [interest]).unwrap();
    scheduler.cancel(cancelled).unwrap();
    scheduler.finish_cancellation(cancelled).unwrap();

    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(retained)
    );
    assert!(child.wait().unwrap().success());
}

#[test]
fn process_exit_before_registration_is_immediately_ready_without_reaping() {
    let mut child = Command::new("/usr/bin/true").spawn().unwrap();
    std::thread::sleep(Duration::from_millis(20));
    let mut scheduler = Scheduler::new(DarwinReactor::new().unwrap());
    let task = scheduler.spawn().unwrap();

    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );
    scheduler.suspend(task, [process_exit(child.id())]).unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );
    assert!(child.wait().unwrap().success());
}
