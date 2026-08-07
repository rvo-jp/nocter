//! Parser for Nocter source syntax.

mod closures;
mod collection_for;
mod constructs;
mod cursor;
mod expressions;
mod items;
mod literals;
mod regions;
mod removed_result_allocation;
mod statements;
mod support;
mod types;

use crate::ast::{AstFile, PackageFile};
use crate::diagnostics::Diagnostic;
use crate::lexer::{Token, TokenKind};
use crate::source::{SourceId, SourceMap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseOutput {
    pub ast: Option<AstFile>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageParseOutput {
    pub package_file: Option<PackageFile>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse(sources: &SourceMap, source: SourceId, tokens: &[Token]) -> ParseOutput {
    if !tokens
        .last()
        .is_some_and(|token| token.kind == TokenKind::Eof)
    {
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

pub fn parse_package_file(
    sources: &SourceMap,
    source: SourceId,
    tokens: &[Token],
) -> PackageParseOutput {
    if !tokens
        .last()
        .is_some_and(|token| token.kind == TokenKind::Eof)
    {
        return PackageParseOutput {
            package_file: None,
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
    parser.skip_newlines();
    match parser.parse_package_file() {
        Ok(package_file) if parser.diagnostics.is_empty() => PackageParseOutput {
            package_file: Some(package_file),
            diagnostics: parser.diagnostics,
        },
        Ok(_) | Err(()) => PackageParseOutput {
            package_file: None,
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
