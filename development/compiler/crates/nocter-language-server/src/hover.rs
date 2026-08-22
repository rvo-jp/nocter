use std::fmt;

use nocter_analysis::SemanticQueryError;
use nocter_json::Value;
use nocter_lsp::{HoverParams, Position, Range, hover_result};
use nocter_source::{CoordinateError, Utf16Position};

use crate::semantic_document::{SemanticDocumentError, semantic_document};
use crate::{DocumentWorkspace, WorkspaceAnalyses};

/// Answers one hover query from the latest immutable generation that contains its document.
pub(crate) fn query_hover(
    documents: &DocumentWorkspace,
    analyses: &WorkspaceAnalyses,
    params: &HoverParams,
) -> Result<Value, HoverQueryError> {
    let Some(document) =
        semantic_document(documents, analyses, params.uri()).map_err(HoverQueryError::Document)?
    else {
        return Ok(Value::Null);
    };
    let snapshot = document.snapshot();
    let source = document.source();
    let position = params.position();
    let offset = source
        .byte_offset(Utf16Position::new(position.line(), position.character()))
        .map_err(HoverQueryError::Coordinate)?;
    let Some(subject) = snapshot
        .semantic_subject(source.id(), offset)
        .map_err(HoverQueryError::Semantic)?
    else {
        return Ok(Value::Null);
    };
    let range = source
        .utf16_range(subject.range())
        .map_err(HoverQueryError::Coordinate)?;
    Ok(hover_result(
        subject.presentation().code(),
        subject.documentation(),
        Range::new(position_value(range.start()), position_value(range.end())),
    ))
}

fn position_value(position: Utf16Position) -> Position {
    Position::new(position.line(), position.character())
}

#[derive(Debug)]
pub enum HoverQueryError {
    Document(SemanticDocumentError),
    Coordinate(CoordinateError),
    Semantic(SemanticQueryError),
}

impl fmt::Display for HoverQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document(error) => error.fmt(formatter),
            Self::Coordinate(error) => error.fmt(formatter),
            Self::Semantic(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for HoverQueryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Document(error) => Some(error),
            Self::Coordinate(error) => Some(error),
            Self::Semantic(error) => Some(error),
        }
    }
}
