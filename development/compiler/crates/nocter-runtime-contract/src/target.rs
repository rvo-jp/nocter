/// One compiler-granted runtime ABI. Consumers select one schema from this identity and cannot
/// infer target capability from source or package spellings.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RuntimeAbiIdentity {
    Arm64DarwinV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeEndianness {
    Little,
    Big,
}

/// Complete numeric ABI authority shared by machine planning and instruction lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAbiSchema {
    word_size: u64,
    pointer_size: u64,
    pointer_alignment: u64,
    stack_alignment: u64,
    argument_register_count: u8,
    direct_value_word_limit: u8,
    direct_result_register_count: u8,
    indirect_result_register: u8,
    pack_pointer_register: u8,
    endianness: RuntimeEndianness,
}

impl RuntimeAbiIdentity {
    #[must_use]
    pub const fn schema(self) -> RuntimeAbiSchema {
        match self {
            Self::Arm64DarwinV1 => RuntimeAbiSchema {
                word_size: 8,
                pointer_size: 8,
                pointer_alignment: 8,
                stack_alignment: 16,
                argument_register_count: 8,
                direct_value_word_limit: 2,
                direct_result_register_count: 2,
                indirect_result_register: 8,
                pack_pointer_register: 0,
                endianness: RuntimeEndianness::Little,
            },
        }
    }
}

impl RuntimeAbiSchema {
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
    pub const fn endianness(self) -> RuntimeEndianness {
        self.endianness
    }
}
