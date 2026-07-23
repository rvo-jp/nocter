use super::documents::OpenDocument;
use super::protocol::{range_for_byte_span, source_file_uri};
use crate::source::{ByteSpan, SourceFile, SourceMap};
use serde_json::{Value, json};
use std::collections::HashMap;

pub(super) fn location_for_byte_span(
    sources: &SourceMap,
    open_documents: &HashMap<String, OpenDocument>,
    span: ByteSpan,
) -> Option<Value> {
    let source = sources.get(span.source)?;
    let uri = open_document_uri_for_source(open_documents, source)
        .unwrap_or_else(|| source_file_uri(source));
    Some(json!({
        "uri": uri,
        "range": range_for_byte_span(source.text(), span)
    }))
}

pub(super) fn location_for_document_span(document: &OpenDocument, span: ByteSpan) -> Value {
    json!({
        "uri": document.uri,
        "range": range_for_byte_span(&document.text, span)
    })
}

fn open_document_uri_for_source(
    open_documents: &HashMap<String, OpenDocument>,
    source: &SourceFile,
) -> Option<String> {
    if let Some(source_path) = source.absolute_path()
        && let Some(document) = open_documents
            .values()
            .find(|document| document.absolute_path.as_ref() == Some(source_path))
    {
        return Some(document.uri.clone());
    }

    open_documents
        .values()
        .find(|document| {
            document.display_path == source.display_path() || document.uri == source.display_path()
        })
        .map(|document| document.uri.clone())
}
