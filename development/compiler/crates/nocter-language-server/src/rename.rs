use std::fmt;

use nocter_analysis::{SemanticRenameError, SemanticSourceEdit};
use nocter_json::Value;
use nocter_lsp::RenameParams;
use nocter_source::{CoordinateError, Utf16Position};

use crate::semantic_document::{SemanticDocumentError, semantic_document};
use crate::workspace_edits::{WorkspaceEditError, candidate_overlay, project_workspace_edit};
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
    let edits = plan
        .edits()
        .iter()
        .map(|edit| SemanticSourceEdit::new(edit.source(), edit.range(), plan.replacement()))
        .collect::<Vec<_>>();
    let overlay =
        candidate_overlay(document.snapshot(), &edits).map_err(RenameQueryError::WorkspaceEdit)?;
    let scope = document
        .analysis()
        .scope()
        .ok_or(RenameQueryError::MissingScope)?;
    let candidate = analyses
        .compile_candidate(
            scope,
            document.path(),
            document.snapshot().generation(),
            overlay,
        )
        .ok_or(RenameQueryError::CandidateRejected)?;
    let capability = document
        .snapshot()
        .validate_rename_candidate(&plan, &candidate)
        .map_err(nocter_analysis::SemanticRenameError::from)
        .map_err(RenameQueryError::Semantic)?
        .ok_or(RenameQueryError::CandidateRejected)?;
    project_workspace_edit(capability, document.snapshot(), &edits)
        .map_err(RenameQueryError::WorkspaceEdit)
}

#[derive(Debug)]
pub enum RenameQueryError {
    Document(SemanticDocumentError),
    Coordinate(CoordinateError),
    Semantic(SemanticRenameError),
    MissingScope,
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
            Self::MissingScope => formatter.write_str("rename document has no analysis scope"),
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
            Self::WorkspaceEdit(error) => Some(error),
            Self::MissingScope | Self::CandidateRejected => None,
        }
    }
}
