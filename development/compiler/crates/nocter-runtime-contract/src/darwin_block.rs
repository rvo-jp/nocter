/// The fixed Darwin Blocks ABI subset admitted by compiler-owned native adapters.
///
/// Nocter source cannot construct or inspect this representation. The schema exists so target
/// materialization and native-adapter tests share one numeric authority instead of reproducing the
/// platform record layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinBlockAbiSchema {
    pointer_size: u64,
    pointer_alignment: u64,
    isa_offset: u64,
    flags_offset: u64,
    reserved_offset: u64,
    invoke_offset: u64,
    descriptor_offset: u64,
    captures_offset: u64,
    header_size: u64,
    descriptor_reserved_offset: u64,
    descriptor_size_offset: u64,
    descriptor_signature_offset: u64,
    descriptor_layout_offset: u64,
    descriptor_size: u64,
    has_signature_flag: u32,
}

impl DarwinBlockAbiSchema {
    /// The ABI observed for the supported ARM64 Darwin target.
    pub const ARM64_DARWIN: Self = Self {
        pointer_size: 8,
        pointer_alignment: 8,
        isa_offset: 0,
        flags_offset: 8,
        reserved_offset: 12,
        invoke_offset: 16,
        descriptor_offset: 24,
        captures_offset: 32,
        header_size: 32,
        descriptor_reserved_offset: 0,
        descriptor_size_offset: 8,
        descriptor_signature_offset: 16,
        descriptor_layout_offset: 24,
        descriptor_size: 32,
        has_signature_flag: 1 << 30,
    };

    #[must_use]
    pub const fn pointer_size(self) -> u64 {
        self.pointer_size
    }

    #[must_use]
    pub const fn pointer_alignment(self) -> u64 {
        self.pointer_alignment
    }

    #[must_use]
    pub const fn isa_offset(self) -> u64 {
        self.isa_offset
    }

    #[must_use]
    pub const fn flags_offset(self) -> u64 {
        self.flags_offset
    }

    #[must_use]
    pub const fn reserved_offset(self) -> u64 {
        self.reserved_offset
    }

    #[must_use]
    pub const fn invoke_offset(self) -> u64 {
        self.invoke_offset
    }

    #[must_use]
    pub const fn descriptor_offset(self) -> u64 {
        self.descriptor_offset
    }

    #[must_use]
    pub const fn captures_offset(self) -> u64 {
        self.captures_offset
    }

    #[must_use]
    pub const fn header_size(self) -> u64 {
        self.header_size
    }

    #[must_use]
    pub const fn descriptor_reserved_offset(self) -> u64 {
        self.descriptor_reserved_offset
    }

    #[must_use]
    pub const fn descriptor_size_offset(self) -> u64 {
        self.descriptor_size_offset
    }

    #[must_use]
    pub const fn descriptor_signature_offset(self) -> u64 {
        self.descriptor_signature_offset
    }

    #[must_use]
    pub const fn descriptor_layout_offset(self) -> u64 {
        self.descriptor_layout_offset
    }

    #[must_use]
    pub const fn descriptor_size(self) -> u64 {
        self.descriptor_size
    }

    #[must_use]
    pub const fn has_signature_flag(self) -> u32 {
        self.has_signature_flag
    }

    /// Computes the complete literal size for pointer-sized captures.
    ///
    /// # Errors
    ///
    /// Rejects arithmetic overflow.
    #[must_use]
    pub const fn literal_size(self, capture_count: u64) -> Option<u64> {
        let Some(captures) = capture_count.checked_mul(self.pointer_size) else {
            return None;
        };
        self.header_size.checked_add(captures)
    }
}

#[cfg(test)]
mod tests {
    use super::DarwinBlockAbiSchema;

    #[test]
    fn arm64_darwin_layout_matches_the_closed_native_record() {
        let schema = DarwinBlockAbiSchema::ARM64_DARWIN;
        assert_eq!(schema.pointer_size(), 8);
        assert_eq!(schema.pointer_alignment(), 8);
        assert_eq!(schema.isa_offset(), 0);
        assert_eq!(schema.flags_offset(), 8);
        assert_eq!(schema.reserved_offset(), 12);
        assert_eq!(schema.invoke_offset(), 16);
        assert_eq!(schema.descriptor_offset(), 24);
        assert_eq!(schema.captures_offset(), 32);
        assert_eq!(schema.literal_size(1), Some(40));
        assert_eq!(schema.descriptor_signature_offset(), 16);
        assert_eq!(schema.descriptor_size(), 32);
        assert_eq!(schema.has_signature_flag(), 0x4000_0000);
    }
}
