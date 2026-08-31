//! Source-independent vocabulary shared by Nocter's syntax and semantic layers.

include!(concat!(env!("OUT_DIR"), "/diagnostic_code.rs"));

/// Closed set of primitive type names defined by the language.
///
/// Keeping this identity below both syntax and semantic modeling makes the set, ordering, and
/// spelling one compiler-wide authority. Contextual parsing does not turn these names into a
/// separate lexical token category.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BuiltinType {
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Usize,
    Isize,
    Str,
    Error,
    Void,
    Never,
}

impl BuiltinType {
    pub const ALL: &'static [Self] = &[
        Self::Bool,
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::Usize,
        Self::Isize,
        Self::Str,
        Self::Error,
        Self::Void,
        Self::Never,
    ];

    pub const COUNT: usize = Self::ALL.len();

    #[must_use]
    pub fn from_spelling(text: &str) -> Option<Self> {
        match text {
            "bool" => Some(Self::Bool),
            "i8" => Some(Self::I8),
            "i16" => Some(Self::I16),
            "i32" => Some(Self::I32),
            "i64" => Some(Self::I64),
            "u8" => Some(Self::U8),
            "u16" => Some(Self::U16),
            "u32" => Some(Self::U32),
            "u64" => Some(Self::U64),
            "usize" => Some(Self::Usize),
            "isize" => Some(Self::Isize),
            "str" => Some(Self::Str),
            "error" => Some(Self::Error),
            "void" => Some(Self::Void),
            "never" => Some(Self::Never),
            _ => None,
        }
    }

    #[must_use]
    pub const fn spelling(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::Usize => "usize",
            Self::Isize => "isize",
            Self::Str => "str",
            Self::Error => "error",
            Self::Void => "void",
            Self::Never => "never",
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.spelling()
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn is_declaration_pattern(self) -> bool {
        !matches!(self, Self::Void | Self::Never)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::BuiltinType;

    #[test]
    fn spellings_are_unique_and_round_trip() {
        let spellings = BuiltinType::ALL
            .iter()
            .map(|builtin| builtin.spelling())
            .collect::<BTreeSet<_>>();
        assert_eq!(spellings.len(), BuiltinType::COUNT);
        for builtin in BuiltinType::ALL {
            assert_eq!(
                BuiltinType::from_spelling(builtin.spelling()),
                Some(*builtin)
            );
        }
        assert_eq!(BuiltinType::from_spelling("String"), None);
    }
}
