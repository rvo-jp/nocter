use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Identifier,
    Keyword(Keyword),
    IntegerLiteral,
    StringLiteral,
    ByteLiteral,
    Newline,
    Punctuation(&'static str),
    Eof,
}

impl TokenKind {
    pub fn json_kind(&self) -> &'static str {
        match self {
            TokenKind::Identifier => "identifier",
            TokenKind::Keyword(_) => "keyword",
            TokenKind::IntegerLiteral => "integer_literal",
            TokenKind::StringLiteral => "string_literal",
            TokenKind::ByteLiteral => "byte_literal",
            TokenKind::Newline => "newline",
            TokenKind::Punctuation(_) => "punctuation",
            TokenKind::Eof => "eof",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Use,
    Func,
    Pub,
    Type,
    Struct,
    Enum,
    Interface,
    Instance,
    Conform,
    Method,
    Let,
    Var,
    Return,
    If,
    Else,
    For,
    In,
    While,
    Loop,
    Break,
    Continue,
    Match,
    Is,
    Otherwise,
    Catch,
    None,
    True,
    False,
    Move,
    As,
    Region,
    Using,
    Primitive,
    Literal,
    Construct,
    Coerce,
    Test,
    Void,
    Never,
}

pub(crate) const KEYWORD_LEXEMES: &[&str] = &[
    "use",
    "func",
    "pub",
    "type",
    "struct",
    "enum",
    "interface",
    "instance",
    "conform",
    "method",
    "let",
    "var",
    "return",
    "if",
    "else",
    "for",
    "in",
    "while",
    "loop",
    "break",
    "continue",
    "match",
    "is",
    "otherwise",
    "catch",
    "none",
    "true",
    "false",
    "move",
    "as",
    "region",
    "using",
    "primitive",
    "literal",
    "construct",
    "coerce",
    "test",
    "void",
    "never",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexOutput {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}
