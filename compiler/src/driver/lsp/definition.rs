use super::documents::OpenDocument;
use super::hover::{definition_span_for_ast, resolve_single_file_for_hover};
use super::protocol::{lsp_position_to_byte_offset, position_from_params, range_for_byte_span};
use crate::analysis::FileAnalysis;
use crate::lexer::lex;
use crate::parser::parse;
use crate::source::{ByteSpan, SourceMap};
use serde_json::{Value, json};

pub(super) fn definition_for_document(
    document: &OpenDocument,
    params: Option<&Value>,
) -> Option<Value> {
    let position = position_from_params(params)?;
    let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
    let mut sources = SourceMap::new();
    let source = sources.add_source(
        document.display_path.clone(),
        document.absolute_path.clone(),
        document.text.clone(),
    );
    let lex_output = lex(&sources, source);
    if !lex_output.diagnostics.is_empty() {
        return None;
    }
    let ast = parse(&sources, source, &lex_output.tokens).ast?;
    let resolved = resolve_single_file_for_hover(&document.text, source, &ast);
    definition_span_for_ast(&document.text, &ast, &resolved, offset)
        .and_then(|span| location_for_byte_span(&sources, span))
}

pub(super) fn definition_for_file_analysis(
    sources: &SourceMap,
    file: &FileAnalysis,
    offset: usize,
) -> Option<Value> {
    let text = sources.get(file.ast.span.source)?.text();
    definition_span_for_ast(text, &file.ast, &file.resolved, offset)
        .and_then(|span| location_for_byte_span(sources, span))
}

fn location_for_byte_span(sources: &SourceMap, span: ByteSpan) -> Option<Value> {
    let source = sources.get(span.source)?;
    Some(json!({
        "uri": uri_for_source_file(source),
        "range": range_for_byte_span(source.text(), span)
    }))
}

fn uri_for_source_file(source: &crate::source::SourceFile) -> String {
    source
        .absolute_path()
        .map(|path| format!("file://{}", percent_encode_path(&path.to_string_lossy())))
        .unwrap_or_else(|| source.display_path().to_string())
}

fn percent_encode_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}
