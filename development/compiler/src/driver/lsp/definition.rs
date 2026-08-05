use super::documents::OpenDocument;
use super::locations::{location_link_for_byte_target, location_link_for_document_target};
use super::protocol::{lsp_position_to_byte_offset, position_from_params};
use crate::analysis::definition::{
    definition_target_for_file_analysis as analysis_definition_target_for_file,
    definition_target_for_text,
};
use crate::analysis::{CompileUnitAnalysis, FileAnalysis};
use crate::source::SourceMap;
use serde_json::Value;
use std::collections::HashMap;

pub(super) fn definition_for_document(
    document: &OpenDocument,
    params: Option<&Value>,
) -> Option<Value> {
    let position = position_from_params(params)?;
    let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
    definition_target_for_text(&document.text, offset)
        .map(|target| location_link_for_document_target(document, target))
}

pub(super) fn definition_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    document: &OpenDocument,
    open_documents: &HashMap<String, OpenDocument>,
    offset: usize,
) -> Option<Value> {
    analysis_definition_target_for_file(sources, analysis, file, offset)
        .and_then(|target| location_link_for_byte_target(sources, open_documents, document, target))
}
