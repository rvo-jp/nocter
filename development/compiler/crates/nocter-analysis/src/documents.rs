use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nocter_filesystem::{DocumentVersion, OpenDocument, SourceOverlay, SourceOverlayError};

use crate::GenerationId;

/// Immutable source view produced by one accepted workspace transition.
#[derive(Clone, Debug)]
pub struct AcceptedSourceGeneration {
    generation: GenerationId,
    source_overlay: SourceOverlay,
}

impl AcceptedSourceGeneration {
    #[must_use]
    pub const fn generation(&self) -> GenerationId {
        self.generation
    }

    #[must_use]
    pub const fn source_overlay(&self) -> &SourceOverlay {
        &self.source_overlay
    }

    #[must_use]
    pub fn into_source_overlay(self) -> SourceOverlay {
        self.source_overlay
    }
}

/// Outcome of one versioned document change.
#[derive(Clone, Debug)]
pub enum DocumentChange {
    Accepted(AcceptedSourceGeneration),
    IgnoredStale { current: DocumentVersion },
}

/// Mutable protocol-independent owner of accepted open documents.
///
/// Mutation stays here. Every accepted transition emits a complete immutable overlay suitable for
/// exactly one [`crate::AnalysisSnapshot`].
#[derive(Debug, Default)]
pub struct WorkspaceDocuments {
    generation: u64,
    documents: BTreeMap<PathBuf, OpenDocument>,
}

impl WorkspaceDocuments {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn current_generation(&self) -> GenerationId {
        GenerationId::new(self.generation)
    }

    #[must_use]
    pub fn document(&self, canonical_path: &Path) -> Option<&OpenDocument> {
        self.documents.get(canonical_path)
    }

    /// Accepts a newly opened canonical source path.
    ///
    /// # Errors
    ///
    /// Returns an error for an already open path, invalid canonical path, or generation overflow.
    pub fn open(
        &mut self,
        canonical_path: impl Into<PathBuf>,
        version: DocumentVersion,
        bytes: impl Into<Arc<[u8]>>,
    ) -> Result<AcceptedSourceGeneration, DocumentStateError> {
        let path = canonical_path.into();
        if self.documents.contains_key(&path) {
            return Err(DocumentStateError::AlreadyOpen(path));
        }
        let mut candidate = self.documents.clone();
        candidate.insert(path, OpenDocument::new(version, bytes));
        self.accept(candidate)
    }

    /// Accepts a strictly newer full-document version or ignores a stale version.
    ///
    /// # Errors
    ///
    /// Returns an error for a path that is not open or generation overflow.
    pub fn change(
        &mut self,
        canonical_path: &Path,
        version: DocumentVersion,
        bytes: impl Into<Arc<[u8]>>,
    ) -> Result<DocumentChange, DocumentStateError> {
        let Some(current) = self.documents.get(canonical_path) else {
            return Err(DocumentStateError::NotOpen(canonical_path.to_path_buf()));
        };
        if version <= current.version() {
            return Ok(DocumentChange::IgnoredStale {
                current: current.version(),
            });
        }
        let mut candidate = self.documents.clone();
        candidate.insert(
            canonical_path.to_path_buf(),
            OpenDocument::new(version, bytes),
        );
        self.accept(candidate).map(DocumentChange::Accepted)
    }

    /// Accepts a save notification, applying included text before emitting its generation.
    ///
    /// The document version remains the last accepted change version because LSP save
    /// notifications carry no new version.
    ///
    /// # Errors
    ///
    /// Returns an error for a path that is not open or generation overflow.
    pub fn save(
        &mut self,
        canonical_path: &Path,
        bytes: Option<Arc<[u8]>>,
    ) -> Result<AcceptedSourceGeneration, DocumentStateError> {
        let Some(current) = self.documents.get(canonical_path) else {
            return Err(DocumentStateError::NotOpen(canonical_path.to_path_buf()));
        };
        let mut candidate = self.documents.clone();
        if let Some(bytes) = bytes {
            candidate.insert(
                canonical_path.to_path_buf(),
                OpenDocument::new(current.version(), bytes),
            );
        }
        self.accept(candidate)
    }

    /// Closes one document and emits a generation that falls back to disk for that path.
    ///
    /// # Errors
    ///
    /// Returns an error for a path that is not open or generation overflow.
    pub fn close(
        &mut self,
        canonical_path: &Path,
    ) -> Result<AcceptedSourceGeneration, DocumentStateError> {
        if !self.documents.contains_key(canonical_path) {
            return Err(DocumentStateError::NotOpen(canonical_path.to_path_buf()));
        }
        let mut candidate = self.documents.clone();
        candidate.remove(canonical_path);
        self.accept(candidate)
    }

    /// Emits a new generation for an external filesystem change while preserving every accepted
    /// open-document override.
    ///
    /// # Errors
    ///
    /// Returns an error when the generation identity space is exhausted.
    pub fn refresh(&mut self) -> Result<AcceptedSourceGeneration, DocumentStateError> {
        self.accept(self.documents.clone())
    }

    fn accept(
        &mut self,
        candidate: BTreeMap<PathBuf, OpenDocument>,
    ) -> Result<AcceptedSourceGeneration, DocumentStateError> {
        let next = self
            .generation
            .checked_add(1)
            .ok_or(DocumentStateError::GenerationExhausted)?;
        let mut builder = SourceOverlay::builder();
        for (path, document) in &candidate {
            builder
                .insert(path.clone(), document.clone())
                .map_err(DocumentStateError::InvalidPath)?;
        }
        let source_overlay = builder.finish();
        self.documents = candidate;
        self.generation = next;
        Ok(AcceptedSourceGeneration {
            generation: GenerationId::new(next),
            source_overlay,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentStateError {
    AlreadyOpen(PathBuf),
    NotOpen(PathBuf),
    InvalidPath(SourceOverlayError),
    GenerationExhausted,
}

impl fmt::Display for DocumentStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyOpen(path) => {
                write!(formatter, "document is already open: {}", path.display())
            }
            Self::NotOpen(path) => write!(formatter, "document is not open: {}", path.display()),
            Self::InvalidPath(error) => error.fmt(formatter),
            Self::GenerationExhausted => {
                formatter.write_str("editor generation identity space is exhausted")
            }
        }
    }
}

impl std::error::Error for DocumentStateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPath(error) => Some(error),
            Self::AlreadyOpen(_) | Self::NotOpen(_) | Self::GenerationExhausted => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use nocter_filesystem::DocumentVersion;

    use super::{DocumentChange, DocumentStateError, WorkspaceDocuments};
    use crate::GenerationId;

    const PATH: &str = "/workspace/index.nct";

    #[test]
    fn accepted_transitions_freeze_independent_monotonic_generations() {
        let mut documents = WorkspaceDocuments::new();
        let first = documents
            .open(PATH, DocumentVersion::new(1), &b"first"[..])
            .unwrap();
        let second = match documents
            .change(Path::new(PATH), DocumentVersion::new(2), &b"second"[..])
            .unwrap()
        {
            DocumentChange::Accepted(generation) => generation,
            DocumentChange::IgnoredStale { .. } => panic!("newer version must be accepted"),
        };

        assert_eq!(first.generation(), GenerationId::new(1));
        assert_eq!(second.generation(), GenerationId::new(2));
        assert_eq!(
            first
                .source_overlay()
                .document(Path::new(PATH))
                .unwrap()
                .bytes(),
            b"first"
        );
        assert_eq!(
            second
                .source_overlay()
                .document(Path::new(PATH))
                .unwrap()
                .bytes(),
            b"second"
        );
    }

    #[test]
    fn stale_change_is_ignored_without_advancing_the_generation() {
        let mut documents = WorkspaceDocuments::new();
        documents
            .open(PATH, DocumentVersion::new(3), &b"current"[..])
            .unwrap();

        let change = documents
            .change(Path::new(PATH), DocumentVersion::new(2), &b"stale"[..])
            .unwrap();

        assert!(matches!(
            change,
            DocumentChange::IgnoredStale { current }
                if current == DocumentVersion::new(3)
        ));
        assert_eq!(documents.current_generation(), GenerationId::new(1));
        assert_eq!(
            documents.document(Path::new(PATH)).unwrap().bytes(),
            b"current"
        );
    }

    #[test]
    fn included_save_text_precedes_its_generation_and_close_removes_the_override() {
        let mut documents = WorkspaceDocuments::new();
        documents
            .open(PATH, DocumentVersion::new(5), &b"open"[..])
            .unwrap();
        let saved = documents
            .save(Path::new(PATH), Some(Arc::from(&b"saved"[..])))
            .unwrap();
        let closed = documents.close(Path::new(PATH)).unwrap();

        let saved_document = saved.source_overlay().document(Path::new(PATH)).unwrap();
        assert_eq!(saved_document.version(), DocumentVersion::new(5));
        assert_eq!(saved_document.bytes(), b"saved");
        assert_eq!(saved.generation(), GenerationId::new(2));
        assert!(closed.source_overlay().is_empty());
        assert_eq!(closed.generation(), GenerationId::new(3));
    }

    #[test]
    fn rejected_transition_does_not_mutate_the_workspace() {
        let mut documents = WorkspaceDocuments::new();
        assert!(matches!(
            documents.open("relative.nct", DocumentVersion::new(1), &b"text"[..]),
            Err(DocumentStateError::InvalidPath(_))
        ));
        assert_eq!(documents.current_generation(), GenerationId::new(0));
        assert!(documents.document(Path::new("relative.nct")).is_none());
    }

    #[test]
    fn external_refresh_advances_generation_without_replacing_open_bytes() {
        let mut documents = WorkspaceDocuments::new();
        documents
            .open(PATH, DocumentVersion::new(1), &b"editor"[..])
            .unwrap();

        let refreshed = documents.refresh().unwrap();

        assert_eq!(refreshed.generation(), GenerationId::new(2));
        assert_eq!(
            refreshed
                .source_overlay()
                .document(Path::new(PATH))
                .unwrap()
                .bytes(),
            b"editor"
        );
    }
}
