use std::fmt;
use std::path::PathBuf;

use nocter_analysis::AnalysisSnapshot;
use nocter_lsp::DocumentUri;
use nocter_source::SourceFile;

use crate::{
    DocumentPathError, DocumentPathResolver, DocumentWorkspace, WorkspaceAnalyses,
    WorkspaceAnalysisGeneration,
};

/// One current successful analysis paired with the exact source requested by an editor query.
pub(crate) struct SemanticDocument<'a> {
    analysis: &'a WorkspaceAnalysisGeneration,
    snapshot: &'a AnalysisSnapshot,
    source: &'a SourceFile,
}

impl<'a> SemanticDocument<'a> {
    pub(crate) const fn analysis(&self) -> &'a WorkspaceAnalysisGeneration {
        self.analysis
    }

    pub(crate) const fn snapshot(&self) -> &'a AnalysisSnapshot {
        self.snapshot
    }

    pub(crate) const fn source(&self) -> &'a SourceFile {
        self.source
    }
}

/// Resolves a URI through stable document identity into one current successful source snapshot.
pub(crate) fn semantic_document<'a>(
    documents: &DocumentWorkspace,
    analyses: &'a WorkspaceAnalyses,
    uri: &DocumentUri,
) -> Result<Option<SemanticDocument<'a>>, SemanticDocumentError> {
    let path = match documents.path(uri) {
        Some(path) => path.to_path_buf(),
        None => DocumentPathResolver::new()
            .resolve(uri)
            .map_err(SemanticDocumentError::Path)?,
    };
    let Some(generation) = analyses.latest_for_document(&path) else {
        return Ok(None);
    };
    let Some(snapshot) = generation.snapshot() else {
        return Ok(None);
    };
    let name = path
        .to_str()
        .ok_or_else(|| SemanticDocumentError::NonUtf8Path(path.clone()))?;
    Ok(snapshot
        .sources()
        .find_by_name(name)
        .map(|source| SemanticDocument {
            analysis: generation,
            snapshot,
            source,
        }))
}

#[derive(Debug)]
pub enum SemanticDocumentError {
    Path(DocumentPathError),
    NonUtf8Path(PathBuf),
}

impl fmt::Display for SemanticDocumentError {
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
        }
    }
}

impl std::error::Error for SemanticDocumentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Path(error) => Some(error),
            Self::NonUtf8Path(_) => None,
        }
    }
}
