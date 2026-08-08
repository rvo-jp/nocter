use super::documents::OpenDocument;
use super::locations::location_link_for_byte_target;
use crate::analysis::implementation::implementation_target_for_file_analysis;
use crate::analysis::{CompileUnitAnalysis, FileAnalysis};
use crate::source::SourceMap;
use serde_json::Value;
use std::collections::HashMap;

pub(super) fn implementation_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    document: &OpenDocument,
    open_documents: &HashMap<String, OpenDocument>,
    offset: usize,
) -> Option<Value> {
    implementation_target_for_file_analysis(analysis, file, offset)
        .and_then(|target| location_link_for_byte_target(sources, open_documents, document, target))
}
