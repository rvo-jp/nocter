use nocter_source::Span;

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
