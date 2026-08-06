use super::documents::OpenDocument;
use super::protocol::{file_uri_for_path, range_for_byte_span};
use crate::analysis::FileAnalysis;
use crate::analysis::package_index::{PackageSemanticIndex, stable_semantic_identity_at};
use crate::package::PackageGraph;
use crate::source::{ByteSpan, SourceId, SourceMap};
use serde_json::{Value, json};
use std::collections::HashMap;

pub(super) struct PackageReferenceQuery<'a> {
    pub(super) sources: &'a SourceMap,
    pub(super) file: &'a FileAnalysis,
    pub(super) index: &'a PackageSemanticIndex,
    pub(super) graph: Option<&'a PackageGraph>,
    pub(super) open_documents: &'a HashMap<String, OpenDocument>,
    pub(super) offset: usize,
    pub(super) include_declaration: bool,
}

pub(super) fn package_references(query: PackageReferenceQuery<'_>) -> Option<Vec<Value>> {
    let identity =
        stable_semantic_identity_at(query.sources, query.file, query.offset, query.graph)?;
    let mut references = Vec::new();
    for occurrence in query.index.references(&identity, query.include_declaration) {
        let text = query.index.source_text(&occurrence.span.source)?;
        let uri = occurrence
            .span
            .source
            .absolute_path()
            .and_then(|path| {
                query
                    .open_documents
                    .values()
                    .find(|document| document.absolute_path.as_deref() == Some(path))
                    .map(|document| document.uri.clone())
                    .or_else(|| Some(file_uri_for_path(path)))
            })
            .unwrap_or_else(|| occurrence.span.source.display_path().to_string());
        references.push(json!({
            "uri": uri,
            "range": range_for_byte_span(
                text,
                ByteSpan::new(SourceId::new(0), occurrence.span.start, occurrence.span.end),
            )
        }));
    }
    Some(references)
}
