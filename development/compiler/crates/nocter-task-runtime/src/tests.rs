use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

use crate::{
    CancellationKind, Reactor, ReactorInterest, ReadinessDirection, RegistrationId, Scheduler,
    SchedulerProgress, TaskState,
};

#[derive(Default)]
struct ModelReactor {
    registered: Vec<(RegistrationId, ReactorInterest)>,
    removed: Vec<(RegistrationId, ReactorInterest)>,
    batches: RefCell<VecDeque<Box<[RegistrationId]>>>,
    reject_at: Option<usize>,
    wait_count: Cell<usize>,
}

impl Reactor for ModelReactor {
    type Error = &'static str;

    fn register(
        &mut self,
        registration: RegistrationId,
        interest: ReactorInterest,
    ) -> Result<(), Self::Error> {
        if self.reject_at == Some(self.registered.len()) {
            return Err("registration rejected");
        }
        self.registered.push((registration, interest));
        Ok(())
    }

    fn deregister(&mut self, registration: RegistrationId, interest: ReactorInterest) {
        self.removed.push((registration, interest));
    }

    fn wait(&mut self) -> Result<Box<[RegistrationId]>, Self::Error> {
        self.wait_count.set(self.wait_count.get() + 1);
        Ok(self.batches.borrow_mut().pop_front().unwrap_or_default())
    }
}

const fn readable(descriptor: u64) -> ReactorInterest {
    ReactorInterest::Descriptor {
        descriptor,
        direction: ReadinessDirection::Readable,
    }
}

#[test]
fn runnable_tasks_resume_in_fifo_order() {
    let mut scheduler = Scheduler::new(ModelReactor::default());
    let first = scheduler.spawn().unwrap();
    let second = scheduler.spawn().unwrap();

    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(first)
    );
    scheduler.yield_now(first).unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(second)
    );
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(first)
    );
}

#[test]
fn an_executor_without_runnable_or_waiting_tasks_never_enters_the_reactor() {
    let mut scheduler = Scheduler::new(ModelReactor::default());

    assert_eq!(scheduler.next_progress().unwrap(), SchedulerProgress::Idle);
    assert_eq!(scheduler.reactor().wait_count.get(), 0);

    let task = scheduler.spawn().unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );
    scheduler.complete(task).unwrap();
    assert_eq!(scheduler.next_progress().unwrap(), SchedulerProgress::Idle);
    assert_eq!(scheduler.reactor().wait_count.get(), 0);
}

#[test]
fn stale_registration_cannot_wake_a_reused_slot() {
    let mut scheduler = Scheduler::new(ModelReactor::default());
    let first = scheduler.spawn().unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(first)
    );
    let old = scheduler.suspend(first, [readable(7)]).unwrap()[0];
    assert_eq!(
        scheduler.cancel(first).unwrap(),
        CancellationKind::PendingFrame
    );
    scheduler.finish_cancellation(first).unwrap();

    let second = scheduler.spawn().unwrap();
    assert_ne!(first, second);
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(second)
    );
    let current = scheduler.suspend(second, [readable(7)]).unwrap()[0];
    assert_eq!(old.slot(), current.slot());
    assert_ne!(old.generation(), current.generation());
    scheduler
        .reactor()
        .batches
        .borrow_mut()
        .push_back(Box::new([old]));

    assert_eq!(scheduler.next_progress().unwrap(), SchedulerProgress::Idle);
    assert_eq!(scheduler.state(second), Some(TaskState::Waiting));
}

#[test]
fn one_event_consumes_a_complete_wait_set_and_simultaneous_timeout_is_stale() {
    let mut scheduler = Scheduler::new(ModelReactor::default());
    let task = scheduler.spawn().unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );
    let registrations = scheduler
        .suspend(task, [ReactorInterest::Timer { deadline: 10 }, readable(3)])
        .unwrap();
    let timer = registrations[0];
    let descriptor = registrations[1];
    scheduler
        .reactor()
        .batches
        .borrow_mut()
        .push_back(Box::new([timer, descriptor]));

    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );
    assert_eq!(scheduler.reactor().removed.len(), 2);
    scheduler.yield_now(task).unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );
}

#[test]
fn process_completion_uses_the_same_generation_qualified_wait_set() {
    let mut scheduler = Scheduler::new(ModelReactor::default());
    let task = scheduler.spawn().unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );
    let registrations = scheduler
        .suspend(
            task,
            [
                ReactorInterest::ProcessExit { process: 41 },
                ReactorInterest::Timer { deadline: 10 },
            ],
        )
        .unwrap();
    let process = registrations[0];
    let timer = registrations[1];
    scheduler
        .reactor()
        .batches
        .borrow_mut()
        .push_back(Box::new([timer, process]));

    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );
    assert_eq!(scheduler.reactor().removed.len(), 2);
    assert_eq!(scheduler.state(task), Some(TaskState::Running));
}

#[test]
fn cancellation_detaches_waiters_before_exposing_cleanup() {
    let mut scheduler = Scheduler::new(ModelReactor::default());
    let task = scheduler.spawn().unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );
    let registrations = scheduler
        .suspend(task, [readable(4), ReactorInterest::Timer { deadline: 20 }])
        .unwrap();

    assert_eq!(
        scheduler.cancel(task).unwrap(),
        CancellationKind::PendingFrame
    );
    assert_eq!(
        scheduler.state(task),
        Some(TaskState::Cancelling(CancellationKind::PendingFrame))
    );
    assert_eq!(
        scheduler
            .reactor()
            .removed
            .iter()
            .map(|(registration, _)| *registration)
            .collect::<Vec<_>>(),
        registrations.as_ref()
    );
    scheduler.finish_cancellation(task).unwrap();
    assert_eq!(scheduler.state(task), None);
}

#[test]
fn completed_cancellation_selects_output_destruction() {
    let mut scheduler = Scheduler::new(ModelReactor::default());
    let task = scheduler.spawn().unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );
    scheduler.complete(task).unwrap();

    assert_eq!(
        scheduler.cancel(task).unwrap(),
        CancellationKind::CompletedOutput
    );
}

#[test]
fn running_tasks_cannot_be_cancelled_between_executor_transitions() {
    let mut scheduler = Scheduler::new(ModelReactor::default());
    let task = scheduler.spawn().unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );

    assert!(scheduler.cancel(task).is_err());
    assert_eq!(scheduler.state(task), Some(TaskState::Running));
}

#[test]
fn runnable_cancellation_publishes_pending_frame_cleanup() {
    let mut scheduler = Scheduler::new(ModelReactor::default());
    let task = scheduler.spawn().unwrap();

    assert_eq!(
        scheduler.cancel(task).unwrap(),
        CancellationKind::PendingFrame
    );
    assert_eq!(scheduler.next_progress().unwrap(), SchedulerProgress::Idle);
}

#[test]
fn failed_wait_registration_rolls_back_without_suspending_the_task() {
    let reactor = ModelReactor {
        reject_at: Some(0),
        ..ModelReactor::default()
    };
    let mut scheduler = Scheduler::new(reactor);
    let task = scheduler.spawn().unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );

    assert!(scheduler.suspend(task, [readable(9)]).is_err());
    assert_eq!(scheduler.state(task), Some(TaskState::Running));
}

#[test]
fn partial_wait_registration_failure_removes_the_installed_prefix() {
    let reactor = ModelReactor {
        reject_at: Some(1),
        ..ModelReactor::default()
    };
    let mut scheduler = Scheduler::new(reactor);
    let task = scheduler.spawn().unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(task)
    );

    assert!(
        scheduler
            .suspend(task, [readable(9), ReactorInterest::Timer { deadline: 30 }])
            .is_err()
    );
    assert_eq!(scheduler.state(task), Some(TaskState::Running));
    assert_eq!(scheduler.reactor().registered.len(), 1);
    assert_eq!(scheduler.reactor().removed.len(), 1);
    assert_eq!(
        scheduler.reactor().registered[0],
        scheduler.reactor().removed[0]
    );
}

#[test]
fn shutdown_detaches_waiting_tasks_and_distinguishes_completed_outputs() {
    let mut scheduler = Scheduler::new(ModelReactor::default());
    let waiting = scheduler.spawn().unwrap();
    let completed = scheduler.spawn().unwrap();
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(waiting)
    );
    let registration = scheduler.suspend(waiting, [readable(11)]).unwrap()[0];
    assert_eq!(
        scheduler.next_progress().unwrap(),
        SchedulerProgress::Resume(completed)
    );
    scheduler.complete(completed).unwrap();

    assert_eq!(
        scheduler
            .begin_shutdown()
            .unwrap()
            .iter()
            .map(|cancellation| (cancellation.task(), cancellation.kind()))
            .collect::<Vec<_>>(),
        [
            (waiting, CancellationKind::PendingFrame),
            (completed, CancellationKind::CompletedOutput),
        ]
    );
    assert_eq!(scheduler.reactor().removed[0].0, registration);
}
