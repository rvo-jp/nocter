/// Closed Darwin `kevent64_s` layout and event vocabulary used by asynchronous runtime adapters.
///
/// This schema is target ABI, not a semantic wait contract. Host FFI and generated instruction
/// lowering validate or encode against this one authority rather than repeating native constants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinEventAbiSchema {
    record_size: u64,
    record_alignment: u64,
    ident_offset: u64,
    filter_offset: u64,
    flags_offset: u64,
    filter_flags_offset: u64,
    data_offset: u64,
    user_data_offset: u64,
    extension_zero_offset: u64,
    extension_one_offset: u64,
    read_filter: i16,
    write_filter: i16,
    process_filter: i16,
    add_flag: u16,
    delete_flag: u16,
    one_shot_flag: u16,
    error_flag: u16,
    process_exit_flag: u32,
}

impl DarwinEventAbiSchema {
    pub const ARM64_DARWIN: Self = Self {
        record_size: 48,
        // The record has a 48-byte packed field sequence, while the arm64 userspace binding
        // over-aligns storage to one word. Generated mappings are page-aligned and use this
        // stronger alignment.
        record_alignment: 8,
        ident_offset: 0,
        filter_offset: 8,
        flags_offset: 10,
        filter_flags_offset: 12,
        data_offset: 16,
        user_data_offset: 24,
        extension_zero_offset: 32,
        extension_one_offset: 40,
        read_filter: -1,
        write_filter: -2,
        process_filter: -5,
        add_flag: 0x0001,
        delete_flag: 0x0002,
        one_shot_flag: 0x0010,
        error_flag: 0x4000,
        process_exit_flag: 0x8000_0000,
    };

    #[must_use]
    pub const fn record_size(self) -> u64 {
        self.record_size
    }

    #[must_use]
    pub const fn record_alignment(self) -> u64 {
        self.record_alignment
    }

    #[must_use]
    pub const fn ident_offset(self) -> u64 {
        self.ident_offset
    }

    #[must_use]
    pub const fn filter_offset(self) -> u64 {
        self.filter_offset
    }

    #[must_use]
    pub const fn flags_offset(self) -> u64 {
        self.flags_offset
    }

    #[must_use]
    pub const fn filter_flags_offset(self) -> u64 {
        self.filter_flags_offset
    }

    #[must_use]
    pub const fn data_offset(self) -> u64 {
        self.data_offset
    }

    #[must_use]
    pub const fn user_data_offset(self) -> u64 {
        self.user_data_offset
    }

    #[must_use]
    pub const fn extension_zero_offset(self) -> u64 {
        self.extension_zero_offset
    }

    #[must_use]
    pub const fn extension_one_offset(self) -> u64 {
        self.extension_one_offset
    }

    #[must_use]
    pub const fn read_filter(self) -> i16 {
        self.read_filter
    }

    #[must_use]
    pub const fn write_filter(self) -> i16 {
        self.write_filter
    }

    #[must_use]
    pub const fn process_filter(self) -> i16 {
        self.process_filter
    }

    #[must_use]
    pub const fn add_flag(self) -> u16 {
        self.add_flag
    }

    #[must_use]
    pub const fn delete_flag(self) -> u16 {
        self.delete_flag
    }

    #[must_use]
    pub const fn one_shot_flag(self) -> u16 {
        self.one_shot_flag
    }

    #[must_use]
    pub const fn error_flag(self) -> u16 {
        self.error_flag
    }

    #[must_use]
    pub const fn process_exit_flag(self) -> u32 {
        self.process_exit_flag
    }
}

#[cfg(test)]
mod tests {
    use super::DarwinEventAbiSchema;

    #[test]
    fn arm64_darwin_event_fields_fit_one_record() {
        let schema = DarwinEventAbiSchema::ARM64_DARWIN;
        assert_eq!(schema.record_size(), 48);
        assert_eq!(schema.record_alignment(), 8);
        assert_eq!(schema.ident_offset(), 0);
        assert_eq!(schema.filter_offset(), 8);
        assert_eq!(schema.flags_offset(), 10);
        assert_eq!(schema.filter_flags_offset(), 12);
        assert_eq!(schema.data_offset(), 16);
        assert_eq!(schema.user_data_offset(), 24);
        assert_eq!(schema.extension_zero_offset(), 32);
        assert_eq!(schema.extension_one_offset(), 40);
        assert!(schema.extension_one_offset() + 8 <= schema.record_size());
    }

    #[test]
    fn arm64_darwin_event_vocabulary_is_disjoint() {
        let schema = DarwinEventAbiSchema::ARM64_DARWIN;
        assert_ne!(schema.read_filter(), schema.write_filter());
        assert_ne!(schema.read_filter(), schema.process_filter());
        assert_ne!(schema.write_filter(), schema.process_filter());
        assert_eq!(schema.add_flag() & schema.delete_flag(), 0);
        assert_eq!(schema.add_flag() & schema.one_shot_flag(), 0);
        assert_eq!(schema.error_flag() & schema.one_shot_flag(), 0);
        assert_ne!(schema.process_exit_flag(), 0);
    }
}
