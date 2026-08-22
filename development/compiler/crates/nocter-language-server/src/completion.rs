use std::fmt;

use nocter_analysis::{SemanticCompletionError, SemanticCompletionKind};
use nocter_json::Value;
use nocter_lsp::{CompletionItem, CompletionItemKind, CompletionParams, completion_result};
use nocter_source::{CoordinateError, Utf16Position};

use crate::semantic_document::{SemanticDocumentError, semantic_document};
use crate::{DocumentWorkspace, WorkspaceAnalyses};

/// Answers lexical and module completion from the current compiler snapshot.
pub(crate) fn query_completion(
    documents: &DocumentWorkspace,
    analyses: &WorkspaceAnalyses,
    params: &CompletionParams,
) -> Result<Value, CompletionQueryError> {
    let Some(document) = semantic_document(documents, analyses, params.uri())
        .map_err(CompletionQueryError::Document)?
    else {
        return Ok(Value::Null);
    };
    let offset = document
        .source()
        .byte_offset(Utf16Position::new(
            params.position().line(),
            params.position().character(),
        ))
        .map_err(CompletionQueryError::Coordinate)?;
    let completions = document
        .snapshot()
        .semantic_completions(document.source().id(), offset)
        .map_err(CompletionQueryError::Semantic)?;
    let items = completions
        .iter()
        .map(|completion| {
            CompletionItem::new(
                completion.label(),
                item_kind(completion.kind()),
                completion.detail(),
            )
        })
        .collect::<Vec<_>>();
    Ok(completion_result(&items))
}

const fn item_kind(kind: SemanticCompletionKind) -> CompletionItemKind {
    match kind {
        SemanticCompletionKind::Module => CompletionItemKind::Module,
        SemanticCompletionKind::Struct => CompletionItemKind::Struct,
        SemanticCompletionKind::Enum => CompletionItemKind::Enum,
        SemanticCompletionKind::Type => CompletionItemKind::Class,
        SemanticCompletionKind::Interface => CompletionItemKind::Interface,
        SemanticCompletionKind::Function => CompletionItemKind::Function,
        SemanticCompletionKind::Constructor => CompletionItemKind::Constructor,
        SemanticCompletionKind::EnumMember => CompletionItemKind::EnumMember,
        SemanticCompletionKind::Field => CompletionItemKind::Field,
        SemanticCompletionKind::Method => CompletionItemKind::Method,
        SemanticCompletionKind::Parameter | SemanticCompletionKind::Variable => {
            CompletionItemKind::Variable
        }
    }
}

#[derive(Debug)]
pub enum CompletionQueryError {
    Document(SemanticDocumentError),
    Coordinate(CoordinateError),
    Semantic(SemanticCompletionError),
}

impl fmt::Display for CompletionQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document(error) => error.fmt(formatter),
            Self::Coordinate(error) => error.fmt(formatter),
            Self::Semantic(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CompletionQueryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Document(error) => Some(error),
            Self::Coordinate(error) => Some(error),
            Self::Semantic(error) => Some(error),
        }
    }
}
