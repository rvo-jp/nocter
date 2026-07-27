use super::documents::OpenDocument;
use super::locations::{location_for_byte_span, location_for_document_span};
use super::protocol::{lsp_position_to_byte_offset, position_from_params};
use crate::analysis::references::{
    reference_spans_for_file_analysis as analysis_reference_spans_for_file,
    reference_spans_for_text,
};
use crate::analysis::{CompileUnitAnalysis, FileAnalysis};
use crate::source::SourceMap;
use serde_json::Value;
use std::collections::HashMap;

pub(super) fn references_for_document(
    document: &OpenDocument,
    params: Option<&Value>,
) -> Vec<Value> {
    let Some(position) = position_from_params(params) else {
        return Vec::new();
    };
    let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
    let include_declaration = include_declaration_from_params(params);

    reference_spans_for_text(&document.text, offset, include_declaration)
        .unwrap_or_default()
        .into_iter()
        .map(|span| location_for_document_span(document, span))
        .collect()
}

pub(super) fn references_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    open_documents: &HashMap<String, OpenDocument>,
    params: Option<&Value>,
    offset: usize,
) -> Vec<Value> {
    let include_declaration = include_declaration_from_params(params);

    analysis_reference_spans_for_file(analysis, file, offset, include_declaration)
        .into_iter()
        .filter_map(|span| location_for_byte_span(sources, open_documents, span))
        .collect()
}

fn include_declaration_from_params(params: Option<&Value>) -> bool {
    params
        .and_then(|params| params.get("context"))
        .and_then(|context| context.get("includeDeclaration"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
