//! Tokenization for `.nct` source files.

use crate::diagnostics::Diagnostic;
use crate::literals::{find_interpolation_end, validate_string_literal_source};
use crate::source::{ByteSpan, JsonSpan, SourceId, SourceMap};
use serde::Serialize;

mod identifiers;
mod json;
mod model;
mod numbers;
mod scanner;
#[cfg(test)]
mod tests;

pub(crate) use identifiers::is_valid_identifier_name;
pub use json::{JsonToken, TokensEnvelope};
pub(crate) use model::KEYWORD_LEXEMES;
pub use model::{Keyword, LexOutput, Token, TokenKind};
pub use scanner::{lex, lex_span};
