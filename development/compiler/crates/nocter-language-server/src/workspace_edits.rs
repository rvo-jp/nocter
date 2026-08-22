use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use nocter_analysis::{AnalysisSnapshot, SemanticSourceEdit};
use nocter_filesystem::{DocumentVersion, OpenDocument, SourceOverlay, SourceOverlayError};
use nocter_json::Value;
use nocter_lsp::{
    DocumentEdit, DocumentUri, DocumentUriError, Position, Range, TextEdit, workspace_edit_result,
};
use nocter_source::{CoordinateError, SourceId};

/// Applies compiler-owned edits to an isolated copy of one analysis generation.
///
/// # Errors
///
/// Rejects missing sources, invalid UTF-8 boundaries, overlapping edits, and overlay failures.
pub(crate) fn candidate_overlay(
    snapshot: &AnalysisSnapshot,
    edits: &[SemanticSourceEdit],
) -> Result<SourceOverlay, WorkspaceEditError> {
    let grouped = grouped_edits(edits)?;
    let mut documents = BTreeMap::new();
    for (path, document) in snapshot.source_overlay().documents() {
        documents.insert(path.to_path_buf(), document.clone());
    }
    for source in snapshot.sources().iter() {
        let path = PathBuf::from(source.name().as_str());
        let version = snapshot
            .document_version(&path)
            .unwrap_or(DocumentVersion::new(0));
        let mut text = source.text().to_owned();
        if let Some(source_edits) = grouped.get(&source.id()) {
            for edit in source_edits.iter().rev() {
                let start = usize::try_from(edit.range().start().get())
                    .map_err(|_| WorkspaceEditError::InvalidEdit(source.id()))?;
                let end = usize::try_from(edit.range().end().get())
                    .map_err(|_| WorkspaceEditError::InvalidEdit(source.id()))?;
                if !text.is_char_boundary(start) || !text.is_char_boundary(end) || start > end {
                    return Err(WorkspaceEditError::InvalidEdit(source.id()));
                }
                text.replace_range(start..end, edit.new_text());
            }
        }
        documents.insert(path, OpenDocument::new(version, text.into_bytes()));
    }
    for source in grouped.keys() {
        if snapshot.sources().get(*source).is_none() {
            return Err(WorkspaceEditError::MissingSource(*source));
        }
    }
    let mut builder = SourceOverlay::builder();
    for (path, document) in documents {
        builder
            .insert(path, document)
            .map_err(WorkspaceEditError::Overlay)?;
    }
    Ok(builder.finish())
}

/// Projects compiler-owned byte edits as one versioned atomic LSP workspace edit.
///
/// # Errors
///
/// Returns source identity, URI, coordinate, or overlap errors without changing editor state.
pub(crate) fn project_workspace_edit(
    snapshot: &AnalysisSnapshot,
    edits: &[SemanticSourceEdit],
) -> Result<Value, WorkspaceEditError> {
    let mut documents = Vec::new();
    for (source_id, source_edits) in grouped_edits(edits)? {
        let source = snapshot
            .sources()
            .get(source_id)
            .ok_or(WorkspaceEditError::MissingSource(source_id))?;
        let path = Path::new(source.name().as_str());
        let uri = DocumentUri::from_file_path(path).map_err(WorkspaceEditError::Uri)?;
        let version = snapshot.document_version(path).map(DocumentVersion::get);
        let edits = source_edits
            .iter()
            .map(|edit| {
                let range = source
                    .utf16_range(edit.range())
                    .map_err(WorkspaceEditError::Coordinate)?;
                Ok(TextEdit::new(
                    Range::new(
                        Position::new(range.start().line(), range.start().character()),
                        Position::new(range.end().line(), range.end().character()),
                    ),
                    edit.new_text(),
                ))
            })
            .collect::<Result<Vec<_>, WorkspaceEditError>>()?;
        documents.push(DocumentEdit::new(uri, version, edits));
    }
    Ok(workspace_edit_result(&documents))
}

fn grouped_edits(
    edits: &[SemanticSourceEdit],
) -> Result<BTreeMap<SourceId, Vec<&SemanticSourceEdit>>, WorkspaceEditError> {
    let mut grouped = BTreeMap::<_, Vec<_>>::new();
    for edit in edits {
        grouped.entry(edit.source()).or_default().push(edit);
    }
    for (source, edits) in &mut grouped {
        edits.sort_by_key(|edit| edit.range());
        if edits
            .windows(2)
            .any(|pair| pair[0].range().end() > pair[1].range().start())
        {
            return Err(WorkspaceEditError::OverlappingEdits(*source));
        }
    }
    Ok(grouped)
}

#[derive(Debug)]
pub enum WorkspaceEditError {
    MissingSource(SourceId),
    InvalidEdit(SourceId),
    OverlappingEdits(SourceId),
    Overlay(SourceOverlayError),
    Uri(DocumentUriError),
    Coordinate(CoordinateError),
}

impl fmt::Display for WorkspaceEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource(source) => {
                write!(formatter, "workspace edit source is missing: {source}")
            }
            Self::InvalidEdit(source) => write!(formatter, "workspace edit is invalid in {source}"),
            Self::OverlappingEdits(source) => {
                write!(formatter, "workspace edits overlap in {source}")
            }
            Self::Overlay(error) => error.fmt(formatter),
            Self::Uri(error) => error.fmt(formatter),
            Self::Coordinate(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for WorkspaceEditError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Overlay(error) => Some(error),
            Self::Uri(error) => Some(error),
            Self::Coordinate(error) => Some(error),
            Self::MissingSource(_) | Self::InvalidEdit(_) | Self::OverlappingEdits(_) => None,
        }
    }
}
