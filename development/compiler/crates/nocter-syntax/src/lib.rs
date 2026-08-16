//! Lossless lexical and syntactic projection of normalized Nocter source.

mod lexer;
mod literal;
mod token;

pub use lexer::{Comment, CommentKind, LexDiagnostic, LexDiagnosticKind, LexedFile, lex};
pub use token::{Keyword, Punctuation, StringDelimiter, Token, TokenKind};
