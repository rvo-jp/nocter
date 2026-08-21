//! Immutable source overlays for one compiler or editor generation.
//!
//! This crate owns read-only source selection, not filesystem mutation. Package resolution and
//! module discovery receive the same [`SourceOverlay`], so an open document cannot be observed as
//! editor text by one phase and disk text by another.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

/// The editor version attached to one open document.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DocumentVersion(i32);

impl DocumentVersion {
    #[must_use]
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> i32 {
        self.0
    }
}

/// One immutable open-document value selected ahead of disk content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenDocument {
    version: DocumentVersion,
    bytes: Arc<[u8]>,
}

impl OpenDocument {
    #[must_use]
    pub fn new(version: DocumentVersion, bytes: impl Into<Arc<[u8]>>) -> Self {
        Self {
            version,
            bytes: bytes.into(),
        }
    }

    #[must_use]
    pub const fn version(&self) -> DocumentVersion {
        self.version
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// A read-only map of canonical absolute source paths to accepted editor versions.
///
/// Clones share the complete immutable map. Reads of paths absent from the map fall back to disk;
/// writes, fetches, and lock generation deliberately have no API here.
#[derive(Clone, Debug, Default)]
pub struct SourceOverlay {
    documents: Arc<BTreeMap<PathBuf, OpenDocument>>,
}

impl SourceOverlay {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn builder() -> SourceOverlayBuilder {
        SourceOverlayBuilder::new()
    }

    #[must_use]
    pub fn document(&self, canonical_path: &Path) -> Option<&OpenDocument> {
        self.documents.get(canonical_path)
    }

    /// Iterates the complete accepted document set in canonical path order.
    ///
    /// This is used to derive speculative read-only overlays without mutating the accepted editor
    /// generation or falling back to disk for another open document.
    pub fn documents(&self) -> impl Iterator<Item = (&Path, &OpenDocument)> {
        self.documents
            .iter()
            .map(|(path, document)| (path.as_path(), document))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Reads the accepted open-document bytes or falls back to the corresponding disk file.
    ///
    /// # Errors
    ///
    /// Returns the disk read error when no open document owns `path`.
    pub fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        if let Some(document) = self.documents.get(path) {
            return Ok(document.bytes().to_vec());
        }
        fs::read(path)
    }

    /// Reports whether `path` is an overlaid file or a regular disk file.
    ///
    /// # Errors
    ///
    /// Returns a disk metadata error other than absence when `path` is not overlaid.
    pub fn is_file(&self, path: &Path) -> io::Result<bool> {
        if self.documents.contains_key(path) {
            return Ok(true);
        }
        match fs::metadata(path) {
            Ok(metadata) => Ok(metadata.is_file()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Resolves a disk path, or accepts an exact canonical virtual-document path.
    ///
    /// # Errors
    ///
    /// Returns the disk canonicalization error when the path is neither present on disk nor an
    /// exact overlay key.
    pub fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        if self.documents.contains_key(path) {
            return Ok(path.to_path_buf());
        }
        fs::canonicalize(path)
    }
}

/// Consumed builder for one immutable source overlay.
#[derive(Debug, Default)]
pub struct SourceOverlayBuilder {
    documents: BTreeMap<PathBuf, OpenDocument>,
}

impl SourceOverlayBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one canonical absolute document path exactly once.
    ///
    /// # Errors
    ///
    /// Returns an error for a relative or lexically non-canonical path, or a duplicate document.
    pub fn insert(
        &mut self,
        canonical_path: impl Into<PathBuf>,
        document: OpenDocument,
    ) -> Result<(), SourceOverlayError> {
        let path = canonical_path.into();
        validate_canonical_path(&path)?;
        if self.documents.insert(path.clone(), document).is_some() {
            return Err(SourceOverlayError::DuplicatePath(path));
        }
        Ok(())
    }

    #[must_use]
    pub fn finish(self) -> SourceOverlay {
        SourceOverlay {
            documents: Arc::new(self.documents),
        }
    }
}

fn validate_canonical_path(path: &Path) -> Result<(), SourceOverlayError> {
    if !path.is_absolute() {
        return Err(SourceOverlayError::RelativePath(path.to_path_buf()));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(SourceOverlayError::NonCanonicalPath(path.to_path_buf()));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceOverlayError {
    RelativePath(PathBuf),
    NonCanonicalPath(PathBuf),
    DuplicatePath(PathBuf),
}

impl fmt::Display for SourceOverlayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RelativePath(path) => {
                write!(
                    formatter,
                    "source overlay path is relative: {}",
                    path.display()
                )
            }
            Self::NonCanonicalPath(path) => write!(
                formatter,
                "source overlay path is not lexically canonical: {}",
                path.display()
            ),
            Self::DuplicatePath(path) => write!(
                formatter,
                "source overlay contains the path more than once: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for SourceOverlayError {}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{DocumentVersion, OpenDocument, SourceOverlay, SourceOverlayError};

    #[test]
    fn open_document_bytes_and_version_override_disk_as_one_value() {
        let directory = TemporaryDirectory::new();
        let path = directory.path().join("index.nct");
        fs::write(&path, b"disk").unwrap();
        let path = fs::canonicalize(path).unwrap();
        let mut builder = SourceOverlay::builder();
        builder
            .insert(
                path.clone(),
                OpenDocument::new(DocumentVersion::new(7), &b"editor"[..]),
            )
            .unwrap();
        let overlay = builder.finish();

        assert_eq!(overlay.read(&path).unwrap(), b"editor");
        assert_eq!(
            overlay.document(&path).unwrap().version(),
            DocumentVersion::new(7)
        );
        assert!(overlay.is_file(&path).unwrap());
        assert_eq!(overlay.canonicalize(&path).unwrap(), path);
    }

    #[test]
    fn unopened_paths_fall_back_to_disk_and_virtual_files_are_visible() {
        let directory = TemporaryDirectory::new();
        let disk = directory.path().join("disk.nct");
        fs::write(&disk, b"disk").unwrap();
        let disk = fs::canonicalize(disk).unwrap();
        let virtual_path = directory.path().join("virtual.nct");
        let mut builder = SourceOverlay::builder();
        builder
            .insert(
                virtual_path.clone(),
                OpenDocument::new(DocumentVersion::new(1), &b"virtual"[..]),
            )
            .unwrap();
        let overlay = builder.finish();

        assert_eq!(overlay.read(&disk).unwrap(), b"disk");
        assert_eq!(overlay.read(&virtual_path).unwrap(), b"virtual");
        assert!(overlay.is_file(&virtual_path).unwrap());
        assert_eq!(overlay.canonicalize(&virtual_path).unwrap(), virtual_path);
    }

    #[test]
    fn builder_rejects_ambiguous_and_duplicate_path_identity() {
        let mut builder = SourceOverlay::builder();
        assert!(matches!(
            builder.insert(
                "relative.nct",
                OpenDocument::new(DocumentVersion::new(1), &b"one"[..])
            ),
            Err(SourceOverlayError::RelativePath(_))
        ));
        let path = PathBuf::from("/tmp/nocter-overlay.nct");
        builder
            .insert(
                path.clone(),
                OpenDocument::new(DocumentVersion::new(1), &b"one"[..]),
            )
            .unwrap();
        assert!(matches!(
            builder.insert(
                path,
                OpenDocument::new(DocumentVersion::new(2), &b"two"[..])
            ),
            Err(SourceOverlayError::DuplicatePath(_))
        ));
    }

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "nocter-filesystem-{}-{}",
                std::process::id(),
                std::thread::current().name().unwrap_or("test")
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
