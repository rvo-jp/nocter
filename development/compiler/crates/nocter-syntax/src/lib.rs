//! Lossless lexical and syntactic projection of normalized Nocter source.

mod diagnostic;
mod lexer;
mod literal;
mod parser;
mod query;
mod token;
mod tree;

pub use diagnostic::{ExpectedSyntax, ParseDiagnostic, ParseDiagnosticKind};
pub use lexer::{Comment, CommentKind, LexDiagnostic, LexDiagnosticKind, LexedFile, lex};
pub use literal::{
    DecodedStringPart, decode_plain_string_expression, decode_string_expression,
    decode_string_literal, decode_string_text,
};
pub use parser::{ParseGoal, parse};
pub use query::declaration_name_token;
pub use token::{BuiltinType, Keyword, Punctuation, StringDelimiter, Token, TokenKind};
pub use tree::{
    MissingSyntax, NodeId, NodeKind, SyntaxElement, SyntaxNode, SyntaxToken, SyntaxTree, TokenId,
};

/// Reports whether `text` is one complete source-level name.
///
/// This is the shared boundary used by the parser and tooling mutations. Contextual spellings
/// remain valid names; reserved keywords, `_`, and `Self` do not.
#[must_use]
pub fn is_valid_name(text: &str) -> bool {
    let mut bytes = text.bytes();
    bytes.next().is_some_and(is_name_start)
        && bytes.all(is_name_continue)
        && !matches!(text, "_" | "Self")
        && Keyword::from_spelling(text).is_none()
}

const fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

const fn is_name_continue(byte: u8) -> bool {
    is_name_start(byte) || byte.is_ascii_digit()
}

#[cfg(test)]
mod name_tests {
    use super::is_valid_name;

    #[test]
    fn validates_the_parser_name_language_without_rejecting_contextual_spellings() {
        assert!(is_valid_name("value"));
        assert!(is_valid_name("T2"));
        assert!(is_valid_name("where"));
        assert!(!is_valid_name(""));
        assert!(!is_valid_name("2value"));
        assert!(!is_valid_name("two-values"));
        assert!(!is_valid_name("_"));
        assert!(!is_valid_name("Self"));
        assert!(!is_valid_name("func"));
    }
}
