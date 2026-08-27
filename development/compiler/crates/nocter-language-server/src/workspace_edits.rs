use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use nocter_analysis::{SemanticSourceEdit, ValidatedSemanticMutation};
use nocter_filesystem::DocumentVersion;
use nocter_json::Value;
use nocter_lsp::{
    DocumentEdit, DocumentUri, DocumentUriError, Position, Range, TextEdit, workspace_edit_result,
};
use nocter_source::{CoordinateError, SourceId};

/// Projects compiler-owned byte edits as one versioned atomic LSP workspace edit.
///
/// # Errors
///
/// Returns source identity, URI, coordinate, or overlap errors without changing editor state.
pub(crate) fn project_workspace_edit(
    mutation: ValidatedSemanticMutation<'_>,
) -> Result<Value, WorkspaceEditError> {
    let (snapshot, edits) = mutation.into_source_edits();
    let mut documents = Vec::new();
    for (source_id, source_edits) in grouped_edits(&edits)? {
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
    OverlappingEdits(SourceId),
    Uri(DocumentUriError),
    Coordinate(CoordinateError),
}

impl fmt::Display for WorkspaceEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource(source) => {
                write!(formatter, "workspace edit source is missing: {source}")
            }
            Self::OverlappingEdits(source) => {
                write!(formatter, "workspace edits overlap in {source}")
            }
            Self::Uri(error) => error.fmt(formatter),
            Self::Coordinate(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for WorkspaceEditError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Uri(error) => Some(error),
            Self::Coordinate(error) => Some(error),
            Self::MissingSource(_) | Self::OverlappingEdits(_) => None,
        }
    }
}
