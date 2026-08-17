use crate::Arm64Register;

/// The closed register role assigned by the Nocter ARM64-Darwin ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64AbiRegisterRole {
    ArgumentAndResult,
    IndirectResult,
    CallerSaved,
    CompilerScratch,
    Reserved,
    CalleeSaved,
    FramePointer,
    Link,
}

/// Target constants and register classification shared by selection, allocation, and frames.
pub struct Arm64NocterAbi;

impl Arm64NocterAbi {
    pub const WORD_SIZE: u64 = 8;
    pub const STACK_ALIGNMENT: u64 = 16;
    pub const ARGUMENT_REGISTER_COUNT: u8 = 8;

    #[must_use]
    pub const fn argument_register(index: u8) -> Option<Arm64Register> {
        if index < Self::ARGUMENT_REGISTER_COUNT {
            Arm64Register::new(index)
        } else {
            None
        }
    }

    #[must_use]
    pub const fn indirect_result_register() -> Arm64Register {
        match Arm64Register::new(8) {
            Some(register) => register,
            None => unreachable!(),
        }
    }

    #[must_use]
    pub const fn role(register: Arm64Register) -> Arm64AbiRegisterRole {
        match register.number() {
            0..=7 => Arm64AbiRegisterRole::ArgumentAndResult,
            8 => Arm64AbiRegisterRole::IndirectResult,
            9..=15 => Arm64AbiRegisterRole::CallerSaved,
            16..=17 => Arm64AbiRegisterRole::CompilerScratch,
            18 => Arm64AbiRegisterRole::Reserved,
            19..=28 => Arm64AbiRegisterRole::CalleeSaved,
            29 => Arm64AbiRegisterRole::FramePointer,
            30 => Arm64AbiRegisterRole::Link,
            _ => unreachable!(),
        }
    }

    /// Registers available to general virtual-register allocation. Compiler scratch registers are
    /// deliberately excluded so late address and fixup materialization always has reserved space.
    #[must_use]
    pub const fn is_allocatable(register: Arm64Register) -> bool {
        matches!(
            Self::role(register),
            Arm64AbiRegisterRole::ArgumentAndResult
                | Arm64AbiRegisterRole::IndirectResult
                | Arm64AbiRegisterRole::CallerSaved
                | Arm64AbiRegisterRole::CalleeSaved
        )
    }

    #[must_use]
    pub const fn is_callee_saved(register: Arm64Register) -> bool {
        matches!(Self::role(register), Arm64AbiRegisterRole::CalleeSaved)
    }
}
