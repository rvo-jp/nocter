//! Single-file parsing helpers for analysis fallbacks.

use crate::ast::AstFile;
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::{ResolveOutput, resolve};
use crate::source::{SourceId, SourceMap};
use std::collections::HashMap;

pub(crate) struct ParsedSingleFile {
    pub(crate) source: SourceId,
    pub(crate) ast: AstFile,
}

pub(crate) fn parse_single_file_text(display_path: &str, text: &str) -> Option<ParsedSingleFile> {
    let mut sources = SourceMap::new();
    let source = sources.add_source(display_path, None, text.to_string());
    let lex_output = lex(&sources, source);
    if !lex_output.diagnostics.is_empty() {
        return None;
    }
    let ast = parse(&sources, source, &lex_output.tokens).ast?;

    Some(ParsedSingleFile { source, ast })
}

pub(crate) fn resolve_single_file_ast(
    display_path: &str,
    text: &str,
    source: SourceId,
    ast: &AstFile,
) -> ResolveOutput {
    let mut sources = SourceMap::new();
    let resolved_source = sources.add_source(display_path, None, text.to_string());
    debug_assert_eq!(resolved_source.raw(), source.raw());
    resolve(&sources, ast)
}

pub(crate) fn analyze_single_file_text(
    display_path: &str,
    text: &str,
) -> Option<(SourceMap, super::CompileUnitAnalysis)> {
    let parsed = parse_single_file_text(display_path, text)?;
    let mut sources = SourceMap::new();
    let source = sources.add_source(display_path, None, text.to_string());
    debug_assert_eq!(source.raw(), parsed.source.raw());
    let unit = super::CompileUnit::new(
        parsed.ast.clone(),
        vec![parsed.ast],
        HashMap::new(),
        HashMap::new(),
        None,
    );
    let analysis = super::analyze_module_compile_unit(&sources, &unit);
    Some((sources, analysis))
}
