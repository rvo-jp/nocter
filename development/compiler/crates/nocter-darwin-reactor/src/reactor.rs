use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::os::fd::RawFd;
use std::time::{Duration, Instant};

use nocter_darwin_event_queue::{EventFilter, EventQueue, EventQueueError};
use nocter_task_runtime::{Reactor, ReactorInterest, ReadinessDirection, RegistrationId};

/// Monotonic counter conversion owned by one reactor instance.
pub trait ReactorClock {
    fn now(&self) -> u64;
    fn duration_until(&self, deadline: u64) -> Duration;
}

/// Nanoseconds elapsed since this clock was created.
pub struct ProcessMonotonicClock {
    epoch: Instant,
}

impl ProcessMonotonicClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            epoch: Instant::now(),
        }
    }

    #[must_use]
    pub fn deadline_after(&self, duration: Duration) -> u64 {
        self.now()
            .saturating_add(u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX))
    }
}

impl Default for ProcessMonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

impl ReactorClock for ProcessMonotonicClock {
    fn now(&self) -> u64 {
        u64::try_from(self.epoch.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    fn duration_until(&self, deadline: u64) -> Duration {
        Duration::from_nanos(deadline.saturating_sub(self.now()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RegistrationLocation {
    Descriptor {
        descriptor: RawFd,
        direction: ReadinessDirection,
    },
    Timer {
        deadline: u64,
    },
    Process {
        process: i32,
    },
    Immediate,
}

#[derive(Debug)]
struct NativeState {
    token: u64,
    registrations: BTreeSet<RegistrationId>,
}

#[derive(Default)]
struct DescriptorState {
    readable: Option<NativeState>,
    writable: Option<NativeState>,
}

impl DescriptorState {
    fn direction(&self, direction: ReadinessDirection) -> Option<&NativeState> {
        match direction {
            ReadinessDirection::Readable => self.readable.as_ref(),
            ReadinessDirection::Writable => self.writable.as_ref(),
        }
    }

    fn direction_mut(&mut self, direction: ReadinessDirection) -> &mut Option<NativeState> {
        match direction {
            ReadinessDirection::Readable => &mut self.readable,
            ReadinessDirection::Writable => &mut self.writable,
        }
    }

    const fn is_empty(&self) -> bool {
        self.readable.is_none() && self.writable.is_none()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeLocation {
    Descriptor {
        descriptor: RawFd,
        direction: ReadinessDirection,
    },
    Process {
        process: i32,
    },
}

/// kqueue-backed Darwin adapter. Task lifecycle remains entirely outside this type.
pub struct DarwinReactor<C = ProcessMonotonicClock> {
    queue: EventQueue,
    clock: C,
    next_token: u64,
    registrations: BTreeMap<RegistrationId, RegistrationLocation>,
    descriptors: BTreeMap<RawFd, DescriptorState>,
    processes: BTreeMap<i32, NativeState>,
    native_tokens: BTreeMap<u64, NativeLocation>,
    timers: BTreeMap<u64, BTreeSet<RegistrationId>>,
    immediate: BTreeSet<RegistrationId>,
}

impl DarwinReactor<ProcessMonotonicClock> {
    /// Creates one reactor and its monotonic deadline domain.
    ///
    /// # Errors
    ///
    /// Returns the native event-queue construction error.
    pub fn new() -> Result<Self, DarwinReactorError> {
        Self::with_clock(ProcessMonotonicClock::new())
    }

    #[must_use]
    pub fn deadline_after(&self, duration: Duration) -> u64 {
        self.clock.deadline_after(duration)
    }
}

impl<C> DarwinReactor<C> {
    /// Creates one reactor over an explicit clock domain.
    ///
    /// # Errors
    ///
    /// Returns the native event-queue construction error.
    pub fn with_clock(clock: C) -> Result<Self, DarwinReactorError> {
        Ok(Self {
            queue: EventQueue::new().map_err(DarwinReactorError::Native)?,
            clock,
            next_token: 0,
            registrations: BTreeMap::new(),
            descriptors: BTreeMap::new(),
            processes: BTreeMap::new(),
            native_tokens: BTreeMap::new(),
            timers: BTreeMap::new(),
            immediate: BTreeSet::new(),
        })
    }

    #[must_use]
    pub const fn clock(&self) -> &C {
        &self.clock
    }

    fn allocate_token(&mut self) -> Result<u64, DarwinReactorError> {
        let token = self.next_token;
        self.next_token = self
            .next_token
            .checked_add(1)
            .ok_or(DarwinReactorError::TokenExhausted)?;
        Ok(token)
    }

    fn register_descriptor(
        &mut self,
        registration: RegistrationId,
        descriptor: u64,
        direction: ReadinessDirection,
    ) -> Result<(), DarwinReactorError> {
        let descriptor = i32::try_from(descriptor)
            .map_err(|_| DarwinReactorError::InvalidDescriptor(descriptor))?;
        if descriptor < 0 {
            return Err(DarwinReactorError::InvalidDescriptor(
                u64::try_from(descriptor).unwrap_or(u64::MAX),
            ));
        }
        if let Some(state) = self.descriptors.get_mut(&descriptor)
            && let Some(native) = state.direction_mut(direction)
        {
            native.registrations.insert(registration);
            self.registrations.insert(
                registration,
                RegistrationLocation::Descriptor {
                    descriptor,
                    direction,
                },
            );
            return Ok(());
        }

        let token = self.allocate_token()?;
        let filter = descriptor_filter(descriptor, direction);
        self.queue
            .register(filter, token)
            .map_err(DarwinReactorError::Native)?;
        let mut registrations = BTreeSet::new();
        registrations.insert(registration);
        *self
            .descriptors
            .entry(descriptor)
            .or_default()
            .direction_mut(direction) = Some(NativeState {
            token,
            registrations,
        });
        self.native_tokens.insert(
            token,
            NativeLocation::Descriptor {
                descriptor,
                direction,
            },
        );
        self.registrations.insert(
            registration,
            RegistrationLocation::Descriptor {
                descriptor,
                direction,
            },
        );
        Ok(())
    }

    fn register_process(
        &mut self,
        registration: RegistrationId,
        process: u64,
    ) -> Result<(), DarwinReactorError> {
        let process = i32::try_from(process)
            .ok()
            .filter(|process| *process > 0)
            .ok_or(DarwinReactorError::InvalidProcess(process))?;
        if let Some(native) = self.processes.get_mut(&process) {
            native.registrations.insert(registration);
        } else {
            let token = self.allocate_token()?;
            if let Err(error) = self
                .queue
                .register(EventFilter::ProcessExit(process), token)
            {
                if error.is_missing_subject() {
                    self.immediate.insert(registration);
                    self.registrations
                        .insert(registration, RegistrationLocation::Immediate);
                    return Ok(());
                }
                return Err(DarwinReactorError::Native(error));
            }
            let mut registrations = BTreeSet::new();
            registrations.insert(registration);
            self.processes.insert(
                process,
                NativeState {
                    token,
                    registrations,
                },
            );
            self.native_tokens
                .insert(token, NativeLocation::Process { process });
        }
        self.registrations
            .insert(registration, RegistrationLocation::Process { process });
        Ok(())
    }

    fn register_timer(&mut self, registration: RegistrationId, deadline: u64) {
        self.timers
            .entry(deadline)
            .or_default()
            .insert(registration);
        self.registrations
            .insert(registration, RegistrationLocation::Timer { deadline });
    }

    fn remove_descriptor_registration(
        &mut self,
        registration: RegistrationId,
        descriptor: RawFd,
        direction: ReadinessDirection,
    ) {
        let Some(state) = self.descriptors.get_mut(&descriptor) else {
            return;
        };
        let native = state.direction_mut(direction);
        let Some(current) = native else {
            return;
        };
        current.registrations.remove(&registration);
        if current.registrations.is_empty() {
            let token = current.token;
            *native = None;
            self.native_tokens.remove(&token);
            let _ = self
                .queue
                .deregister(descriptor_filter(descriptor, direction));
        }
        if state.is_empty() {
            self.descriptors.remove(&descriptor);
        }
    }

    fn remove_process_registration(&mut self, registration: RegistrationId, process: i32) {
        let Some(native) = self.processes.get_mut(&process) else {
            return;
        };
        native.registrations.remove(&registration);
        if native.registrations.is_empty() {
            let token = native.token;
            self.processes.remove(&process);
            self.native_tokens.remove(&token);
            let _ = self.queue.deregister(EventFilter::ProcessExit(process));
        }
    }

    fn remove_timer_registration(&mut self, registration: RegistrationId, deadline: u64) {
        let Some(registrations) = self.timers.get_mut(&deadline) else {
            return;
        };
        registrations.remove(&registration);
        if registrations.is_empty() {
            self.timers.remove(&deadline);
        }
    }

    fn wait_timeout(&self) -> Option<Duration>
    where
        C: ReactorClock,
    {
        self.timers
            .first_key_value()
            .map(|(deadline, _)| self.clock.duration_until(*deadline))
    }

    fn collect_native_events(
        &self,
        events: &[nocter_darwin_event_queue::NativeEvent],
        ready: &mut Vec<RegistrationId>,
    ) -> Result<(), DarwinReactorError> {
        for event in events {
            let Some(location) = self.native_tokens.get(&event.token()) else {
                continue;
            };
            if event.failed() {
                return Err(DarwinReactorError::FailedNativeEvent(event.filter()));
            }
            match *location {
                NativeLocation::Descriptor {
                    descriptor,
                    direction,
                } if event.filter() == descriptor_filter(descriptor, direction) => {
                    let registrations = self
                        .descriptors
                        .get(&descriptor)
                        .and_then(|state| state.direction(direction))
                        .map(|native| &native.registrations);
                    if let Some(registrations) = registrations {
                        ready.extend(registrations.iter().copied());
                    }
                }
                NativeLocation::Process { process }
                    if event.filter() == EventFilter::ProcessExit(process) =>
                {
                    if let Some(native) = self.processes.get(&process) {
                        ready.extend(native.registrations.iter().copied());
                    }
                }
                NativeLocation::Descriptor { .. } | NativeLocation::Process { .. } => {}
            }
        }
        Ok(())
    }

    fn collect_expired_timers(&self, ready: &mut Vec<RegistrationId>)
    where
        C: ReactorClock,
    {
        let now = self.clock.now();
        for (_, registrations) in self.timers.range(..=now) {
            ready.extend(registrations.iter().copied());
        }
    }
}

impl<C: ReactorClock> Reactor for DarwinReactor<C> {
    type Error = DarwinReactorError;

    fn register(
        &mut self,
        registration: RegistrationId,
        interest: ReactorInterest,
    ) -> Result<(), Self::Error> {
        if self.registrations.contains_key(&registration) {
            return Err(DarwinReactorError::DuplicateRegistration(registration));
        }
        match interest {
            ReactorInterest::Descriptor {
                descriptor,
                direction,
            } => self.register_descriptor(registration, descriptor, direction),
            ReactorInterest::Timer { deadline } => {
                self.register_timer(registration, deadline);
                Ok(())
            }
            ReactorInterest::ProcessExit { process } => {
                self.register_process(registration, process)
            }
        }
    }

    fn deregister(&mut self, registration: RegistrationId, _: ReactorInterest) {
        let Some(location) = self.registrations.remove(&registration) else {
            return;
        };
        match location {
            RegistrationLocation::Descriptor {
                descriptor,
                direction,
            } => self.remove_descriptor_registration(registration, descriptor, direction),
            RegistrationLocation::Timer { deadline } => {
                self.remove_timer_registration(registration, deadline);
            }
            RegistrationLocation::Process { process } => {
                self.remove_process_registration(registration, process);
            }
            RegistrationLocation::Immediate => {
                self.immediate.remove(&registration);
            }
        }
    }

    fn wait(&mut self) -> Result<Box<[RegistrationId]>, Self::Error> {
        let mut ready = self.immediate.iter().copied().collect::<Vec<_>>();
        let immediate = !ready.is_empty();
        let events = loop {
            let timeout = immediate
                .then_some(Duration::ZERO)
                .or_else(|| self.wait_timeout());
            match self.queue.wait(self.native_tokens.len(), timeout) {
                Ok(events) => break events,
                Err(error) if error.is_interrupted() => {}
                Err(error) => return Err(DarwinReactorError::Native(error)),
            }
        };
        self.collect_native_events(&events, &mut ready)?;
        self.collect_expired_timers(&mut ready);
        ready.sort_unstable();
        ready.dedup();
        Ok(ready.into_boxed_slice())
    }
}

const fn descriptor_filter(descriptor: RawFd, direction: ReadinessDirection) -> EventFilter {
    match direction {
        ReadinessDirection::Readable => EventFilter::Readable(descriptor),
        ReadinessDirection::Writable => EventFilter::Writable(descriptor),
    }
}

#[derive(Debug)]
pub enum DarwinReactorError {
    DuplicateRegistration(RegistrationId),
    InvalidDescriptor(u64),
    InvalidProcess(u64),
    TokenExhausted,
    FailedNativeEvent(EventFilter),
    Native(EventQueueError),
}

impl fmt::Display for DarwinReactorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Darwin reactor failed: {self:?}")
    }
}

impl std::error::Error for DarwinReactorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Native(error) => Some(error),
            Self::DuplicateRegistration(_)
            | Self::InvalidDescriptor(_)
            | Self::InvalidProcess(_)
            | Self::TokenExhausted
            | Self::FailedNativeEvent(_) => None,
        }
    }
}

#[cfg(test)]
mod tests;
