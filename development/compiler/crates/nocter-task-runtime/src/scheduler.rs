use std::collections::VecDeque;
use std::fmt;

use crate::{Reactor, ReactorInterest, RegistrationId, TaskId};

/// Observable lifecycle state of one current-generation task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskState {
    Runnable,
    Running,
    Waiting,
    Completed,
    Cancelling(CancellationKind),
}

/// Which computation-owned storage must be destroyed after cancellation detaches all wakeups.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationKind {
    PendingFrame,
    CompletedOutput,
}

/// One scheduler step visible to the executor loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerProgress {
    Resume(TaskId),
    Idle,
}

/// One cleanup obligation emitted only after its task has been detached from every wakeup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaskCancellation {
    task: TaskId,
    kind: CancellationKind,
}

impl TaskCancellation {
    const fn new(task: TaskId, kind: CancellationKind) -> Self {
        Self { task, kind }
    }

    #[must_use]
    pub const fn task(self) -> TaskId {
        self.task
    }

    #[must_use]
    pub const fn kind(self) -> CancellationKind {
        self.kind
    }
}

#[derive(Debug)]
pub enum SchedulerError<E> {
    UnknownTask(TaskId),
    InvalidTaskState { task: TaskId, actual: TaskState },
    EmptyWaitSet(TaskId),
    CapacityExhausted,
    Reactor(E),
}

impl<E: fmt::Debug> fmt::Display for SchedulerError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "asynchronous scheduler transition failed: {self:?}"
        )
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SchedulerError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Reactor(error) => Some(error),
            Self::UnknownTask(_)
            | Self::InvalidTaskState { .. }
            | Self::EmptyWaitSet(_)
            | Self::CapacityExhausted => None,
        }
    }
}

struct TaskSlot {
    generation: u32,
    state: Option<TaskSlotState>,
}

enum TaskSlotState {
    Runnable,
    Running,
    Waiting(Box<[RegistrationId]>),
    Completed,
    Cancelling(CancellationKind),
}

impl TaskSlotState {
    const fn public(&self) -> TaskState {
        match self {
            Self::Runnable => TaskState::Runnable,
            Self::Running => TaskState::Running,
            Self::Waiting(_) => TaskState::Waiting,
            Self::Completed => TaskState::Completed,
            Self::Cancelling(kind) => TaskState::Cancelling(*kind),
        }
    }
}

struct RegistrationSlot {
    generation: u32,
    active: Option<Registration>,
}

#[derive(Clone, Copy)]
struct Registration {
    task: TaskId,
    interest: ReactorInterest,
}

/// Single-threaded scheduler state. Computation bytes and resume/cancel functions remain owned by
/// the generated executor; this type owns only lifecycle and wakeup identity.
pub struct Scheduler<R> {
    reactor: R,
    tasks: Vec<TaskSlot>,
    free_tasks: Vec<u32>,
    registrations: Vec<RegistrationSlot>,
    free_registrations: Vec<u32>,
    runnable: VecDeque<TaskId>,
}

impl<R: Reactor> Scheduler<R> {
    #[must_use]
    pub fn new(reactor: R) -> Self {
        Self {
            reactor,
            tasks: Vec::new(),
            free_tasks: Vec::new(),
            registrations: Vec::new(),
            free_registrations: Vec::new(),
            runnable: VecDeque::new(),
        }
    }

    #[must_use]
    pub const fn reactor(&self) -> &R {
        &self.reactor
    }

    #[must_use]
    pub fn state(&self, task: TaskId) -> Option<TaskState> {
        self.task_slot(task)
            .and_then(|slot| slot.state.as_ref())
            .map(TaskSlotState::public)
    }

    /// Installs one pending computation and makes it runnable exactly once.
    ///
    /// # Errors
    ///
    /// Returns `CapacityExhausted` when no generation-qualified task identity remains.
    pub fn spawn(&mut self) -> Result<TaskId, SchedulerError<R::Error>> {
        let task = self.allocate_task()?;
        self.runnable.push_back(task);
        Ok(task)
    }

    /// Obtains the next runnable task, blocking in the reactor only when no runnable task exists.
    ///
    /// # Errors
    ///
    /// Returns a reactor wait error or an invalid internal transition.
    pub fn next_progress(&mut self) -> Result<SchedulerProgress, SchedulerError<R::Error>> {
        if let Some(task) = self.take_runnable()? {
            return Ok(SchedulerProgress::Resume(task));
        }
        if !self.has_active_registration() {
            return Ok(SchedulerProgress::Idle);
        }
        let events = self.reactor.wait().map_err(SchedulerError::Reactor)?;
        self.accept_events(&events)?;
        Ok(self
            .take_runnable()?
            .map_or(SchedulerProgress::Idle, SchedulerProgress::Resume))
    }

    /// Returns a running task to the FIFO runnable queue.
    ///
    /// # Errors
    ///
    /// Rejects an unknown task or a task not currently running.
    pub fn yield_now(&mut self, task: TaskId) -> Result<(), SchedulerError<R::Error>> {
        self.transition_state(task, TaskState::Running, TaskSlotState::Runnable)?;
        self.runnable.push_back(task);
        Ok(())
    }

    /// Atomically registers one complete wait set and suspends a running task.
    ///
    /// A partial reactor-registration failure removes every installed member and leaves the task
    /// running, so no half-visible wait state can escape.
    ///
    /// # Errors
    ///
    /// Rejects an unknown or non-running task, an empty wait set, identity exhaustion, or a
    /// reactor registration failure.
    pub fn suspend(
        &mut self,
        task: TaskId,
        interests: impl IntoIterator<Item = ReactorInterest>,
    ) -> Result<Box<[RegistrationId]>, SchedulerError<R::Error>> {
        self.require_state(task, TaskState::Running)?;
        let interests = interests.into_iter().collect::<Vec<_>>();
        if interests.is_empty() {
            return Err(SchedulerError::EmptyWaitSet(task));
        }
        let mut installed = Vec::with_capacity(interests.len());
        for interest in interests {
            let registration = match self.allocate_registration(task, interest) {
                Ok(registration) => registration,
                Err(error) => {
                    self.remove_registrations_reverse(&installed);
                    return Err(error);
                }
            };
            if let Err(error) = self.reactor.register(registration, interest) {
                self.release_registration(registration);
                self.remove_registrations_reverse(&installed);
                return Err(SchedulerError::Reactor(error));
            }
            installed.push(registration);
        }
        let registrations = installed.into_boxed_slice();
        self.transition_state(
            task,
            TaskState::Running,
            TaskSlotState::Waiting(registrations.clone()),
        )?;
        Ok(registrations)
    }

    /// Marks a running task complete. Its output remains owned until consumed or cancelled.
    ///
    /// # Errors
    ///
    /// Rejects an unknown task or a task not currently running.
    pub fn complete(&mut self, task: TaskId) -> Result<(), SchedulerError<R::Error>> {
        self.transition_state(task, TaskState::Running, TaskSlotState::Completed)
    }

    /// Detaches every possible wakeup before publishing the computation cleanup obligation.
    ///
    /// # Errors
    ///
    /// Rejects an unknown task, a running task, or a cancellation already in progress.
    pub fn cancel(&mut self, task: TaskId) -> Result<CancellationKind, SchedulerError<R::Error>> {
        let state = self.take_state(task)?;
        let kind = match state {
            TaskSlotState::Runnable => CancellationKind::PendingFrame,
            TaskSlotState::Waiting(registrations) => {
                for registration in registrations.iter().copied() {
                    self.remove_registration(registration);
                }
                CancellationKind::PendingFrame
            }
            TaskSlotState::Completed => CancellationKind::CompletedOutput,
            TaskSlotState::Running | TaskSlotState::Cancelling(_) => {
                let actual = state.public();
                self.store_state(task, state)?;
                return Err(SchedulerError::InvalidTaskState { task, actual });
            }
        };
        self.store_state(task, TaskSlotState::Cancelling(kind))?;
        Ok(kind)
    }

    /// Releases a task only after its published cancellation cleanup has completed.
    ///
    /// # Errors
    ///
    /// Rejects an unknown task or a task without a published cancellation obligation.
    pub fn finish_cancellation(&mut self, task: TaskId) -> Result<(), SchedulerError<R::Error>> {
        let actual = self.state(task).ok_or(SchedulerError::UnknownTask(task))?;
        if !matches!(actual, TaskState::Cancelling(_)) {
            return Err(SchedulerError::InvalidTaskState { task, actual });
        }
        self.release_task(task);
        Ok(())
    }

    /// Transfers a completed result out of scheduler ownership and retires the task identity.
    ///
    /// # Errors
    ///
    /// Rejects an unknown task or a task that has not completed.
    pub fn consume_completed(&mut self, task: TaskId) -> Result<(), SchedulerError<R::Error>> {
        self.require_state(task, TaskState::Completed)?;
        self.release_task(task);
        Ok(())
    }

    /// Detaches all wakeups and publishes every cleanup required for orderly executor shutdown.
    /// No state changes occur when a task is currently running or already being cancelled.
    ///
    /// # Errors
    ///
    /// Rejects shutdown while a task is running or cancellation cleanup is already in progress.
    pub fn begin_shutdown(&mut self) -> Result<Box<[TaskCancellation]>, SchedulerError<R::Error>> {
        let tasks = self
            .tasks
            .iter()
            .enumerate()
            .filter_map(|(slot, entry)| entry.state.as_ref().map(|_| (slot, entry.generation)))
            .map(|(slot, generation)| {
                u32::try_from(slot)
                    .map(|slot| TaskId::new(slot, generation))
                    .map_err(|_| SchedulerError::CapacityExhausted)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if let Some((task, actual)) = tasks.iter().find_map(|task| {
            self.state(*task).and_then(|state| {
                matches!(state, TaskState::Running | TaskState::Cancelling(_))
                    .then_some((*task, state))
            })
        }) {
            return Err(SchedulerError::InvalidTaskState { task, actual });
        }
        tasks
            .into_iter()
            .map(|task| {
                self.cancel(task)
                    .map(|kind| TaskCancellation::new(task, kind))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice)
    }

    fn take_runnable(&mut self) -> Result<Option<TaskId>, SchedulerError<R::Error>> {
        while let Some(task) = self.runnable.pop_front() {
            if self.state(task) == Some(TaskState::Runnable) {
                self.transition_state(task, TaskState::Runnable, TaskSlotState::Running)?;
                return Ok(Some(task));
            }
        }
        Ok(None)
    }

    fn accept_events(&mut self, events: &[RegistrationId]) -> Result<(), SchedulerError<R::Error>> {
        let mut ready = events
            .iter()
            .copied()
            .filter_map(|registration| {
                self.registration(registration)
                    .map(|active| (registration, active.task))
            })
            .collect::<Vec<_>>();
        ready.sort_unstable();
        for (registration, task) in ready {
            let Some(TaskSlotState::Waiting(registrations)) =
                self.task_slot(task).and_then(|slot| slot.state.as_ref())
            else {
                continue;
            };
            if !registrations.contains(&registration) {
                continue;
            }
            let registrations = registrations.clone();
            for member in registrations.iter().copied() {
                self.remove_registration(member);
            }
            self.transition_state(task, TaskState::Waiting, TaskSlotState::Runnable)?;
            self.runnable.push_back(task);
        }
        Ok(())
    }

    fn allocate_task(&mut self) -> Result<TaskId, SchedulerError<R::Error>> {
        let slot = if let Some(slot) = self.free_tasks.pop() {
            slot
        } else {
            let slot =
                u32::try_from(self.tasks.len()).map_err(|_| SchedulerError::CapacityExhausted)?;
            self.tasks.push(TaskSlot {
                generation: 0,
                state: None,
            });
            slot
        };
        let entry = &mut self.tasks[slot as usize];
        debug_assert!(entry.state.is_none());
        entry.state = Some(TaskSlotState::Runnable);
        Ok(TaskId::new(slot, entry.generation))
    }

    fn release_task(&mut self, task: TaskId) {
        let entry = &mut self.tasks[task.slot() as usize];
        entry.state = None;
        if let Some(generation) = entry.generation.checked_add(1) {
            entry.generation = generation;
            self.free_tasks.push(task.slot());
        }
    }

    fn allocate_registration(
        &mut self,
        task: TaskId,
        interest: ReactorInterest,
    ) -> Result<RegistrationId, SchedulerError<R::Error>> {
        let slot = if let Some(slot) = self.free_registrations.pop() {
            slot
        } else {
            let slot = u32::try_from(self.registrations.len())
                .map_err(|_| SchedulerError::CapacityExhausted)?;
            self.registrations.push(RegistrationSlot {
                generation: 0,
                active: None,
            });
            slot
        };
        let entry = &mut self.registrations[slot as usize];
        debug_assert!(entry.active.is_none());
        entry.active = Some(Registration { task, interest });
        Ok(RegistrationId::new(slot, entry.generation))
    }

    fn remove_registration(&mut self, registration: RegistrationId) {
        let Some(active) = self.registration(registration) else {
            return;
        };
        self.release_registration(registration);
        self.reactor.deregister(registration, active.interest);
    }

    fn remove_registrations_reverse(&mut self, registrations: &[RegistrationId]) {
        for registration in registrations.iter().rev().copied() {
            self.remove_registration(registration);
        }
    }

    fn release_registration(&mut self, registration: RegistrationId) {
        let entry = &mut self.registrations[registration.slot() as usize];
        entry.active = None;
        if let Some(generation) = entry.generation.checked_add(1) {
            entry.generation = generation;
            self.free_registrations.push(registration.slot());
        }
    }

    fn registration(&self, id: RegistrationId) -> Option<Registration> {
        self.registrations
            .get(id.slot() as usize)
            .filter(|slot| slot.generation == id.generation())
            .and_then(|slot| slot.active)
    }

    fn has_active_registration(&self) -> bool {
        self.registrations
            .iter()
            .any(|registration| registration.active.is_some())
    }

    fn require_state(
        &self,
        task: TaskId,
        expected: TaskState,
    ) -> Result<(), SchedulerError<R::Error>> {
        let actual = self.state(task).ok_or(SchedulerError::UnknownTask(task))?;
        if actual != expected {
            return Err(SchedulerError::InvalidTaskState { task, actual });
        }
        Ok(())
    }

    fn transition_state(
        &mut self,
        task: TaskId,
        expected: TaskState,
        next: TaskSlotState,
    ) -> Result<(), SchedulerError<R::Error>> {
        self.require_state(task, expected)?;
        self.store_state(task, next)
    }

    fn take_state(&mut self, task: TaskId) -> Result<TaskSlotState, SchedulerError<R::Error>> {
        self.task_slot_mut(task)
            .ok_or(SchedulerError::UnknownTask(task))?
            .state
            .take()
            .ok_or(SchedulerError::UnknownTask(task))
    }

    fn store_state(
        &mut self,
        task: TaskId,
        state: TaskSlotState,
    ) -> Result<(), SchedulerError<R::Error>> {
        let slot = self
            .task_slot_mut(task)
            .ok_or(SchedulerError::UnknownTask(task))?;
        slot.state = Some(state);
        Ok(())
    }

    fn task_slot(&self, task: TaskId) -> Option<&TaskSlot> {
        self.tasks
            .get(task.slot() as usize)
            .filter(|slot| slot.generation == task.generation())
    }

    fn task_slot_mut(&mut self, task: TaskId) -> Option<&mut TaskSlot> {
        self.tasks
            .get_mut(task.slot() as usize)
            .filter(|slot| slot.generation == task.generation())
    }
}
