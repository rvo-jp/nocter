use super::documents::OpenDocument;
use super::protocol::{
    lsp_position_to_byte_offset, position_from_params, range_for_byte_span, source_file_uri,
};
use crate::analysis::definition::{
    definition_span_for_file_analysis as analysis_definition_span_for_file,
    definition_span_for_text,
};
use crate::analysis::{CompileUnitAnalysis, FileAnalysis};
use crate::source::{ByteSpan, SourceMap};
use serde_json::{Value, json};

pub(super) fn definition_for_document(
    document: &OpenDocument,
    params: Option<&Value>,
) -> Option<Value> {
    let position = position_from_params(params)?;
    let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
    definition_span_for_text(&document.text, offset)
        .map(|span| location_for_document_span(document, span))
}

pub(super) fn definition_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<Value> {
    analysis_definition_span_for_file(sources, analysis, file, offset)
        .and_then(|span| location_for_byte_span(sources, span))
}

fn location_for_byte_span(sources: &SourceMap, span: ByteSpan) -> Option<Value> {
    let source = sources.get(span.source)?;
    Some(json!({
        "uri": source_file_uri(source),
        "range": range_for_byte_span(source.text(), span)
    }))
}

fn location_for_document_span(document: &OpenDocument, span: ByteSpan) -> Value {
    json!({
        "uri": document.uri,
        "range": range_for_byte_span(&document.text, span)
    })
}
