use crate::{Arm64CodeBuilder, Arm64DataSize, Arm64Instruction, Arm64NocterAbi, Arm64Register};

const SUPERVISOR_CALL_IMMEDIATE: u16 = 0x80;

/// A compiler-owned Darwin kernel entry whose number is fixed by the target ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DarwinSystemCall {
    Exit,
    Write,
    Close,
    Wait4,
    Kill,
    Socket,
    Connect,
    Bind,
    SetSocketOption,
    GetSocketOption,
    SendTo,
    ReceiveMessage,
    GetSocketName,
    GetPeerName,
    Fcntl,
    GetEntropy,
    MemoryUnmap,
    MemoryMap,
    Kqueue,
    Kevent64,
    Select,
    GetTimeOfDay,
}

impl DarwinSystemCall {
    const fn number(self) -> u64 {
        match self {
            Self::Exit => 1,
            Self::Write => 0x0200_0004,
            Self::Close => 0x0200_0006,
            Self::Wait4 => 0x0200_0007,
            Self::Kill => 0x0200_0025,
            Self::Socket => 0x0200_0061,
            Self::Connect => 0x0200_0062,
            Self::Bind => 0x0200_0068,
            Self::SetSocketOption => 0x0200_0069,
            Self::GetSocketOption => 0x0200_0076,
            Self::SendTo => 0x0200_0085,
            Self::ReceiveMessage => 0x0200_001b,
            Self::GetSocketName => 0x0200_0020,
            Self::GetPeerName => 0x0200_001f,
            Self::Fcntl => 0x0200_005c,
            Self::GetEntropy => 0x0200_01f4,
            Self::MemoryUnmap => 0x0200_0049,
            Self::MemoryMap => 0x0200_00c5,
            Self::Kqueue => 0x0200_016a,
            Self::Kevent64 => 0x0200_0171,
            Self::Select => 0x0200_005d,
            Self::GetTimeOfDay => 0x0200_0074,
        }
    }
}

/// Fixed Darwin socket and descriptor values used by the closed datagram boundary.
pub(crate) struct DarwinDatagramAbi;

impl DarwinDatagramAbi {
    pub(crate) const IPV4_FAMILY: u64 = 2;
    pub(crate) const IPV6_FAMILY: u64 = 30;
    pub(crate) const DATAGRAM_SOCKET: u64 = 2;
    pub(crate) const SOCKET_LEVEL: u64 = 0xffff;
    pub(crate) const SOCKET_ERROR: u64 = 0x1007;
    pub(crate) const IPV6_LEVEL: u64 = 41;
    pub(crate) const IPV6_ONLY: u64 = 27;
    pub(crate) const INTEGER_OPTION_SIZE: u64 = 4;
    pub(crate) const SET_DESCRIPTOR_FLAGS: u64 = 2;
    pub(crate) const SET_STATUS_FLAGS: u64 = 4;
    pub(crate) const SET_NO_SIGPIPE: u64 = 73;
    pub(crate) const CLOSE_ON_EXEC: u64 = 1;
    pub(crate) const NONBLOCKING: u64 = 4;
}

/// Emits a compiler-selected Darwin system call after its arguments have been prepared.
pub(crate) fn emit_system_call(code: &mut Arm64CodeBuilder, call: DarwinSystemCall) {
    crate::frame_access::load_immediate(
        code,
        system_call_register(),
        call.number(),
        Arm64DataSize::Bits64,
    );
    emit_loaded_system_call(code);
}

/// Emits a Darwin system call whose number is already present in the kernel ABI register.
///
/// Only the source-level raw syscall primitive uses this form. Compiler-owned calls use
/// [`emit_system_call`] so their numeric identity remains private to this module.
pub(crate) fn emit_loaded_system_call(code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::SupervisorCall {
        immediate: SUPERVISOR_CALL_IMMEDIATE,
    });
}

#[must_use]
pub(crate) fn system_call_register() -> Arm64Register {
    Arm64NocterAbi::compiler_scratch_register(0)
        .expect("the ABI reserves a Darwin syscall-number register")
}

/// Darwin constants used to construct a private anonymous memory mapping.
pub(crate) struct DarwinMemoryMapAbi;

impl DarwinMemoryMapAbi {
    pub(crate) const READ_WRITE_PROTECTION: u64 = 3;
    pub(crate) const PRIVATE_ANONYMOUS_FLAGS: u64 = 0x1002;
    pub(crate) const ANONYMOUS_DESCRIPTOR: u64 = u64::MAX;
}

/// Darwin event-queue limits and timeout layout not contained in one event record.
pub(crate) struct DarwinEventQueueAbi;

impl DarwinEventQueueAbi {
    pub(crate) const MAX_SUBJECT: u64 = i32::MAX as u64;
    pub(crate) const MAX_EVENT_COUNT: u64 = i32::MAX as u64;
    pub(crate) const TIMESPEC_SIZE: u64 = 16;
    pub(crate) const TIMESPEC_SECONDS_OFFSET: u64 = 0;
    pub(crate) const TIMESPEC_NANOSECONDS_OFFSET: u64 = 8;
}

/// Darwin errno values shared by compiler-owned target services.
pub(crate) struct DarwinErrorAbi;

impl DarwinErrorAbi {
    pub(crate) const INTERRUPTED: u64 = 4;
    pub(crate) const MISSING_PROCESS: u64 = 3;
}

/// Darwin's five-argument `select` timeout ABI.
pub(crate) struct DarwinSelectAbi;

impl DarwinSelectAbi {
    pub(crate) const TIMEOUT_ARGUMENT_INDEX: u8 = 4;
    pub(crate) const TIMEVAL_SIZE: u64 = 16;
    pub(crate) const SECONDS_OFFSET: u64 = 0;
    pub(crate) const MICROSECONDS_OFFSET: u64 = 8;
}

/// Darwin's native `timeval` result layout.
pub(crate) struct DarwinTimevalAbi;

impl DarwinTimevalAbi {
    pub(crate) const SIZE: u64 = 16;
    pub(crate) const SECONDS_OFFSET: u64 = 0;
    /// Offset of a signed 32-bit count followed by four padding bytes. The backend zeroes the
    /// complete word before the syscall and therefore publishes it as a validated `u64` lane.
    pub(crate) const MICROSECONDS_OFFSET: u64 = 8;
}

/// Stable Darwin process descriptors used by compiler-owned diagnostics.
pub(crate) struct DarwinProcessAbi;

impl DarwinProcessAbi {
    pub(crate) const STANDARD_ERROR: u64 = 2;
    pub(crate) const KILL_SIGNAL: u64 = 9;
    pub(crate) const MAX_PROCESS_ID: u64 = i32::MAX as u64;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_queue_timeout_is_two_words() {
        assert_eq!(DarwinEventQueueAbi::TIMESPEC_SIZE, 16);
        assert_eq!(DarwinEventQueueAbi::TIMESPEC_SECONDS_OFFSET, 0);
        assert_eq!(DarwinEventQueueAbi::TIMESPEC_NANOSECONDS_OFFSET, 8);
    }

    #[test]
    fn compiler_owned_system_calls_have_distinct_numbers() {
        let calls = [
            DarwinSystemCall::Exit,
            DarwinSystemCall::Write,
            DarwinSystemCall::Close,
            DarwinSystemCall::Wait4,
            DarwinSystemCall::Kill,
            DarwinSystemCall::Socket,
            DarwinSystemCall::Connect,
            DarwinSystemCall::Bind,
            DarwinSystemCall::SetSocketOption,
            DarwinSystemCall::GetSocketOption,
            DarwinSystemCall::SendTo,
            DarwinSystemCall::ReceiveMessage,
            DarwinSystemCall::GetSocketName,
            DarwinSystemCall::GetPeerName,
            DarwinSystemCall::Fcntl,
            DarwinSystemCall::GetEntropy,
            DarwinSystemCall::MemoryUnmap,
            DarwinSystemCall::MemoryMap,
            DarwinSystemCall::Kqueue,
            DarwinSystemCall::Kevent64,
            DarwinSystemCall::Select,
            DarwinSystemCall::GetTimeOfDay,
        ];
        for (index, call) in calls.iter().enumerate() {
            assert!(
                calls[index + 1..]
                    .iter()
                    .all(|other| call.number() != other.number())
            );
        }
    }

    #[test]
    fn select_timeout_layout_names_the_fifth_argument() {
        assert_eq!(DarwinSelectAbi::TIMEOUT_ARGUMENT_INDEX, 4);
        assert_eq!(DarwinSelectAbi::TIMEVAL_SIZE, 16);
        assert_eq!(DarwinSelectAbi::SECONDS_OFFSET, 0);
        assert_eq!(DarwinSelectAbi::MICROSECONDS_OFFSET, 8);
    }

    #[test]
    fn timeval_layout_is_two_words() {
        assert_eq!(DarwinTimevalAbi::SIZE, 16);
        assert_eq!(DarwinTimevalAbi::SECONDS_OFFSET, 0);
        assert_eq!(DarwinTimevalAbi::MICROSECONDS_OFFSET, 8);
    }
}
