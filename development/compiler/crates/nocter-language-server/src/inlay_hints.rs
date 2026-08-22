use std::fmt;

use nocter_analysis::{SemanticInlayHintError, SemanticInlayHintKind};
use nocter_json::Value;
use nocter_lsp::{InlayHint, InlayHintKind, InlayHintParams, Position, inlay_hints_result};
use nocter_source::{CoordinateError, Utf16Position, Utf16Range};

use crate::semantic_document::{SemanticDocumentError, semantic_document};
use crate::{DocumentWorkspace, WorkspaceAnalyses};

/// Projects checked compiler-owned inlay facts into the requested UTF-16 source range.
pub(crate) fn query_inlay_hints(
    documents: &DocumentWorkspace,
    analyses: &WorkspaceAnalyses,
    params: &InlayHintParams,
) -> Result<Value, InlayHintQueryError> {
    let Some(document) = semantic_document(documents, analyses, params.uri())
        .map_err(InlayHintQueryError::Document)?
    else {
        return Ok(Value::Null);
    };
    let requested = document
        .source()
        .text_range(Utf16Range::new(
            Utf16Position::new(
                params.range().start().line(),
                params.range().start().character(),
            ),
            Utf16Position::new(
                params.range().end().line(),
                params.range().end().character(),
            ),
        ))
        .map_err(InlayHintQueryError::RequestCoordinate)?;
    let semantic = document
        .snapshot()
        .semantic_inlay_hints(document.source().id(), requested)
        .map_err(InlayHintQueryError::Semantic)?;
    let positions = semantic
        .iter()
        .map(|hint| {
            document
                .source()
                .utf16_position(hint.position())
                .map_err(InlayHintQueryError::ResultCoordinate)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let hints = semantic
        .iter()
        .zip(&positions)
        .map(|(hint, position)| {
            InlayHint::new(
                Position::new(position.line(), position.character()),
                hint.label(),
                hint_kind(hint.kind()),
            )
        })
        .collect::<Vec<_>>();
    Ok(inlay_hints_result(&hints))
}

const fn hint_kind(kind: SemanticInlayHintKind) -> Option<InlayHintKind> {
    match kind {
        SemanticInlayHintKind::Type => Some(InlayHintKind::Type),
        SemanticInlayHintKind::Provenance => None,
    }
}

#[derive(Debug)]
pub enum InlayHintQueryError {
    Document(SemanticDocumentError),
    RequestCoordinate(CoordinateError),
    ResultCoordinate(CoordinateError),
    Semantic(SemanticInlayHintError),
}

impl fmt::Display for InlayHintQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document(error) => error.fmt(formatter),
            Self::RequestCoordinate(error) | Self::ResultCoordinate(error) => error.fmt(formatter),
            Self::Semantic(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for InlayHintQueryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Document(error) => Some(error),
            Self::RequestCoordinate(error) | Self::ResultCoordinate(error) => Some(error),
            Self::Semantic(error) => Some(error),
        }
    }
}
