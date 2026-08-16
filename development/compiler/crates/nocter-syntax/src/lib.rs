//! Lossless lexical and syntactic projection of normalized Nocter source.

mod diagnostic;
mod lexer;
mod literal;
mod parser;
mod token;
mod tree;

pub use diagnostic::{ExpectedSyntax, ParseDiagnostic, ParseDiagnosticKind};
pub use lexer::{Comment, CommentKind, LexDiagnostic, LexDiagnosticKind, LexedFile, lex};
pub use parser::{ParseGoal, parse};
pub use token::{Keyword, Punctuation, StringDelimiter, Token, TokenKind};
pub use tree::{
    MissingSyntax, NodeId, NodeKind, SyntaxElement, SyntaxNode, SyntaxToken, SyntaxTree, TokenId,
};
