use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::{
    Computation, ComputationPoll, Executor, ExecutorError, ExecutorProgress, Reactor,
    ReactorInterest, ReadinessDirection, RegistrationId, TaskState, WaitSet,
};

#[derive(Default)]
struct ExecutorReactor {
    active: Vec<(RegistrationId, ReactorInterest)>,
    reject_next: bool,
    empty_waits: usize,
    log: Rc<RefCell<Vec<&'static str>>>,
}

impl Reactor for ExecutorReactor {
    type Error = &'static str;

    fn register(
        &mut self,
        registration: RegistrationId,
        interest: ReactorInterest,
    ) -> Result<(), Self::Error> {
        if self.reject_next {
            self.reject_next = false;
            return Err("registration rejected");
        }
        self.log.borrow_mut().push("register");
        self.active.push((registration, interest));
        Ok(())
    }

    fn deregister(&mut self, registration: RegistrationId, _: ReactorInterest) {
        self.log.borrow_mut().push("deregister");
        self.active.retain(|(actual, _)| *actual != registration);
    }

    fn wait(&mut self) -> Result<Box<[RegistrationId]>, Self::Error> {
        if self.empty_waits != 0 {
            self.empty_waits -= 1;
            return Ok(Box::new([]));
        }
        Ok(self
            .active
            .iter()
            .map(|(registration, _)| *registration)
            .collect::<Vec<_>>()
            .into_boxed_slice())
    }
}

struct ScriptedComputation {
    polls: VecDeque<ComputationPoll>,
    output: u64,
    log: Rc<RefCell<Vec<&'static str>>>,
}

impl ScriptedComputation {
    fn new(polls: impl IntoIterator<Item = ComputationPoll>, output: u64) -> Self {
        Self {
            polls: polls.into_iter().collect(),
            output,
            log: Rc::default(),
        }
    }

    fn with_log(
        polls: impl IntoIterator<Item = ComputationPoll>,
        output: u64,
        log: Rc<RefCell<Vec<&'static str>>>,
    ) -> Self {
        Self {
            polls: polls.into_iter().collect(),
            output,
            log,
        }
    }
}

impl Computation for ScriptedComputation {
    type Output = u64;

    fn resume(&mut self) -> ComputationPoll {
        self.log.borrow_mut().push("resume");
        self.polls.pop_front().expect("scripted poll exists")
    }

    fn cancel(self) {
        self.log.borrow_mut().push("cancel");
    }

    fn consume(self) -> Self::Output {
        self.log.borrow_mut().push("consume");
        self.output
    }
}

const fn readable(descriptor: u64) -> ReactorInterest {
    ReactorInterest::Descriptor {
        descriptor,
        direction: ReadinessDirection::Readable,
    }
}

#[test]
fn executor_owns_poll_wait_completion_and_consumption_order() {
    let mut executor = Executor::new(ExecutorReactor::default());
    let task = executor
        .spawn(ScriptedComputation::new(
            [
                ComputationPoll::Pending(WaitSet::one(readable(3))),
                ComputationPoll::Completed,
            ],
            42,
        ))
        .unwrap();

    assert_eq!(executor.step().unwrap(), ExecutorProgress::Resumed(task));
    assert_eq!(executor.scheduler().state(task), Some(TaskState::Waiting));
    assert_eq!(executor.step().unwrap(), ExecutorProgress::Resumed(task));
    assert_eq!(executor.scheduler().state(task), Some(TaskState::Completed));
    assert_eq!(executor.consume(task).unwrap(), 42);
    assert_eq!(executor.scheduler().state(task), None);
}

#[test]
fn executor_detaches_waits_before_invoking_computation_cancellation() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let reactor = ExecutorReactor {
        log: Rc::clone(&log),
        ..ExecutorReactor::default()
    };
    let mut executor = Executor::new(reactor);
    let task = executor
        .spawn(ScriptedComputation::with_log(
            [ComputationPoll::Pending(WaitSet::one(readable(4)))],
            0,
            Rc::clone(&log),
        ))
        .unwrap();
    executor.step().unwrap();

    executor.cancel(task).unwrap();

    let log = log.borrow();
    let deregister = log.iter().position(|event| *event == "deregister").unwrap();
    let cancel = log.iter().position(|event| *event == "cancel").unwrap();
    assert!(deregister < cancel);
    assert_eq!(executor.scheduler().state(task), None);
}

#[test]
fn failed_wait_registration_returns_the_task_to_the_runnable_queue() {
    let reactor = ExecutorReactor {
        reject_next: true,
        ..ExecutorReactor::default()
    };
    let mut executor = Executor::new(reactor);
    let task = executor
        .spawn(ScriptedComputation::new(
            [
                ComputationPoll::Pending(WaitSet::one(readable(5))),
                ComputationPoll::Completed,
            ],
            7,
        ))
        .unwrap();

    assert!(matches!(
        executor.step(),
        Err(ExecutorError::Scheduler(crate::SchedulerError::Reactor(
            "registration rejected"
        )))
    ));
    assert_eq!(executor.scheduler().state(task), Some(TaskState::Runnable));
    executor.run_until_idle().unwrap();
    assert_eq!(executor.consume(task).unwrap(), 7);
}

#[test]
fn executor_does_not_treat_an_empty_reactor_batch_as_quiescence() {
    let reactor = ExecutorReactor {
        empty_waits: 1,
        ..ExecutorReactor::default()
    };
    let mut executor = Executor::new(reactor);
    let task = executor
        .spawn(ScriptedComputation::new(
            [
                ComputationPoll::Pending(WaitSet::one(readable(7))),
                ComputationPoll::Completed,
            ],
            9,
        ))
        .unwrap();

    executor.run_until_idle().unwrap();

    assert_eq!(executor.scheduler().state(task), Some(TaskState::Completed));
    assert_eq!(executor.consume(task).unwrap(), 9);
}

#[test]
fn orderly_shutdown_cancels_waiting_and_completed_payloads() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let reactor = ExecutorReactor {
        log: Rc::clone(&log),
        ..ExecutorReactor::default()
    };
    let mut executor = Executor::new(reactor);
    let waiting = executor
        .spawn(ScriptedComputation::with_log(
            [ComputationPoll::Pending(WaitSet::one(readable(6)))],
            0,
            Rc::clone(&log),
        ))
        .unwrap();
    let completed = executor
        .spawn(ScriptedComputation::with_log(
            [ComputationPoll::Completed],
            0,
            Rc::clone(&log),
        ))
        .unwrap();
    executor.step().unwrap();
    executor.step().unwrap();

    executor.shutdown().unwrap();

    assert_eq!(executor.scheduler().state(waiting), None);
    assert_eq!(executor.scheduler().state(completed), None);
    assert_eq!(
        log.borrow()
            .iter()
            .filter(|event| **event == "cancel")
            .count(),
        2
    );
}
