//! Canonical built-in integer semantics shared by ABI, IR, and native code generation.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum IntegerType {
    I8,
    I16,
    I32,
    I64,
    Isize,
    U8,
    U16,
    U32,
    U64,
    Usize,
}

impl IntegerType {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 10] = [
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::Isize,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::Usize,
    ];

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "i8" => Self::I8,
            "i16" => Self::I16,
            "i32" => Self::I32,
            "i64" => Self::I64,
            "isize" => Self::Isize,
            "u8" => Self::U8,
            "u16" => Self::U16,
            "u32" => Self::U32,
            "u64" => Self::U64,
            "usize" => Self::Usize,
            _ => return None,
        })
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::Isize => "isize",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::Usize => "usize",
        }
    }

    pub(crate) const fn bit_width(self) -> u32 {
        match self {
            Self::I8 | Self::U8 => 8,
            Self::I16 | Self::U16 => 16,
            Self::I32 | Self::U32 => 32,
            Self::I64 | Self::Isize | Self::U64 | Self::Usize => 64,
        }
    }

    pub(crate) const fn is_signed(self) -> bool {
        matches!(
            self,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 | Self::Isize
        )
    }

    pub(crate) const fn mask(self) -> u64 {
        match self.bit_width() {
            8 => u8::MAX as u64,
            16 => u16::MAX as u64,
            32 => u32::MAX as u64,
            64 => u64::MAX,
            _ => unreachable!(),
        }
    }

    /// Converts an in-range source integer magnitude into its canonical ABI word.
    /// Signed values are sign-extended and unsigned values are zero-extended.
    pub(crate) const fn canonical_word(self, bits: u64) -> u64 {
        let bits = bits & self.mask();
        if !self.is_signed() || self.bit_width() == 64 {
            return bits;
        }
        let sign_bit = 1_u64 << (self.bit_width() - 1);
        if bits & sign_bit == 0 {
            bits
        } else {
            bits | !self.mask()
        }
    }

    /// Encodes an in-range negative literal magnitude without executing a
    /// runtime subtraction that would overflow for the signed minimum value.
    pub(crate) const fn negated_magnitude_word(self, magnitude: u64) -> u64 {
        let bits = 0_u64.wrapping_sub(magnitude) & self.mask();
        self.canonical_word(bits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_cover_every_integer_once() {
        let names = IntegerType::ALL.map(IntegerType::name);
        assert_eq!(
            names,
            [
                "i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64", "usize"
            ]
        );
        for (kind, name) in IntegerType::ALL.into_iter().zip(names) {
            assert_eq!(IntegerType::from_name(name), Some(kind));
        }
    }

    #[test]
    fn canonical_words_extend_by_signedness() {
        assert_eq!(IntegerType::I8.canonical_word(0x80), 0xffff_ffff_ffff_ff80);
        assert_eq!(
            IntegerType::I16.canonical_word(0x8000),
            0xffff_ffff_ffff_8000
        );
        assert_eq!(
            IntegerType::I32.canonical_word(0x8000_0000),
            0xffff_ffff_8000_0000
        );
        assert_eq!(
            IntegerType::U32.canonical_word(u32::MAX as u64),
            u32::MAX as u64
        );
        assert_eq!(IntegerType::I64.canonical_word(u64::MAX), u64::MAX);
    }

    #[test]
    fn negated_magnitude_words_include_signed_minimums() {
        assert_eq!(IntegerType::I8.negated_magnitude_word(1), u64::MAX);
        assert_eq!(
            IntegerType::I8.negated_magnitude_word(128),
            0xffff_ffff_ffff_ff80
        );
        assert_eq!(
            IntegerType::I32.negated_magnitude_word(1_u64 << 31),
            0xffff_ffff_8000_0000
        );
        assert_eq!(
            IntegerType::I64.negated_magnitude_word(1_u64 << 63),
            0x8000_0000_0000_0000
        );
    }
}
