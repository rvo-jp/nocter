//! Lossless lexical and syntactic projection of normalized Nocter source.

mod body_surface;
mod bound;
mod completeness;
mod contextual;
mod diagnostic;
mod documentation;
mod lexer;
mod literal;
mod name;
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
pub use bound::BoundSyntax;
pub use completeness::node_is_complete;
pub use contextual::ContextualSpelling;
pub use diagnostic::{ExpectedSyntax, ParseDiagnostic, ParseDiagnosticKind};
pub use lexer::{Comment, CommentKind, LexDiagnostic, LexDiagnosticKind, LexedFile, lex};
pub use literal::{
    DecodedStringPart, FloatLiteralSpelling, FloatLiteralSuffix, decode_byte_literal,
    decode_character_literal, decode_plain_string_expression, decode_string_expression,
    decode_string_literal, decode_string_text,
};
pub use name::{is_valid_module_segment, is_valid_name};
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
pub use query::{declaration_contextual_keyword_token, declaration_name_token};
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
