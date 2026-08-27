use std::fmt;

use nocter_analysis::{SemanticMutationBuildError, SemanticRenameError};
use nocter_json::Value;
use nocter_lsp::RenameParams;
use nocter_source::{CoordinateError, Utf16Position};

use crate::semantic_document::{SemanticDocumentError, semantic_document};
use crate::workspace_edits::{WorkspaceEditError, project_workspace_edit};
use crate::{DocumentWorkspace, WorkspaceAnalyses};

/// Plans and validates one atomic semantic rename.
pub(crate) fn query_rename(
    documents: &DocumentWorkspace,
    analyses: &WorkspaceAnalyses,
    params: &RenameParams,
) -> Result<Value, RenameQueryError> {
    let Some(document) =
        semantic_document(documents, analyses, params.uri()).map_err(RenameQueryError::Document)?
    else {
        return Ok(Value::Null);
    };
    let offset = document
        .source()
        .byte_offset(Utf16Position::new(
            params.position().line(),
            params.position().character(),
        ))
        .map_err(RenameQueryError::Coordinate)?;
    let Some(plan) = document
        .snapshot()
        .semantic_rename(document.source().id(), offset, params.new_name())
        .map_err(RenameQueryError::Semantic)?
    else {
        return Ok(Value::Null);
    };
    let candidate = document
        .snapshot()
        .semantic_rename_candidate(plan)
        .map_err(RenameQueryError::Mutation)?;
    let mutation = analyses
        .validate_candidate(document.analysis(), candidate)
        .map_err(nocter_analysis::SemanticRenameError::from)
        .map_err(RenameQueryError::Semantic)?
        .ok_or(RenameQueryError::CandidateRejected)?;
    project_workspace_edit(mutation).map_err(RenameQueryError::WorkspaceEdit)
}

#[derive(Debug)]
pub enum RenameQueryError {
    Document(SemanticDocumentError),
    Coordinate(CoordinateError),
    Semantic(SemanticRenameError),
    Mutation(SemanticMutationBuildError),
    WorkspaceEdit(WorkspaceEditError),
    CandidateRejected,
}

impl RenameQueryError {
    #[must_use]
    pub const fn is_request_error(&self) -> bool {
        matches!(
            self,
            Self::Document(_)
                | Self::Coordinate(_)
                | Self::Semantic(
                    SemanticRenameError::InvalidReplacement(_)
                        | SemanticRenameError::ReadOnlyOccurrence(_),
                )
                | Self::CandidateRejected
        )
    }
}

impl fmt::Display for RenameQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document(error) => error.fmt(formatter),
            Self::Coordinate(error) => error.fmt(formatter),
            Self::Semantic(error) => error.fmt(formatter),
            Self::Mutation(error) => error.fmt(formatter),
            Self::WorkspaceEdit(error) => error.fmt(formatter),
            Self::CandidateRejected => {
                formatter.write_str("rename would collide with or rebind an existing declaration")
            }
        }
    }
}

impl std::error::Error for RenameQueryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Document(error) => Some(error),
            Self::Coordinate(error) => Some(error),
            Self::Semantic(error) => Some(error),
            Self::Mutation(error) => Some(error),
            Self::WorkspaceEdit(error) => Some(error),
            Self::CandidateRejected => None,
        }
    }
}
