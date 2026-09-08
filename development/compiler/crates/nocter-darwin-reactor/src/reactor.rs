use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::os::fd::RawFd;
use std::time::{Duration, Instant};

use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token};
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
}

struct DescriptorState {
    token: Token,
    readable: BTreeSet<RegistrationId>,
    writable: BTreeSet<RegistrationId>,
}

impl DescriptorState {
    fn interest(&self) -> Option<Interest> {
        match (self.readable.is_empty(), self.writable.is_empty()) {
            (false, false) => Some(Interest::READABLE | Interest::WRITABLE),
            (false, true) => Some(Interest::READABLE),
            (true, false) => Some(Interest::WRITABLE),
            (true, true) => None,
        }
    }
}

/// kqueue-backed Darwin adapter. Task lifecycle remains entirely outside this type.
pub struct DarwinReactor<C = ProcessMonotonicClock> {
    poll: Poll,
    events: Events,
    clock: C,
    next_token: usize,
    registrations: BTreeMap<RegistrationId, RegistrationLocation>,
    descriptors: BTreeMap<RawFd, DescriptorState>,
    descriptor_tokens: BTreeMap<Token, RawFd>,
    timers: BTreeMap<u64, BTreeSet<RegistrationId>>,
}

impl DarwinReactor<ProcessMonotonicClock> {
    /// Creates one reactor and its monotonic deadline domain.
    ///
    /// # Errors
    ///
    /// Returns the native poll-construction error.
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
    /// Returns the native poll-construction error.
    pub fn with_clock(clock: C) -> Result<Self, DarwinReactorError> {
        Ok(Self {
            poll: Poll::new().map_err(DarwinReactorError::Io)?,
            events: Events::with_capacity(128),
            clock,
            next_token: 0,
            registrations: BTreeMap::new(),
            descriptors: BTreeMap::new(),
            descriptor_tokens: BTreeMap::new(),
            timers: BTreeMap::new(),
        })
    }

    #[must_use]
    pub const fn clock(&self) -> &C {
        &self.clock
    }

    fn allocate_token(&mut self) -> Result<Token, DarwinReactorError> {
        let token = Token(self.next_token);
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
        if let Some(state) = self.descriptors.get_mut(&descriptor) {
            let previous = state.interest();
            registrations_for_direction(state, direction).insert(registration);
            let next = state
                .interest()
                .expect("the new registration makes the set nonempty");
            if previous != Some(next) {
                let mut source = SourceFd(&descriptor);
                if let Err(error) = self
                    .poll
                    .registry()
                    .reregister(&mut source, state.token, next)
                {
                    registrations_for_direction(state, direction).remove(&registration);
                    return Err(DarwinReactorError::Io(error));
                }
            }
        } else {
            let token = self.allocate_token()?;
            let mut state = DescriptorState {
                token,
                readable: BTreeSet::new(),
                writable: BTreeSet::new(),
            };
            registrations_for_direction(&mut state, direction).insert(registration);
            let mut source = SourceFd(&descriptor);
            self.poll
                .registry()
                .register(
                    &mut source,
                    token,
                    state.interest().expect("one registration is present"),
                )
                .map_err(DarwinReactorError::Io)?;
            self.descriptors.insert(descriptor, state);
            self.descriptor_tokens.insert(token, descriptor);
        }
        self.registrations.insert(
            registration,
            RegistrationLocation::Descriptor {
                descriptor,
                direction,
            },
        );
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
        let previous = state.interest();
        registrations_for_direction(state, direction).remove(&registration);
        let next = state.interest();
        let token = state.token;
        let mut source = SourceFd(&descriptor);
        match next {
            Some(next) if previous != Some(next) => {
                let _ = self.poll.registry().reregister(&mut source, token, next);
            }
            Some(_) => {}
            None => {
                self.descriptors.remove(&descriptor);
                self.descriptor_tokens.remove(&token);
                let _ = self.poll.registry().deregister(&mut source);
            }
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

    fn collect_descriptor_events(&self, ready: &mut Vec<RegistrationId>) {
        for event in &self.events {
            let Some(descriptor) = self.descriptor_tokens.get(&event.token()) else {
                continue;
            };
            let Some(state) = self.descriptors.get(descriptor) else {
                continue;
            };
            if event.is_readable() || event.is_read_closed() || event.is_error() {
                ready.extend(state.readable.iter().copied());
            }
            if event.is_writable() || event.is_write_closed() || event.is_error() {
                ready.extend(state.writable.iter().copied());
            }
        }
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
        }
    }

    fn wait(&mut self) -> Result<Box<[RegistrationId]>, Self::Error> {
        self.events.clear();
        let timeout = self.wait_timeout();
        self.poll
            .poll(&mut self.events, timeout)
            .map_err(DarwinReactorError::Io)?;
        let mut ready = Vec::new();
        self.collect_descriptor_events(&mut ready);
        self.collect_expired_timers(&mut ready);
        ready.sort_unstable();
        ready.dedup();
        Ok(ready.into_boxed_slice())
    }
}

fn registrations_for_direction(
    state: &mut DescriptorState,
    direction: ReadinessDirection,
) -> &mut BTreeSet<RegistrationId> {
    match direction {
        ReadinessDirection::Readable => &mut state.readable,
        ReadinessDirection::Writable => &mut state.writable,
    }
}

#[derive(Debug)]
pub enum DarwinReactorError {
    DuplicateRegistration(RegistrationId),
    InvalidDescriptor(u64),
    TokenExhausted,
    Io(std::io::Error),
}

impl fmt::Display for DarwinReactorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Darwin reactor failed: {self:?}")
    }
}

impl std::error::Error for DarwinReactorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::DuplicateRegistration(_) | Self::InvalidDescriptor(_) | Self::TokenExhausted => {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests;
