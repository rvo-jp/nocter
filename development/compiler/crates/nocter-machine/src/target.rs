use nocter_target_program::TargetAbiIdentity;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineEndianness {
    Little,
    Big,
}

/// Closed layout facts selected by the toolchain ABI identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineTarget {
    word_size: u64,
    pointer_size: u64,
    pointer_alignment: u64,
    stack_alignment: u64,
    endianness: MachineEndianness,
}

impl MachineTarget {
    pub(crate) const fn select(abi: TargetAbiIdentity) -> Self {
        match abi {
            TargetAbiIdentity::Arm64DarwinV1 => Self {
                word_size: 8,
                pointer_size: 8,
                pointer_alignment: 8,
                stack_alignment: 16,
                endianness: MachineEndianness::Little,
            },
        }
    }

    #[must_use]
    pub const fn word_size(self) -> u64 {
        self.word_size
    }

    #[must_use]
    pub const fn pointer_size(self) -> u64 {
        self.pointer_size
    }

    #[must_use]
    pub const fn pointer_alignment(self) -> u64 {
        self.pointer_alignment
    }

    #[must_use]
    pub const fn stack_alignment(self) -> u64 {
        self.stack_alignment
    }

    #[must_use]
    pub const fn endianness(self) -> MachineEndianness {
        self.endianness
    }
}
