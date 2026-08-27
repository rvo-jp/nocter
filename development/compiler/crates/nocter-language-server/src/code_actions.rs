use std::fmt;

use nocter_analysis::{
    EvidenceIntegrityError, SemanticCodeActionError, SemanticMutationBuildError,
};
use nocter_json::Value;
use nocter_lsp::{CodeAction, CodeActionParams, code_actions_result};
use nocter_source::{CoordinateError, Utf16Position, Utf16Range};

use crate::semantic_document::{SemanticDocumentError, semantic_document};
use crate::workspace_edits::{WorkspaceEditError, project_workspace_edit};
use crate::{DocumentWorkspace, WorkspaceAnalyses};

/// Plans, recompiles, and projects all valid quick fixes in the requested source range.
pub(crate) fn query_code_actions(
    documents: &DocumentWorkspace,
    analyses: &WorkspaceAnalyses,
    params: &CodeActionParams,
) -> Result<Value, CodeActionQueryError> {
    if !params.quick_fixes_requested() {
        return Ok(Value::Array(Vec::new()));
    }
    let Some(document) = semantic_document(documents, analyses, params.uri())
        .map_err(CodeActionQueryError::Document)?
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
        .map_err(CodeActionQueryError::RequestCoordinate)?;
    let planned = document
        .snapshot()
        .semantic_code_actions(document.source().id(), requested)
        .map_err(CodeActionQueryError::Semantic)?;
    let mut validated = Vec::new();
    for action in &planned {
        let candidate = action
            .candidate(document.snapshot())
            .map_err(CodeActionQueryError::Mutation)?;
        let Some(mutation) = analyses
            .validate_candidate(document.analysis(), candidate)
            .map_err(CodeActionQueryError::CandidateEvidence)?
        else {
            continue;
        };
        let edit = project_workspace_edit(mutation).map_err(CodeActionQueryError::WorkspaceEdit)?;
        validated.push((action, edit));
    }
    let preferred = validated.len() == 1;
    let actions = validated
        .iter()
        .map(|(action, edit)| CodeAction::new(action.title(), edit, preferred))
        .collect::<Vec<_>>();
    Ok(code_actions_result(&actions))
}

#[derive(Debug)]
pub enum CodeActionQueryError {
    Document(SemanticDocumentError),
    RequestCoordinate(CoordinateError),
    Semantic(SemanticCodeActionError),
    Mutation(SemanticMutationBuildError),
    CandidateEvidence(EvidenceIntegrityError),
    WorkspaceEdit(WorkspaceEditError),
}

impl CodeActionQueryError {
    #[must_use]
    pub const fn is_request_error(&self) -> bool {
        matches!(self, Self::Document(_) | Self::RequestCoordinate(_))
    }
}

impl fmt::Display for CodeActionQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document(error) => error.fmt(formatter),
            Self::RequestCoordinate(error) => error.fmt(formatter),
            Self::Semantic(error) => error.fmt(formatter),
            Self::Mutation(error) => error.fmt(formatter),
            Self::CandidateEvidence(error) => error.fmt(formatter),
            Self::WorkspaceEdit(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CodeActionQueryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Document(error) => Some(error),
            Self::RequestCoordinate(error) => Some(error),
            Self::Semantic(error) => Some(error),
            Self::Mutation(error) => Some(error),
            Self::CandidateEvidence(error) => Some(error),
            Self::WorkspaceEdit(error) => Some(error),
        }
    }
}
