use super::documents::OpenDocument;
use super::protocol::{LspPosition, LspRange, byte_offset_to_lsp_position};
use crate::diagnostics::Diagnostic;
use crate::source::JsonSpan;
use serde::Serialize;
use serde_json::{Value, json};
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub(super) struct LspDiagnostic {
    range: LspRange,
    severity: u8,
    pub(super) code: String,
    source: &'static str,
    message: String,
}

pub(super) fn publish_diagnostics(uri: &str, diagnostics: Vec<LspDiagnostic>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {
            "uri": uri,
            "diagnostics": diagnostics
        }
    })
}

pub(super) fn diagnostics_for_lsp(
    document: &OpenDocument,
    diagnostics: Vec<Diagnostic>,
) -> Vec<LspDiagnostic> {
    diagnostics
        .into_iter()
        .filter_map(|diagnostic| diagnostic_for_lsp(document, diagnostic))
        .collect()
}

fn diagnostic_for_lsp(document: &OpenDocument, diagnostic: Diagnostic) -> Option<LspDiagnostic> {
    let span = diagnostic.primary_span.as_deref();
    if let Some(span) = span
        && !span_belongs_to_document(document, span)
    {
        return None;
    }

    let range = span
        .map(|span| range_for_span(&document.text, span))
        .unwrap_or_else(|| LspRange {
            start: LspPosition {
                line: 0,
                character: 0,
            },
            end: LspPosition {
                line: 0,
                character: 0,
            },
        });

    Some(LspDiagnostic {
        range,
        severity: 1,
        code: diagnostic.code,
        source: "nocter",
        message: diagnostic.message,
    })
}

fn span_belongs_to_document(document: &OpenDocument, span: &JsonSpan) -> bool {
    if let (Some(document_path), Some(span_path)) = (&document.absolute_path, &span.absolute_path) {
        return Path::new(span_path) == document_path;
    }

    span.file == document.display_path || span.file == document.uri
}

fn range_for_span(text: &str, span: &JsonSpan) -> LspRange {
    LspRange {
        start: byte_offset_to_lsp_position(text, span.start_byte),
        end: byte_offset_to_lsp_position(text, span.end_byte),
    }
}
