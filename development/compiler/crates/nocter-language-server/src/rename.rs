use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use nocter_analysis::{
    AnalysisSnapshot, SemanticRenameEdit, SemanticRenameError, SemanticRenamePlan,
};
use nocter_filesystem::{DocumentVersion, OpenDocument, SourceOverlay, SourceOverlayError};
use nocter_json::Value;
use nocter_lsp::{
    DocumentEdit, DocumentUri, DocumentUriError, Position, Range, RenameParams, TextEdit,
    workspace_edit_result,
};
use nocter_source::{CoordinateError, SourceId, Utf16Position};

use crate::semantic_document::{SemanticDocumentError, semantic_document};
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
    let overlay = candidate_overlay(document.snapshot(), &plan)?;
    let scope = document
        .analysis()
        .scope()
        .ok_or(RenameQueryError::MissingScope)?;
    let candidate = analyses
        .compile_candidate(scope, document.snapshot().generation(), overlay)
        .ok_or(RenameQueryError::CandidateRejected)?;
    if !document
        .snapshot()
        .validates_rename_candidate(&plan, &candidate)
    {
        return Err(RenameQueryError::CandidateRejected);
    }
    project_workspace_edit(document.snapshot(), &plan)
}

fn candidate_overlay(
    snapshot: &AnalysisSnapshot,
    plan: &SemanticRenamePlan,
) -> Result<SourceOverlay, RenameQueryError> {
    let grouped = grouped_edits(plan.edits());
    let mut documents = BTreeMap::new();
    for (path, document) in snapshot.source_overlay().documents() {
        documents.insert(path.to_path_buf(), document.clone());
    }
    for source in snapshot.sources().iter() {
        let path = PathBuf::from(source.name().as_str());
        let version = snapshot
            .document_version(&path)
            .unwrap_or(DocumentVersion::new(0));
        let mut text = source.text().to_owned();
        if let Some(edits) = grouped.get(&source.id()) {
            for edit in edits.iter().rev() {
                let start = usize::try_from(edit.range().start().get())
                    .map_err(|_| RenameQueryError::InvalidEdit(source.id()))?;
                let end = usize::try_from(edit.range().end().get())
                    .map_err(|_| RenameQueryError::InvalidEdit(source.id()))?;
                if !text.is_char_boundary(start) || !text.is_char_boundary(end) || start > end {
                    return Err(RenameQueryError::InvalidEdit(source.id()));
                }
                text.replace_range(start..end, plan.replacement());
            }
        }
        documents.insert(path, OpenDocument::new(version, text.into_bytes()));
    }
    let mut builder = SourceOverlay::builder();
    for (path, document) in documents {
        builder
            .insert(path, document)
            .map_err(RenameQueryError::Overlay)?;
    }
    Ok(builder.finish())
}

fn project_workspace_edit(
    snapshot: &AnalysisSnapshot,
    plan: &SemanticRenamePlan,
) -> Result<Value, RenameQueryError> {
    let mut documents = Vec::new();
    for (source_id, edits) in grouped_edits(plan.edits()) {
        let source = snapshot
            .sources()
            .get(source_id)
            .ok_or(RenameQueryError::MissingSource(source_id))?;
        let path = Path::new(source.name().as_str());
        let uri = DocumentUri::from_file_path(path).map_err(RenameQueryError::Uri)?;
        let version = snapshot.document_version(path).map(DocumentVersion::get);
        let edits = edits
            .iter()
            .map(|edit| {
                let range = source
                    .utf16_range(edit.range())
                    .map_err(RenameQueryError::Coordinate)?;
                Ok(TextEdit::new(
                    Range::new(
                        Position::new(range.start().line(), range.start().character()),
                        Position::new(range.end().line(), range.end().character()),
                    ),
                    plan.replacement(),
                ))
            })
            .collect::<Result<Vec<_>, RenameQueryError>>()?;
        documents.push(DocumentEdit::new(uri, version, edits));
    }
    Ok(workspace_edit_result(&documents))
}

fn grouped_edits(edits: &[SemanticRenameEdit]) -> BTreeMap<SourceId, Vec<SemanticRenameEdit>> {
    let mut grouped = BTreeMap::<_, Vec<_>>::new();
    for edit in edits {
        grouped.entry(edit.source()).or_default().push(*edit);
    }
    grouped
}

#[derive(Debug)]
pub enum RenameQueryError {
    Document(SemanticDocumentError),
    Coordinate(CoordinateError),
    Semantic(SemanticRenameError),
    MissingScope,
    MissingSource(SourceId),
    InvalidEdit(SourceId),
    Overlay(SourceOverlayError),
    Uri(DocumentUriError),
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
            Self::MissingSource(source) => write!(formatter, "rename source is missing: {source}"),
            Self::InvalidEdit(source) => write!(formatter, "rename edit is invalid in {source}"),
            Self::Overlay(error) => error.fmt(formatter),
            Self::Uri(error) => error.fmt(formatter),
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
            Self::Overlay(error) => Some(error),
            Self::Uri(error) => Some(error),
            Self::MissingScope
            | Self::MissingSource(_)
            | Self::InvalidEdit(_)
            | Self::CandidateRejected => None,
        }
    }
}
