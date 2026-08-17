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
    argument_register_count: u8,
    direct_value_word_limit: u8,
    direct_result_register_count: u8,
    indirect_result_register: u8,
    pack_pointer_register: u8,
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
                argument_register_count: 8,
                direct_value_word_limit: 2,
                direct_result_register_count: 2,
                indirect_result_register: 8,
                pack_pointer_register: 0,
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
    pub const fn argument_register_count(self) -> u8 {
        self.argument_register_count
    }

    #[must_use]
    pub const fn direct_value_word_limit(self) -> u8 {
        self.direct_value_word_limit
    }

    #[must_use]
    pub const fn direct_result_register_count(self) -> u8 {
        self.direct_result_register_count
    }

    #[must_use]
    pub const fn indirect_result_register(self) -> u8 {
        self.indirect_result_register
    }

    #[must_use]
    pub const fn pack_pointer_register(self) -> u8 {
        self.pack_pointer_register
    }

    #[must_use]
    pub const fn endianness(self) -> MachineEndianness {
        self.endianness
    }
}
