//! Lossless lexical and syntactic projection of normalized Nocter source.

mod body_surface;
mod completeness;
mod diagnostic;
mod documentation;
mod lexer;
mod literal;
mod navigation;
mod origin;
mod parser;
mod provider;
mod query;
mod surface;
mod token;
mod tree;
mod tuple;

pub use body_surface::{BodySyntaxLocator, BodySyntaxProjection, BodySyntaxSurface};
pub use completeness::node_is_complete;
pub use diagnostic::{ExpectedSyntax, ParseDiagnostic, ParseDiagnosticKind};
pub use lexer::{Comment, CommentKind, LexDiagnostic, LexDiagnosticKind, LexedFile, lex};
pub use literal::{
    DecodedStringPart, decode_plain_string_expression, decode_string_expression,
    decode_string_literal, decode_string_text,
};
pub use navigation::{
    child_node_iter, child_nodes, descendant_identifier_iter, descendant_node_iter,
    descendant_token_iter, direct_identifier, direct_identifier_iter, direct_node,
    direct_node_iter, direct_nodes, direct_token, direct_token_iter, direct_tokens,
    first_direct_token, outermost_descendant_node_iter,
};
pub use nocter_language::BuiltinType;
pub use origin::SyntaxOrigin;
pub use parser::{ParseGoal, ParsedSyntax, parse, parse_reusable};
pub use provider::{DirectSourceSyntax, SourceSyntaxError, SourceSyntaxProvider};
pub use query::declaration_name_token;
pub use surface::{
    DeclarationSyntaxLocator, DeclarationSyntaxProjection, DeclarationSyntaxSurface,
    project_declaration_syntax,
};
pub use token::{Keyword, Punctuation, StringDelimiter, Token, TokenKind};
pub use tree::{
    MissingSyntax, NodeId, NodeKind, PostfixSuffixKind, SyntaxElement, SyntaxNode, SyntaxToken,
    SyntaxTree, TokenId,
};
pub use tuple::{TupleElementIndex, TupleElementIndexError};

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
