use crate::BuiltinType;

/// Source-independent representation facts used to validate explicit primitive conversions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuiltinNumericType {
    Integer { bits: u8, signed: bool },
    Float { bits: u8, precision: u8 },
}

impl BuiltinNumericType {
    #[must_use]
    pub const fn for_builtin(builtin: BuiltinType) -> Option<Self> {
        let numeric = match builtin {
            BuiltinType::I8 => Self::Integer {
                bits: 8,
                signed: true,
            },
            BuiltinType::I16 => Self::Integer {
                bits: 16,
                signed: true,
            },
            BuiltinType::I32 => Self::Integer {
                bits: 32,
                signed: true,
            },
            BuiltinType::I64 | BuiltinType::Isize => Self::Integer {
                bits: 64,
                signed: true,
            },
            BuiltinType::U8 => Self::Integer {
                bits: 8,
                signed: false,
            },
            BuiltinType::U16 => Self::Integer {
                bits: 16,
                signed: false,
            },
            BuiltinType::U32 => Self::Integer {
                bits: 32,
                signed: false,
            },
            BuiltinType::U64 | BuiltinType::Usize => Self::Integer {
                bits: 64,
                signed: false,
            },
            BuiltinType::F32 => Self::Float {
                bits: 32,
                precision: 24,
            },
            BuiltinType::F64 => Self::Float {
                bits: 64,
                precision: 53,
            },
            BuiltinType::Bool
            | BuiltinType::Char
            | BuiltinType::Str
            | BuiltinType::Error
            | BuiltinType::Void
            | BuiltinType::Never => return None,
        };
        Some(numeric)
    }
}

/// Reports whether `as` preserves every value in the source primitive's complete domain.
#[must_use]
pub const fn lossless_builtin_numeric_conversion(source: BuiltinType, target: BuiltinType) -> bool {
    let (Some(source), Some(target)) = (
        BuiltinNumericType::for_builtin(source),
        BuiltinNumericType::for_builtin(target),
    ) else {
        return false;
    };
    match (source, target) {
        (
            BuiltinNumericType::Integer {
                bits: source_bits,
                signed: source_signed,
            },
            BuiltinNumericType::Integer {
                bits: target_bits,
                signed: target_signed,
            },
        ) => match (source_signed, target_signed) {
            (false, false) | (true, true) => source_bits <= target_bits,
            (false, true) => source_bits < target_bits,
            (true, false) => false,
        },
        (
            BuiltinNumericType::Integer { bits, signed },
            BuiltinNumericType::Float { precision, .. },
        ) => bits - if signed { 1 } else { 0 } <= precision,
        (
            BuiltinNumericType::Float {
                bits: source_bits, ..
            },
            BuiltinNumericType::Float {
                bits: target_bits, ..
            },
        ) => source_bits < target_bits,
        (BuiltinNumericType::Float { .. }, BuiltinNumericType::Integer { .. }) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::lossless_builtin_numeric_conversion as lossless;
    use crate::BuiltinType;

    #[test]
    fn lossless_conversion_is_decided_from_complete_source_domains() {
        assert!(lossless(BuiltinType::U32, BuiltinType::I64));
        assert!(!lossless(BuiltinType::I32, BuiltinType::U64));
        assert!(lossless(BuiltinType::I16, BuiltinType::F32));
        assert!(lossless(BuiltinType::U16, BuiltinType::F32));
        assert!(!lossless(BuiltinType::I32, BuiltinType::F32));
        assert!(lossless(BuiltinType::I32, BuiltinType::F64));
        assert!(lossless(BuiltinType::F32, BuiltinType::F64));
        assert!(!lossless(BuiltinType::F64, BuiltinType::F32));
    }
}
