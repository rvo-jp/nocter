use std::fmt;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::time::Duration;

use nocter_runtime_contract::DarwinEventAbiSchema;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EventFilter {
    Readable(i32),
    Writable(i32),
    ProcessExit(i32),
}

impl EventFilter {
    fn native(self, token: u64, flags: u16) -> Result<libc::kevent64_s, EventQueueError> {
        let schema = DarwinEventAbiSchema::ARM64_DARWIN;
        let (ident, filter, filter_flags) = match self {
            Self::Readable(descriptor) => (
                u64::try_from(descriptor)
                    .map_err(|_| EventQueueError::InvalidDescriptor(descriptor))?,
                schema.read_filter(),
                0,
            ),
            Self::Writable(descriptor) => (
                u64::try_from(descriptor)
                    .map_err(|_| EventQueueError::InvalidDescriptor(descriptor))?,
                schema.write_filter(),
                0,
            ),
            Self::ProcessExit(process) => (
                u64::try_from(process)
                    .ok()
                    .filter(|_| process > 0)
                    .ok_or(EventQueueError::InvalidProcess(process))?,
                schema.process_filter(),
                schema.process_exit_flag(),
            ),
        };
        Ok(libc::kevent64_s {
            ident,
            filter,
            flags,
            fflags: filter_flags,
            data: 0,
            udata: token,
            ext: [0; 2],
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeEvent {
    token: u64,
    filter: EventFilter,
    failed: bool,
}

impl NativeEvent {
    #[must_use]
    pub const fn token(self) -> u64 {
        self.token
    }

    #[must_use]
    pub const fn filter(self) -> EventFilter {
        self.filter
    }

    #[must_use]
    pub const fn failed(self) -> bool {
        self.failed
    }
}

pub struct EventQueue {
    descriptor: OwnedFd,
}

impl EventQueue {
    /// Opens a new owned Darwin event queue.
    ///
    /// # Errors
    ///
    /// Returns the operating-system error when the queue cannot be created.
    pub fn new() -> Result<Self, EventQueueError> {
        // SAFETY: `kqueue` has no pointer arguments. A nonnegative return value is a newly owned
        // descriptor, transferred immediately into `OwnedFd` exactly once.
        let descriptor = unsafe { libc::kqueue() };
        if descriptor < 0 {
            return Err(EventQueueError::Io(std::io::Error::last_os_error()));
        }
        // SAFETY: the successful `kqueue` return is a fresh owned descriptor.
        let descriptor = unsafe { OwnedFd::from_raw_fd(descriptor) };
        Ok(Self { descriptor })
    }

    /// Adds one descriptor direction or process-exit observation with an opaque token.
    ///
    /// # Errors
    ///
    /// Rejects invalid native subjects and returns native registration failures.
    pub fn register(&self, filter: EventFilter, token: u64) -> Result<(), EventQueueError> {
        let schema = DarwinEventAbiSchema::ARM64_DARWIN;
        let mut flags = schema.add_flag();
        if matches!(filter, EventFilter::ProcessExit(_)) {
            flags |= schema.one_shot_flag();
        }
        self.apply(filter.native(token, flags)?)
    }

    /// Removes one native observation.
    ///
    /// # Errors
    ///
    /// Rejects invalid native subjects and returns native deregistration failures.
    pub fn deregister(&self, filter: EventFilter) -> Result<(), EventQueueError> {
        self.apply(filter.native(0, DarwinEventAbiSchema::ARM64_DARWIN.delete_flag())?)
    }

    /// Waits for native observations or the relative timeout.
    ///
    /// # Errors
    ///
    /// Rejects capacities or timeouts outside the native ABI and returns native wait or event
    /// decoding failures. Interrupted waits are returned to the absolute-deadline owner.
    pub fn wait(
        &self,
        maximum_events: usize,
        timeout: Option<Duration>,
    ) -> Result<Box<[NativeEvent]>, EventQueueError> {
        let capacity = maximum_events.max(1);
        let native_capacity = i32::try_from(capacity)
            .map_err(|_| EventQueueError::EventCapacityOverflow(maximum_events))?;
        let mut events = vec![zero_event(); capacity];
        let timeout = timeout.map(duration_to_timespec).transpose()?;
        // SAFETY: the queue descriptor is owned by `self`; the change list is empty; `events`
        // is initialized writable storage for exactly `native_capacity` records; and the
        // optional timespec remains live for the duration of the call.
        let count = unsafe {
            libc::kevent64(
                self.descriptor.as_raw_fd(),
                std::ptr::null(),
                0,
                events.as_mut_ptr(),
                native_capacity,
                0,
                timeout
                    .as_ref()
                    .map_or(std::ptr::null(), std::ptr::from_ref),
            )
        };
        if count < 0 {
            // The owner of the absolute deadline decides whether and how to retry interruption.
            // Retrying this relative timeout here would silently restart elapsed timer duration.
            return Err(EventQueueError::Io(std::io::Error::last_os_error()));
        }
        events.truncate(
            usize::try_from(count).map_err(|_| EventQueueError::InvalidNativeEventCount(count))?,
        );
        events
            .into_iter()
            .map(decode_event)
            .collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice)
    }

    fn apply(&self, change: libc::kevent64_s) -> Result<(), EventQueueError> {
        loop {
            // SAFETY: the queue descriptor is owned by `self`; `change` is one fully initialized
            // record; no output buffer is supplied; and all pointers remain valid for the call.
            let result = unsafe {
                libc::kevent64(
                    self.descriptor.as_raw_fd(),
                    std::ptr::from_ref(&change),
                    1,
                    std::ptr::null_mut(),
                    0,
                    0,
                    std::ptr::null(),
                )
            };
            if result >= 0 {
                return Ok(());
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(EventQueueError::Io(error));
            }
        }
    }
}

fn decode_event(event: libc::kevent64_s) -> Result<NativeEvent, EventQueueError> {
    let schema = DarwinEventAbiSchema::ARM64_DARWIN;
    let subject = i32::try_from(event.ident)
        .map_err(|_| EventQueueError::InvalidNativeSubject(event.ident))?;
    let filter = if event.filter == schema.read_filter() {
        EventFilter::Readable(subject)
    } else if event.filter == schema.write_filter() {
        EventFilter::Writable(subject)
    } else if event.filter == schema.process_filter() {
        EventFilter::ProcessExit(subject)
    } else {
        return Err(EventQueueError::UnknownNativeFilter(event.filter));
    };
    Ok(NativeEvent {
        token: event.udata,
        filter,
        failed: event.flags & schema.error_flag() != 0,
    })
}

fn duration_to_timespec(duration: Duration) -> Result<libc::timespec, EventQueueError> {
    Ok(libc::timespec {
        tv_sec: duration
            .as_secs()
            .try_into()
            .map_err(|_| EventQueueError::TimeoutOverflow(duration))?,
        tv_nsec: duration.subsec_nanos().into(),
    })
}

const fn zero_event() -> libc::kevent64_s {
    libc::kevent64_s {
        ident: 0,
        filter: 0,
        flags: 0,
        fflags: 0,
        data: 0,
        udata: 0,
        ext: [0; 2],
    }
}

#[derive(Debug)]
pub enum EventQueueError {
    EventCapacityOverflow(usize),
    TimeoutOverflow(Duration),
    InvalidDescriptor(i32),
    InvalidProcess(i32),
    InvalidNativeEventCount(i32),
    InvalidNativeSubject(u64),
    UnknownNativeFilter(i16),
    Io(std::io::Error),
}

impl fmt::Display for EventQueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Darwin event queue failed: {self:?}")
    }
}

impl std::error::Error for EventQueueError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::EventCapacityOverflow(_)
            | Self::TimeoutOverflow(_)
            | Self::InvalidDescriptor(_)
            | Self::InvalidProcess(_)
            | Self::InvalidNativeEventCount(_)
            | Self::InvalidNativeSubject(_)
            | Self::UnknownNativeFilter(_) => None,
        }
    }
}

impl EventQueueError {
    #[must_use]
    pub fn is_interrupted(&self) -> bool {
        matches!(Self::source_io(self), Some(error) if error.kind() == std::io::ErrorKind::Interrupted)
    }

    /// Whether a native process observation lost the race with process exit before registration.
    #[must_use]
    pub fn is_missing_subject(&self) -> bool {
        matches!(
            Self::source_io(self).and_then(std::io::Error::raw_os_error),
            Some(libc::ESRCH)
        )
    }

    fn source_io(&self) -> Option<&std::io::Error> {
        match self {
            Self::Io(error) => Some(error),
            Self::EventCapacityOverflow(_)
            | Self::TimeoutOverflow(_)
            | Self::InvalidDescriptor(_)
            | Self::InvalidProcess(_)
            | Self::InvalidNativeEventCount(_)
            | Self::InvalidNativeSubject(_)
            | Self::UnknownNativeFilter(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::mem::{align_of, offset_of, size_of};
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;
    use std::process::Command;

    use super::*;

    #[test]
    fn native_record_matches_the_closed_runtime_schema() {
        let schema = DarwinEventAbiSchema::ARM64_DARWIN;
        assert_eq!(size_of::<libc::kevent64_s>() as u64, schema.record_size());
        assert_eq!(
            align_of::<libc::kevent64_s>() as u64,
            schema.record_alignment()
        );
        assert_eq!(
            offset_of!(libc::kevent64_s, ident) as u64,
            schema.ident_offset()
        );
        assert_eq!(
            offset_of!(libc::kevent64_s, filter) as u64,
            schema.filter_offset()
        );
        assert_eq!(
            offset_of!(libc::kevent64_s, flags) as u64,
            schema.flags_offset()
        );
        assert_eq!(
            offset_of!(libc::kevent64_s, fflags) as u64,
            schema.filter_flags_offset()
        );
        assert_eq!(
            offset_of!(libc::kevent64_s, data) as u64,
            schema.data_offset()
        );
        assert_eq!(
            offset_of!(libc::kevent64_s, udata) as u64,
            schema.user_data_offset()
        );
        assert_eq!(
            offset_of!(libc::kevent64_s, ext) as u64,
            schema.extension_zero_offset()
        );
        assert_eq!(libc::EVFILT_READ, schema.read_filter());
        assert_eq!(libc::EVFILT_WRITE, schema.write_filter());
        assert_eq!(libc::EVFILT_PROC, schema.process_filter());
        assert_eq!(libc::EV_ADD, schema.add_flag());
        assert_eq!(libc::EV_DELETE, schema.delete_flag());
        assert_eq!(libc::EV_ONESHOT, schema.one_shot_flag());
        assert_eq!(libc::EV_ERROR, schema.error_flag());
        assert_eq!(libc::NOTE_EXIT, schema.process_exit_flag());
    }

    #[test]
    fn descriptor_registration_delivers_its_opaque_token() {
        let (reader, mut writer) = UnixStream::pair().unwrap();
        let queue = EventQueue::new().unwrap();
        let filter = EventFilter::Readable(reader.as_raw_fd());
        queue.register(filter, 91).unwrap();
        std::io::Write::write_all(&mut writer, b"ready").unwrap();
        let events = queue.wait(1, Some(Duration::from_secs(1))).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].token(), 91);
        assert_eq!(events[0].filter(), filter);
        assert!(!events[0].failed());
    }

    #[test]
    fn exit_before_registration_is_reported_without_reaping_the_child() {
        let mut child = Command::new("/usr/bin/true").spawn().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        let queue = EventQueue::new().unwrap();
        let filter = EventFilter::ProcessExit(i32::try_from(child.id()).unwrap());
        let error = queue.register(filter, 92).unwrap_err();
        assert!(error.is_missing_subject());
        assert!(child.wait().unwrap().success());
    }
}
