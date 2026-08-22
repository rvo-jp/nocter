use std::fmt;

use nocter_analysis::SemanticHighlightKind;
use nocter_json::Value;
use nocter_lsp::{
    SemanticToken, SemanticTokenEncodingError, SemanticTokenType, SemanticTokensParams,
    semantic_tokens_result,
};
use nocter_source::CoordinateError;

use crate::semantic_document::{SemanticDocumentError, semantic_document};
use crate::{DocumentWorkspace, WorkspaceAnalyses};

/// Projects compiler-classified semantic ranges into one full-document LSP token result.
pub(crate) fn query_semantic_tokens(
    documents: &DocumentWorkspace,
    analyses: &WorkspaceAnalyses,
    params: &SemanticTokensParams,
) -> Result<Value, SemanticTokensQueryError> {
    let Some(document) = semantic_document(documents, analyses, params.uri())
        .map_err(SemanticTokensQueryError::Document)?
    else {
        return Ok(Value::Null);
    };
    let source = document.source();
    let tokens = document
        .snapshot()
        .semantic_highlights(source.id())
        .iter()
        .copied()
        .map(|highlight| {
            let range = source
                .utf16_range(highlight.range())
                .map_err(SemanticTokensQueryError::Coordinate)?;
            if range.start().line() != range.end().line() {
                return Err(SemanticTokensQueryError::Multiline);
            }
            Ok(SemanticToken::new(
                range.start().line(),
                range.start().character(),
                range.end().character() - range.start().character(),
                token_type(highlight.kind()),
                highlight.is_declaration(),
                highlight.is_readonly(),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result_id = document.analysis().generation().get().to_string();
    semantic_tokens_result(&result_id, &tokens).map_err(SemanticTokensQueryError::Encoding)
}

const fn token_type(kind: SemanticHighlightKind) -> SemanticTokenType {
    match kind {
        SemanticHighlightKind::Namespace => SemanticTokenType::Namespace,
        SemanticHighlightKind::Type => SemanticTokenType::Type,
        SemanticHighlightKind::Struct => SemanticTokenType::Struct,
        SemanticHighlightKind::Enum => SemanticTokenType::Enum,
        SemanticHighlightKind::Interface => SemanticTokenType::Interface,
        SemanticHighlightKind::TypeParameter => SemanticTokenType::TypeParameter,
        SemanticHighlightKind::Parameter => SemanticTokenType::Parameter,
        SemanticHighlightKind::Variable => SemanticTokenType::Variable,
        SemanticHighlightKind::Property => SemanticTokenType::Property,
        SemanticHighlightKind::EnumMember => SemanticTokenType::EnumMember,
        SemanticHighlightKind::Function => SemanticTokenType::Function,
        SemanticHighlightKind::Method => SemanticTokenType::Method,
        SemanticHighlightKind::Keyword => SemanticTokenType::Keyword,
    }
}

#[derive(Debug)]
pub enum SemanticTokensQueryError {
    Document(SemanticDocumentError),
    Coordinate(CoordinateError),
    Multiline,
    Encoding(SemanticTokenEncodingError),
}

impl fmt::Display for SemanticTokensQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document(error) => error.fmt(formatter),
            Self::Coordinate(error) => error.fmt(formatter),
            Self::Multiline => formatter.write_str("semantic token range spans multiple lines"),
            Self::Encoding(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SemanticTokensQueryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Document(error) => Some(error),
            Self::Coordinate(error) => Some(error),
            Self::Encoding(error) => Some(error),
            Self::Multiline => None,
        }
    }
}
