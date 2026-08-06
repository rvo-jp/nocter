use super::documents::OpenDocument;
use super::protocol::{file_uri_for_path, range_for_byte_span};
use crate::analysis::FileAnalysis;
use crate::analysis::package_index::{
    PackageSemanticIndex, RenamePlan, stable_semantic_identity_at,
};
use crate::package::PackageGraph;
use crate::source::{ByteSpan, SourceId, SourceMap};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

pub(super) struct RenameQuery<'a> {
    pub(super) document: &'a OpenDocument,
    pub(super) sources: &'a SourceMap,
    pub(super) file: &'a FileAnalysis,
    pub(super) index: &'a PackageSemanticIndex,
    pub(super) graph: Option<&'a PackageGraph>,
    pub(super) editable_root: &'a Path,
    pub(super) open_documents: &'a HashMap<String, OpenDocument>,
    pub(super) offset: usize,
}

pub(super) fn prepare_rename(query: &RenameQuery<'_>) -> Option<Value> {
    let occurrence = query.file.occurrences.at_offset(query.offset)?;
    let identity =
        stable_semantic_identity_at(query.sources, query.file, query.offset, query.graph)?;
    let placeholder = query
        .document
        .text
        .get(occurrence.focus_span.start..occurrence.focus_span.end)?;
    query
        .index
        .rename_plan(&identity, placeholder, query.editable_root)?;
    Some(json!({
        "range": range_for_byte_span(&query.document.text, occurrence.focus_span),
        "placeholder": placeholder
    }))
}

pub(super) fn rename_workspace_edit(query: &RenameQuery<'_>, new_name: &str) -> Option<Value> {
    let identity =
        stable_semantic_identity_at(query.sources, query.file, query.offset, query.graph)?;
    let plan = query
        .index
        .rename_plan(&identity, new_name, query.editable_root)?;
    Some(workspace_edit_for_plan(&plan, query.open_documents))
}

fn workspace_edit_for_plan(
    plan: &RenamePlan,
    open_documents: &HashMap<String, OpenDocument>,
) -> Value {
    let mut edits_by_path = BTreeMap::<_, Vec<_>>::new();
    for edit in &plan.edits {
        edits_by_path
            .entry(edit.absolute_path.clone())
            .or_default()
            .push(edit);
    }

    let document_changes = edits_by_path
        .into_iter()
        .map(|(path, edits)| {
            let open = open_documents
                .values()
                .find(|document| document.absolute_path.as_deref() == Some(path.as_path()));
            let uri = open
                .map(|document| document.uri.clone())
                .unwrap_or_else(|| file_uri_for_path(&path));
            let version = open.and_then(|document| document.version);
            let text_edits = edits
                .iter()
                .map(|edit| {
                    json!({
                        "range": range_for_byte_span(
                            &edit.source_text,
                            ByteSpan::new(SourceId::new(0), edit.start, edit.end),
                        ),
                        "newText": edit.new_name
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "textDocument": { "uri": uri, "version": version },
                "edits": text_edits
            })
        })
        .collect::<Vec<_>>();

    json!({ "documentChanges": document_changes })
}
