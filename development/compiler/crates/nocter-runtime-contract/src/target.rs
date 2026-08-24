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

pub const ALLOCATION_FAILURE_ERROR_CODE: &str = "std.mem.out_of_memory";
pub const ALLOCATION_FAILURE_ERROR_MESSAGE: &str = "allocation failed";

/// Closed storage and reporting layout for the compiler built-in failure value.
///
/// Machine planning and instruction lowering consume this schema independently. Neither layer
/// may redeclare the numeric layout from its own target assumptions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeErrorAbiSchema {
    handle_size: u64,
    handle_alignment: u64,
    node_kind_offset: u64,
    node_allocation_size_offset: u64,
    node_cause_offset: u64,
    node_code_length_offset: u64,
    node_message_length_offset: u64,
    node_payload_offset: u64,
    static_leaf_kind: u64,
    owned_leaf_kind: u64,
    owned_context_kind: u64,
    report_separator_offset: u64,
    report_current_node_offset: u64,
    report_root_handle_offset: u64,
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
                    handle_size: 8,
                    handle_alignment: 8,
                    node_kind_offset: 0,
                    node_allocation_size_offset: 8,
                    node_cause_offset: 16,
                    node_code_length_offset: 24,
                    node_message_length_offset: 32,
                    node_payload_offset: 40,
                    static_leaf_kind: 0,
                    owned_leaf_kind: 1,
                    owned_context_kind: 2,
                    report_separator_offset: 0,
                    report_current_node_offset: 8,
                    report_root_handle_offset: 16,
                    report_buffer_size: 24,
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

    /// Encodes the immutable allocation-failure leaf admitted by this runtime ABI.
    ///
    /// # Panics
    ///
    /// Panics only if the closed runtime schema or the two compile-time strings cannot be
    /// represented in host memory. Such a schema is invalid for compiler construction.
    #[must_use]
    pub fn allocation_failure_error_node(self) -> Box<[u8]> {
        let error = self.error();
        let total = error
            .node_payload_offset()
            .checked_add(ALLOCATION_FAILURE_ERROR_CODE.len() as u64)
            .and_then(|size| size.checked_add(ALLOCATION_FAILURE_ERROR_MESSAGE.len() as u64))
            .expect("the closed allocation-failure node size is representable");
        let mut bytes = vec![0; usize::try_from(total).expect("the node size fits host memory")];
        write_word(
            &mut bytes,
            error.node_kind_offset(),
            error.static_leaf_kind(),
            self.endianness(),
        );
        write_word(
            &mut bytes,
            error.node_code_length_offset(),
            ALLOCATION_FAILURE_ERROR_CODE.len() as u64,
            self.endianness(),
        );
        write_word(
            &mut bytes,
            error.node_message_length_offset(),
            ALLOCATION_FAILURE_ERROR_MESSAGE.len() as u64,
            self.endianness(),
        );
        let payload = usize::try_from(error.node_payload_offset())
            .expect("the allocation-failure payload offset fits host memory");
        let message = payload + ALLOCATION_FAILURE_ERROR_CODE.len();
        bytes[payload..message].copy_from_slice(ALLOCATION_FAILURE_ERROR_CODE.as_bytes());
        bytes[message..].copy_from_slice(ALLOCATION_FAILURE_ERROR_MESSAGE.as_bytes());
        bytes.into_boxed_slice()
    }
}

fn write_word(bytes: &mut [u8], offset: u64, value: u64, endianness: RuntimeEndianness) {
    let offset = usize::try_from(offset).expect("runtime schema offsets fit host memory");
    let encoded = match endianness {
        RuntimeEndianness::Little => value.to_le_bytes(),
        RuntimeEndianness::Big => value.to_be_bytes(),
    };
    bytes[offset..offset + encoded.len()].copy_from_slice(&encoded);
}

impl RuntimeErrorAbiSchema {
    #[must_use]
    pub const fn size(self) -> u64 {
        self.handle_size
    }
    #[must_use]
    pub const fn alignment(self) -> u64 {
        self.handle_alignment
    }
    #[must_use]
    pub const fn node_kind_offset(self) -> u64 {
        self.node_kind_offset
    }
    #[must_use]
    pub const fn node_allocation_size_offset(self) -> u64 {
        self.node_allocation_size_offset
    }
    #[must_use]
    pub const fn node_cause_offset(self) -> u64 {
        self.node_cause_offset
    }
    #[must_use]
    pub const fn node_code_length_offset(self) -> u64 {
        self.node_code_length_offset
    }
    #[must_use]
    pub const fn node_message_length_offset(self) -> u64 {
        self.node_message_length_offset
    }
    #[must_use]
    pub const fn node_payload_offset(self) -> u64 {
        self.node_payload_offset
    }
    #[must_use]
    pub const fn static_leaf_kind(self) -> u64 {
        self.static_leaf_kind
    }
    #[must_use]
    pub const fn owned_leaf_kind(self) -> u64 {
        self.owned_leaf_kind
    }
    #[must_use]
    pub const fn owned_context_kind(self) -> u64 {
        self.owned_context_kind
    }
    #[must_use]
    pub const fn report_separator_offset(self) -> u64 {
        self.report_separator_offset
    }
    #[must_use]
    pub const fn report_current_node_offset(self) -> u64 {
        self.report_current_node_offset
    }
    #[must_use]
    pub const fn report_root_handle_offset(self) -> u64 {
        self.report_root_handle_offset
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
        assert_eq!(error.size(), 8);
        assert_eq!(error.alignment(), 8);
        assert_eq!(error.node_kind_offset(), 0);
        assert_eq!(error.node_allocation_size_offset(), 8);
        assert_eq!(error.node_cause_offset(), 16);
        assert_eq!(error.node_code_length_offset(), 24);
        assert_eq!(error.node_message_length_offset(), 32);
        assert_eq!(error.node_payload_offset(), 40);
        assert_eq!(error.static_leaf_kind(), 0);
        assert_eq!(error.owned_leaf_kind(), 1);
        assert_eq!(error.owned_context_kind(), 2);
        assert_eq!(error.report_separator_offset(), 0);
        assert_eq!(error.report_current_node_offset(), 8);
        assert_eq!(error.report_root_handle_offset(), 16);
        assert_eq!(error.report_buffer_size(), 24);
        assert_eq!(error.report_buffer_alignment(), 8);
        let static_error = target.allocation_failure_error_node();
        assert!(static_error.ends_with(b"std.mem.out_of_memoryallocation failed"));
    }
}
