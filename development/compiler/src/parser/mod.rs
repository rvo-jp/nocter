//! Parser for Nocter source syntax.

mod closures;
mod collection_for;
mod cursor;
mod expressions;
mod items;
mod literals;
mod regions;
mod statements;
mod support;
mod types;

use crate::ast::AstFile;
use crate::diagnostics::Diagnostic;
use crate::lexer::Token;
use crate::source::{SourceId, SourceMap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseOutput {
    pub ast: Option<AstFile>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse(sources: &SourceMap, source: SourceId, tokens: &[Token]) -> ParseOutput {
    if tokens.is_empty() {
        return ParseOutput {
            ast: None,
            diagnostics: vec![Diagnostic::error(
                "E0200",
                "parser requires a token stream ending in EOF",
            )],
        };
    }

    let mut parser = Parser {
        sources,
        source,
        tokens,
        index: 0,
        pending_token: None,
        diagnostics: Vec::new(),
        literal_pack_capture: None,
    };

    match parser.parse_source_file() {
        Ok(ast) if parser.diagnostics.is_empty() => ParseOutput {
            ast: Some(ast),
            diagnostics: parser.diagnostics,
        },
        Ok(_) | Err(()) => ParseOutput {
            ast: None,
            diagnostics: parser.diagnostics,
        },
    }
}

type ParseResult<T> = Result<T, ()>;

struct Parser<'a> {
    sources: &'a SourceMap,
    source: SourceId,
    tokens: &'a [Token],
    index: usize,
    pending_token: Option<Token>,
    diagnostics: Vec<Diagnostic>,
    literal_pack_capture: Option<String>,
}

#[cfg(test)]
mod tests;
