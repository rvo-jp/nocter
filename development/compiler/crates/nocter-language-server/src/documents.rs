use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nocter_analysis::{
    AcceptedSourceGeneration, DocumentChange, DocumentStateError, WorkspaceDocuments,
};
use nocter_filesystem::DocumentVersion;
use nocter_lsp::{DidChangeParams, DidCloseParams, DidOpenParams, DidSaveParams, DocumentUri};

use crate::{DocumentPathError, DocumentPathResolver};

/// One accepted source generation paired with the stable document identity that triggered it.
#[derive(Clone, Debug)]
pub struct AcceptedDocumentGeneration {
    path: PathBuf,
    source: AcceptedSourceGeneration,
}

impl AcceptedDocumentGeneration {
    fn new(path: PathBuf, source: AcceptedSourceGeneration) -> Self {
        Self { path, source }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn generation(&self) -> nocter_analysis::GenerationId {
        self.source.generation()
    }

    #[must_use]
    pub const fn source_overlay(&self) -> &nocter_filesystem::SourceOverlay {
        self.source.source_overlay()
    }

    #[must_use]
    pub fn into_parts(self) -> (PathBuf, AcceptedSourceGeneration) {
        (self.path, self.source)
    }
}

/// A document change either advances analysis or is ignored by the version gate.
#[derive(Clone, Debug)]
pub enum DocumentWorkspaceChange {
    Accepted(AcceptedDocumentGeneration),
    IgnoredStale { current: DocumentVersion },
}

/// Owns stable URI-to-path identities and accepted document generations for one server process.
#[derive(Debug, Default)]
pub struct DocumentWorkspace {
    paths: BTreeMap<DocumentUri, PathBuf>,
    documents: WorkspaceDocuments,
}

impl DocumentWorkspace {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn path(&self, uri: &DocumentUri) -> Option<&Path> {
        self.paths.get(uri).map(PathBuf::as_path)
    }

    /// Opens one URI and freezes its canonical path for every later notification.
    ///
    /// # Errors
    ///
    /// Returns duplicate URI, URI/path resolution, or document-state failure.
    pub fn open(
        &mut self,
        params: &DidOpenParams,
    ) -> Result<AcceptedDocumentGeneration, DocumentWorkspaceError> {
        if self.paths.contains_key(params.uri()) {
            return Err(DocumentWorkspaceError::AlreadyOpenUri(params.uri().clone()));
        }
        let path = DocumentPathResolver::new()
            .resolve(params.uri())
            .map_err(DocumentWorkspaceError::Path)?;
        let generation = self
            .documents
            .open(
                path.clone(),
                DocumentVersion::new(params.version()),
                Arc::<[u8]>::from(params.text().as_bytes()),
            )
            .map_err(DocumentWorkspaceError::State)?;
        self.paths.insert(params.uri().clone(), path.clone());
        Ok(AcceptedDocumentGeneration::new(path, generation))
    }

    /// Applies one strictly newer full-document replacement to the open URI identity.
    ///
    /// # Errors
    ///
    /// Returns unknown URI or document-state failure.
    pub fn change(
        &mut self,
        params: &DidChangeParams,
    ) -> Result<DocumentWorkspaceChange, DocumentWorkspaceError> {
        let path = self.require_path(params.uri())?.to_path_buf();
        self.documents
            .change(
                &path,
                DocumentVersion::new(params.version()),
                Arc::<[u8]>::from(params.text().as_bytes()),
            )
            .map(|change| match change {
                DocumentChange::Accepted(source) => {
                    DocumentWorkspaceChange::Accepted(AcceptedDocumentGeneration::new(path, source))
                }
                DocumentChange::IgnoredStale { current } => {
                    DocumentWorkspaceChange::IgnoredStale { current }
                }
            })
            .map_err(DocumentWorkspaceError::State)
    }

    /// Applies included saved text before freezing the save generation.
    ///
    /// # Errors
    ///
    /// Returns unknown URI or document-state failure.
    pub fn save(
        &mut self,
        params: &DidSaveParams,
    ) -> Result<AcceptedDocumentGeneration, DocumentWorkspaceError> {
        let path = self.require_path(params.uri())?.to_path_buf();
        let bytes = params.text().map(|text| Arc::<[u8]>::from(text.as_bytes()));
        self.documents
            .save(&path, bytes)
            .map(|source| AcceptedDocumentGeneration::new(path, source))
            .map_err(DocumentWorkspaceError::State)
    }

    /// Closes one URI after emitting its disk-fallback generation.
    ///
    /// # Errors
    ///
    /// Returns unknown URI or document-state failure.
    pub fn close(
        &mut self,
        params: &DidCloseParams,
    ) -> Result<AcceptedDocumentGeneration, DocumentWorkspaceError> {
        let path = self.require_path(params.uri())?.to_path_buf();
        let generation = self
            .documents
            .close(&path)
            .map_err(DocumentWorkspaceError::State)?;
        self.paths.remove(params.uri());
        Ok(AcceptedDocumentGeneration::new(path, generation))
    }

    fn require_path(&self, uri: &DocumentUri) -> Result<&Path, DocumentWorkspaceError> {
        self.path(uri)
            .ok_or_else(|| DocumentWorkspaceError::UnknownUri(uri.clone()))
    }
}

#[derive(Debug)]
pub enum DocumentWorkspaceError {
    AlreadyOpenUri(DocumentUri),
    UnknownUri(DocumentUri),
    Path(DocumentPathError),
    State(DocumentStateError),
}

impl fmt::Display for DocumentWorkspaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyOpenUri(uri) => {
                write!(formatter, "document URI is already open: {}", uri.as_str())
            }
            Self::UnknownUri(uri) => {
                write!(formatter, "document URI is not open: {}", uri.as_str())
            }
            Self::Path(error) => error.fmt(formatter),
            Self::State(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DocumentWorkspaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Path(error) => Some(error),
            Self::State(error) => Some(error),
            Self::AlreadyOpenUri(_) | Self::UnknownUri(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use nocter_analysis::GenerationId;
    use nocter_json::parse;

    use super::*;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn freezes_one_path_identity_across_the_complete_document_lifecycle() {
        let temporary = TemporaryDirectory::new();
        let path = temporary.path().join("new.nct");
        let uri = format!("file://{}", path.display());
        let mut workspace = DocumentWorkspace::new();

        let opened = workspace.open(&open_params(&uri, 1, "first")).unwrap();
        assert_eq!(opened.generation(), GenerationId::new(1));
        let canonical = workspace
            .path(&DocumentUri::new(uri.clone()).unwrap())
            .unwrap()
            .to_path_buf();
        assert_eq!(
            opened
                .source_overlay()
                .document(&canonical)
                .unwrap()
                .bytes(),
            b"first"
        );

        let stale = workspace.change(&change_params(&uri, 1, "stale")).unwrap();
        assert!(matches!(
            stale,
            DocumentWorkspaceChange::IgnoredStale { .. }
        ));
        let changed = workspace.change(&change_params(&uri, 2, "second")).unwrap();
        let DocumentWorkspaceChange::Accepted(changed) = changed else {
            panic!("newer version must be accepted")
        };
        assert_eq!(changed.generation(), GenerationId::new(2));

        let saved = workspace.save(&save_params(&uri, Some("saved"))).unwrap();
        assert_eq!(saved.generation(), GenerationId::new(3));
        assert_eq!(
            saved.source_overlay().document(&canonical).unwrap().bytes(),
            b"saved"
        );

        let closed = workspace.close(&close_params(&uri)).unwrap();
        assert_eq!(closed.generation(), GenerationId::new(4));
        assert!(closed.source_overlay().document(&canonical).is_none());
    }

    fn open_params(uri: &str, version: i32, text: &str) -> DidOpenParams {
        DidOpenParams::decode(Some(
            parse(&format!(
                "{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":{version},\"text\":\"{text}\"}}}}"
            ))
            .unwrap(),
        ))
        .unwrap()
    }

    fn change_params(uri: &str, version: i32, text: &str) -> DidChangeParams {
        DidChangeParams::decode(Some(
            parse(&format!(
                "{{\"textDocument\":{{\"uri\":\"{uri}\",\"version\":{version}}},\"contentChanges\":[{{\"text\":\"{text}\"}}]}}"
            ))
            .unwrap(),
        ))
        .unwrap()
    }

    fn save_params(uri: &str, text: Option<&str>) -> DidSaveParams {
        let text = text.map_or_else(String::new, |text| format!(",\"text\":\"{text}\""));
        DidSaveParams::decode(Some(
            parse(&format!("{{\"textDocument\":{{\"uri\":\"{uri}\"}}{text}}}")).unwrap(),
        ))
        .unwrap()
    }

    fn close_params(uri: &str) -> DidCloseParams {
        DidCloseParams::decode(Some(
            parse(&format!("{{\"textDocument\":{{\"uri\":\"{uri}\"}}}}")).unwrap(),
        ))
        .unwrap()
    }

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "nocter-document-workspace-{}-{id}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
