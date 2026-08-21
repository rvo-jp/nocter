use nocter_source::Span;

/// Closed contextual type spellings recognized by the syntax grammar.
///
/// These do not form a separate lexical token category: most are identifier tokens while `void`
/// and `never` are keywords. Parser contexts use this classification without duplicating spelling
/// tables or changing value-position identifier rules.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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
    pub const fn as_str(self) -> &'static str {
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
    pub const fn is_declaration_pattern(self) -> bool {
        !matches!(self, Self::Void | Self::Never)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StringDelimiter {
    SingleLine,
    MultiLine,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TokenKind {
    Identifier,
    Keyword(Keyword),
    IntegerLiteral,
    ByteLiteral,
    StringStart(StringDelimiter),
    StringText,
    InterpolationStart,
    InterpolationEnd,
    StringEnd(StringDelimiter),
    Newline,
    Punctuation(Punctuation),
    Eof,
}

impl TokenKind {
    /// Stable category spelling used by compiler-owned tooling protocols.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Identifier => "identifier",
            Self::Keyword(_) => "keyword",
            Self::IntegerLiteral => "integer_literal",
            Self::ByteLiteral => "byte_literal",
            Self::StringStart(_) => "string_start",
            Self::StringText => "string_text",
            Self::InterpolationStart => "interpolation_start",
            Self::InterpolationEnd => "interpolation_end",
            Self::StringEnd(_) => "string_end",
            Self::Newline => "newline",
            Self::Punctuation(_) => "punctuation",
            Self::Eof => "eof",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Token {
    kind: TokenKind,
    span: Span,
    joint_to_next: bool,
}

impl Token {
    #[must_use]
    pub const fn new(kind: TokenKind, span: Span) -> Self {
        Self {
            kind,
            span,
            joint_to_next: false,
        }
    }

    #[must_use]
    pub const fn kind(self) -> TokenKind {
        self.kind
    }

    #[must_use]
    pub const fn span(self) -> Span {
        self.span
    }

    #[must_use]
    pub const fn is_joint_to_next(self) -> bool {
        self.joint_to_next
    }

    pub(crate) fn set_joint_to_next(&mut self, joint: bool) {
        self.joint_to_next = joint;
    }
}

macro_rules! keywords {
    ($($variant:ident => $text:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum Keyword {
            $($variant),+
        }

        impl Keyword {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            #[must_use]
            pub fn from_spelling(text: &str) -> Option<Self> {
                match text {
                    $($text => Some(Self::$variant),)+
                    _ => None,
                }
            }

            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text,)+
                }
            }
        }
    };
}

keywords! {
    As => "as",
    Break => "break",
    Catch => "catch",
    Continue => "continue",
    Construct => "construct",
    Conform => "conform",
    Else => "else",
    Enum => "enum",
    False => "false",
    For => "for",
    Func => "func",
    If => "if",
    In => "in",
    Instance => "instance",
    Interface => "interface",
    Is => "is",
    Let => "let",
    Literal => "literal",
    Loop => "loop",
    Match => "match",
    Method => "method",
    Move => "move",
    Never => "never",
    None => "none",
    Operator => "operator",
    Otherwise => "otherwise",
    Primitive => "primitive",
    Pub => "pub",
    Region => "region",
    Return => "return",
    Struct => "struct",
    Test => "test",
    True => "true",
    Type => "type",
    Use => "use",
    Using => "using",
    Var => "var",
    Void => "void",
    While => "while",
}

macro_rules! punctuation {
    ($($variant:ident => $text:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum Punctuation {
            $($variant),+
        }

        impl Punctuation {
            /// Punctuation spellings in longest-match priority order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text,)+
                }
            }

            #[must_use]
            pub fn longest_prefix(text: &str) -> Option<Self> {
                Self::ALL
                    .iter()
                    .copied()
                    .find(|punctuation| text.starts_with(punctuation.as_str()))
            }
        }
    };
}

punctuation! {
    ReadWrite => "&+",
    Range => "..<",
    Expansion => "...",
    EqualEqual => "==",
    BangEqual => "!=",
    LessEqual => "<=",
    GreaterEqual => ">=",
    LogicalAnd => "&&",
    LogicalOr => "||",
    ShiftLeft => "<<",
    ShiftRight => ">>",
    PlusEqual => "+=",
    MinusEqual => "-=",
    StarEqual => "*=",
    SlashEqual => "/=",
    PercentEqual => "%=",
    Hash => "#",
    Dot => ".",
    Slash => "/",
    Star => "*",
    Ampersand => "&",
    Less => "<",
    Greater => ">",
    Equal => "=",
    Plus => "+",
    Minus => "-",
    Percent => "%",
    Bang => "!",
    Question => "?",
    Pipe => "|",
    LeftParen => "(",
    RightParen => ")",
    LeftBrace => "{",
    RightBrace => "}",
    LeftBracket => "[",
    RightBracket => "]",
    Colon => ":",
    Comma => ",",
    Semicolon => ";",
}

#[cfg(test)]
mod tests;
