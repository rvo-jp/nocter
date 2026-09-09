use crate::{Arm64CodeBuilder, Arm64DataSize, Arm64Instruction, Arm64NocterAbi, Arm64Register};

const SUPERVISOR_CALL_IMMEDIATE: u16 = 0x80;

/// A compiler-owned Darwin kernel entry whose number is fixed by the target ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DarwinSystemCall {
    Exit,
    Write,
    MemoryUnmap,
    MemoryMap,
    Poll,
}

impl DarwinSystemCall {
    const fn number(self) -> u64 {
        match self {
            Self::Exit => 1,
            Self::Write => 0x0200_0004,
            Self::MemoryUnmap => 0x0200_0049,
            Self::MemoryMap => 0x0200_00c5,
            Self::Poll => 0x0200_00e6,
        }
    }
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

/// Darwin's native `pollfd` layout and value domain.
pub(crate) struct DarwinPollAbi;

impl DarwinPollAbi {
    pub(crate) const INTERRUPTED_ERROR: u64 = 4;
    pub(crate) const INPUT_EVENT: u64 = 1;
    pub(crate) const OUTPUT_EVENT: u64 = 4;
    pub(crate) const DESCRIPTOR_SIZE: u64 = 8;
    pub(crate) const EVENTS_OFFSET: u32 = 4;
    pub(crate) const RETURNED_EVENTS_OFFSET: u32 = 6;
    pub(crate) const MAX_DESCRIPTOR: u64 = i32::MAX as u64;
    pub(crate) const MAX_COUNT: u64 = u32::MAX as u64;
}

/// Stable Darwin process descriptors used by compiler-owned diagnostics.
pub(crate) struct DarwinProcessAbi;

impl DarwinProcessAbi {
    pub(crate) const STANDARD_ERROR: u64 = 2;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_layout_fields_fit_the_native_record() {
        assert!(u64::from(DarwinPollAbi::EVENTS_OFFSET) + 2 <= DarwinPollAbi::DESCRIPTOR_SIZE);
        assert!(
            u64::from(DarwinPollAbi::RETURNED_EVENTS_OFFSET) + 2 <= DarwinPollAbi::DESCRIPTOR_SIZE
        );
    }

    #[test]
    fn compiler_owned_system_calls_have_distinct_numbers() {
        let calls = [
            DarwinSystemCall::Exit,
            DarwinSystemCall::Write,
            DarwinSystemCall::MemoryUnmap,
            DarwinSystemCall::MemoryMap,
            DarwinSystemCall::Poll,
        ];
        for (index, call) in calls.iter().enumerate() {
            assert!(
                calls[index + 1..]
                    .iter()
                    .all(|other| call.number() != other.number())
            );
        }
    }
}
