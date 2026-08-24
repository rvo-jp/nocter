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

/// Closed storage and reporting layout for the compiler built-in failure value.
///
/// Machine planning and instruction lowering consume this schema independently. Neither layer
/// may redeclare the numeric layout from its own target assumptions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeErrorAbiSchema {
    size: u64,
    alignment: u64,
    code_offset: u64,
    message_offset: u64,
    view_pointer_offset: u64,
    view_length_offset: u64,
    report_buffer_size: u64,
    report_buffer_alignment: u64,
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
    error: RuntimeErrorAbiSchema,
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
                error: RuntimeErrorAbiSchema {
                    size: 32,
                    alignment: 8,
                    code_offset: 0,
                    message_offset: 16,
                    view_pointer_offset: 0,
                    view_length_offset: 8,
                    report_buffer_size: 8,
                    report_buffer_alignment: 8,
                },
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
    #[must_use]
    pub const fn error(self) -> RuntimeErrorAbiSchema {
        self.error
    }
}

impl RuntimeErrorAbiSchema {
    #[must_use]
    pub const fn size(self) -> u64 {
        self.size
    }
    #[must_use]
    pub const fn alignment(self) -> u64 {
        self.alignment
    }
    #[must_use]
    pub const fn code_offset(self) -> u64 {
        self.code_offset
    }
    #[must_use]
    pub const fn message_offset(self) -> u64 {
        self.message_offset
    }
    #[must_use]
    pub const fn view_pointer_offset(self) -> u64 {
        self.view_pointer_offset
    }
    #[must_use]
    pub const fn view_length_offset(self) -> u64 {
        self.view_length_offset
    }
    #[must_use]
    pub const fn report_buffer_size(self) -> u64 {
        self.report_buffer_size
    }
    #[must_use]
    pub const fn report_buffer_alignment(self) -> u64 {
        self.report_buffer_alignment
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeAbiIdentity;

    #[test]
    fn one_runtime_schema_owns_the_complete_error_payload_contract() {
        let target = RuntimeAbiIdentity::Arm64DarwinV1.schema();
        let error = target.error();
        assert_eq!(error.size(), 32);
        assert_eq!(error.alignment(), 8);
        assert_eq!(error.code_offset(), 0);
        assert_eq!(error.message_offset(), 16);
        assert_eq!(error.view_pointer_offset(), 0);
        assert_eq!(error.view_length_offset(), 8);
        assert_eq!(error.report_buffer_size(), 8);
        assert_eq!(error.report_buffer_alignment(), 8);
    }
}
