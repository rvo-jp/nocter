use super::documents::OpenDocument;
use super::protocol::{LspPosition, LspRange, byte_offset_to_lsp_position};
use crate::diagnostics::{Diagnostic, DiagnosticNote, Severity};
use crate::source::{JsonSpan, SourceFile, SourceMap};
use serde::Serialize;
use serde_json::{Value, json};
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LspDiagnostic {
    range: LspRange,
    severity: u8,
    pub(super) code: String,
    source: &'static str,
    pub(super) message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) related_information: Vec<LspDiagnosticRelatedInformation>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LspDiagnosticRelatedInformation {
    pub(super) location: LspLocation,
    pub(super) message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LspLocation {
    pub(super) uri: String,
    pub(super) range: LspRange,
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
    open_documents: &[&OpenDocument],
    sources: &SourceMap,
    diagnostics: Vec<Diagnostic>,
) -> Vec<LspDiagnostic> {
    diagnostics
        .into_iter()
        .filter_map(|diagnostic| diagnostic_for_lsp(document, open_documents, sources, diagnostic))
        .collect()
}

fn diagnostic_for_lsp(
    document: &OpenDocument,
    open_documents: &[&OpenDocument],
    sources: &SourceMap,
    diagnostic: Diagnostic,
) -> Option<LspDiagnostic> {
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

    let (related_information, appended_notes) =
        related_information_for_notes(document, open_documents, sources, diagnostic.notes);
    let message = message_with_notes_and_help(diagnostic.message, appended_notes, diagnostic.help);

    Some(LspDiagnostic {
        range,
        severity: lsp_severity(diagnostic.severity),
        code: diagnostic.code,
        source: "nocter",
        message,
        related_information,
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

fn related_information_for_notes(
    document: &OpenDocument,
    open_documents: &[&OpenDocument],
    sources: &SourceMap,
    notes: Vec<DiagnosticNote>,
) -> (Vec<LspDiagnosticRelatedInformation>, Vec<String>) {
    let mut related_information = Vec::new();
    let mut appended_notes = Vec::new();

    for note in notes {
        let Some(span) = note.span else {
            appended_notes.push(note.message);
            continue;
        };

        match location_for_span(document, open_documents, sources, &span) {
            Some(location) => related_information.push(LspDiagnosticRelatedInformation {
                location,
                message: note.message,
            }),
            None => appended_notes.push(note.message),
        }
    }

    (related_information, appended_notes)
}

fn location_for_span(
    document: &OpenDocument,
    open_documents: &[&OpenDocument],
    sources: &SourceMap,
    span: &JsonSpan,
) -> Option<LspLocation> {
    if span_belongs_to_document(document, span) {
        return Some(LspLocation {
            uri: document.uri.clone(),
            range: range_for_span(&document.text, span),
        });
    }

    if let Some(open_document) = open_documents
        .iter()
        .copied()
        .find(|open_document| span_belongs_to_document(open_document, span))
    {
        return Some(LspLocation {
            uri: open_document.uri.clone(),
            range: range_for_span(&open_document.text, span),
        });
    }

    if let Some(source) = sources.file_for_json_span(span) {
        return Some(LspLocation {
            uri: uri_for_source_file(source),
            range: range_for_span(source.text(), span),
        });
    }

    let uri = uri_for_span(document, span)?;
    Some(LspLocation {
        uri,
        range: range_from_json_span(span),
    })
}

fn uri_for_source_file(source: &SourceFile) -> String {
    source
        .absolute_path()
        .map(|path| file_uri_for_path(path))
        .unwrap_or_else(|| source.display_path().to_string())
}

fn uri_for_span(document: &OpenDocument, span: &JsonSpan) -> Option<String> {
    if span_belongs_to_document(document, span) {
        return Some(document.uri.clone());
    }

    if let Some(absolute_path) = &span.absolute_path {
        return Some(file_uri_for_path(Path::new(absolute_path)));
    }

    if span.file.starts_with("file://") {
        return Some(span.file.clone());
    }

    let path = Path::new(&span.file);
    path.is_absolute().then(|| file_uri_for_path(path))
}

fn file_uri_for_path(path: &Path) -> String {
    let path = path.to_string_lossy();
    let mut uri = String::from("file://");

    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                uri.push(byte as char)
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }

    uri
}

fn range_from_json_span(span: &JsonSpan) -> LspRange {
    LspRange {
        start: LspPosition {
            line: span.start_line.saturating_sub(1),
            character: span.start_column_byte.saturating_sub(1),
        },
        end: LspPosition {
            line: span.end_line.saturating_sub(1),
            character: span.end_column_byte.saturating_sub(1),
        },
    }
}

fn message_with_notes_and_help(
    mut message: String,
    appended_notes: Vec<String>,
    help: Option<String>,
) -> String {
    for note in appended_notes {
        message.push_str("\nnote: ");
        message.push_str(&note);
    }

    if let Some(help) = help {
        message.push_str("\nhelp: ");
        message.push_str(&help);
    }

    message
}

fn lsp_severity(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 1,
        Severity::Warning => 2,
        Severity::Note => 3,
    }
}
