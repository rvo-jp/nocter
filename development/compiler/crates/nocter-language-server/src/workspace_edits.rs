use std::fmt;
use std::path::Path;

use nocter_analysis::ValidatedSemanticMutation;
use nocter_filesystem::DocumentVersion;
use nocter_json::Value;
use nocter_lsp::{
    DocumentEdit, DocumentUri, DocumentUriError, Position, Range, TextEdit, workspace_edit_result,
};
use nocter_source::CoordinateError;

/// Projects compiler-owned byte edits as one versioned atomic LSP workspace edit.
///
/// # Errors
///
/// Returns URI or coordinate errors without changing editor state.
pub(crate) fn project_workspace_edit(
    mutation: ValidatedSemanticMutation<'_>,
) -> Result<Value, WorkspaceEditError> {
    let mut documents = Vec::new();
    for source_edits in mutation.into_source_edit_groups() {
        let source = source_edits.source();
        let path = Path::new(source.name().as_str());
        let uri = DocumentUri::from_file_path(path).map_err(WorkspaceEditError::Uri)?;
        let version = source_edits.document_version().map(DocumentVersion::get);
        let edits = source_edits
            .edits()
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

#[derive(Debug)]
pub enum WorkspaceEditError {
    Uri(DocumentUriError),
    Coordinate(CoordinateError),
}

impl fmt::Display for WorkspaceEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
        }
    }
}
