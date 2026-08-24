use nocter_runtime_contract::{RuntimeAbiIdentity, RuntimeAbiSchema};

pub use nocter_runtime_contract::RuntimeEndianness as MachineEndianness;

/// Closed layout facts selected by the toolchain ABI identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineTarget {
    schema: RuntimeAbiSchema,
}

impl MachineTarget {
    pub(crate) const fn select(abi: RuntimeAbiIdentity) -> Self {
        Self {
            schema: abi.schema(),
        }
    }

    #[must_use]
    pub const fn word_size(self) -> u64 {
        self.schema.word_size()
    }

    #[must_use]
    pub const fn pointer_size(self) -> u64 {
        self.schema.pointer_size()
    }

    #[must_use]
    pub const fn pointer_alignment(self) -> u64 {
        self.schema.pointer_alignment()
    }

    #[must_use]
    pub const fn stack_alignment(self) -> u64 {
        self.schema.stack_alignment()
    }

    #[must_use]
    pub const fn argument_register_count(self) -> u8 {
        self.schema.argument_register_count()
    }

    #[must_use]
    pub const fn direct_value_word_limit(self) -> u8 {
        self.schema.direct_value_word_limit()
    }

    #[must_use]
    pub const fn direct_result_register_count(self) -> u8 {
        self.schema.direct_result_register_count()
    }

    #[must_use]
    pub const fn indirect_result_register(self) -> u8 {
        self.schema.indirect_result_register()
    }

    #[must_use]
    pub const fn pack_pointer_register(self) -> u8 {
        self.schema.pack_pointer_register()
    }

    #[must_use]
    pub const fn endianness(self) -> MachineEndianness {
        self.schema.endianness()
    }

    #[must_use]
    pub const fn error(self) -> nocter_runtime_contract::RuntimeErrorAbiSchema {
        self.schema.error()
    }
}
