use std::fmt;
use std::path::PathBuf;

use nocter_json::Value;
use nocter_lsp::{HoverParams, Position, Range, hover_result};
use nocter_source::{CoordinateError, Utf16Position};

use crate::{DocumentPathError, DocumentPathResolver, DocumentWorkspace, WorkspaceAnalyses};

/// Answers one hover query from the latest immutable generation that contains its document.
pub(crate) fn query_hover(
    documents: &DocumentWorkspace,
    analyses: &WorkspaceAnalyses,
    params: &HoverParams,
) -> Result<Value, HoverQueryError> {
    let path = match documents.path(params.uri()) {
        Some(path) => path.to_path_buf(),
        None => DocumentPathResolver::new()
            .resolve(params.uri())
            .map_err(HoverQueryError::Path)?,
    };
    let Some(generation) = analyses.latest_for_document(&path) else {
        return Ok(Value::Null);
    };
    let Some(snapshot) = generation.snapshot() else {
        return Ok(Value::Null);
    };
    let name = path
        .to_str()
        .ok_or_else(|| HoverQueryError::NonUtf8Path(path.clone()))?;
    let Some(source) = snapshot.sources().find_by_name(name) else {
        return Ok(Value::Null);
    };
    let position = params.position();
    let offset = source
        .byte_offset(Utf16Position::new(position.line(), position.character()))
        .map_err(HoverQueryError::Coordinate)?;
    let Some(subject) = snapshot.semantic_subject(source.id(), offset) else {
        return Ok(Value::Null);
    };
    let range = source
        .utf16_range(subject.range())
        .map_err(HoverQueryError::Coordinate)?;
    Ok(hover_result(
        subject.presentation().code(),
        Range::new(position_value(range.start()), position_value(range.end())),
    ))
}

fn position_value(position: Utf16Position) -> Position {
    Position::new(position.line(), position.character())
}

#[derive(Debug)]
pub enum HoverQueryError {
    Path(DocumentPathError),
    NonUtf8Path(PathBuf),
    Coordinate(CoordinateError),
}

impl fmt::Display for HoverQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(error) => error.fmt(formatter),
            Self::NonUtf8Path(path) => {
                write!(
                    formatter,
                    "document path is not valid UTF-8: {}",
                    path.display()
                )
            }
            Self::Coordinate(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for HoverQueryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Path(error) => Some(error),
            Self::Coordinate(error) => Some(error),
            Self::NonUtf8Path(_) => None,
        }
    }
}
