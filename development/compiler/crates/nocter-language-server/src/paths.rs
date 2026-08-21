use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use nocter_lsp::{DocumentUri, DocumentUriError};

/// Resolves one protocol document identity to a canonical local path.
#[derive(Clone, Copy, Debug, Default)]
pub struct DocumentPathResolver;

impl DocumentPathResolver {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Resolves an existing file, or a virtual file whose parent directory exists.
    ///
    /// # Errors
    ///
    /// Returns URI decoding, filesystem, non-file, or missing-filename failure.
    pub fn resolve(self, uri: &DocumentUri) -> Result<PathBuf, DocumentPathError> {
        let path = uri.file_path().map_err(DocumentPathError::Uri)?;
        match fs::canonicalize(&path) {
            Ok(canonical) => {
                if canonical.is_file() {
                    Ok(canonical)
                } else {
                    Err(DocumentPathError::NotFile(canonical))
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => virtual_path(&path),
            Err(error) => Err(DocumentPathError::Filesystem {
                operation: "canonicalize document",
                path,
                error,
            }),
        }
    }
}

fn virtual_path(path: &Path) -> Result<PathBuf, DocumentPathError> {
    let name = path
        .file_name()
        .ok_or_else(|| DocumentPathError::MissingFileName(path.to_path_buf()))?;
    let parent = path
        .parent()
        .ok_or_else(|| DocumentPathError::MissingFileName(path.to_path_buf()))?;
    let parent = fs::canonicalize(parent).map_err(|error| DocumentPathError::Filesystem {
        operation: "canonicalize virtual document parent",
        path: parent.to_path_buf(),
        error,
    })?;
    if !parent.is_dir() {
        return Err(DocumentPathError::ParentNotDirectory(parent));
    }
    Ok(parent.join(name))
}

#[derive(Debug)]
pub enum DocumentPathError {
    Uri(DocumentUriError),
    Filesystem {
        operation: &'static str,
        path: PathBuf,
        error: io::Error,
    },
    NotFile(PathBuf),
    ParentNotDirectory(PathBuf),
    MissingFileName(PathBuf),
}

impl fmt::Display for DocumentPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uri(error) => error.fmt(formatter),
            Self::Filesystem {
                operation,
                path,
                error,
            } => write!(formatter, "cannot {operation} {}: {error}", path.display()),
            Self::NotFile(path) => {
                write!(formatter, "document path is not a file: {}", path.display())
            }
            Self::ParentNotDirectory(path) => write!(
                formatter,
                "virtual document parent is not a directory: {}",
                path.display()
            ),
            Self::MissingFileName(path) => write!(
                formatter,
                "document path has no file name: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for DocumentPathError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Uri(error) => Some(error),
            Self::Filesystem { error, .. } => Some(error),
            Self::NotFile(_) | Self::ParentNotDirectory(_) | Self::MissingFileName(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn resolves_existing_and_virtual_documents_through_canonical_parents() {
        let temporary = TemporaryDirectory::new();
        let existing = temporary.path().join("existing.nct");
        fs::write(&existing, b"").unwrap();

        let existing_uri = file_uri(&existing);
        assert_eq!(
            DocumentPathResolver::new().resolve(&existing_uri).unwrap(),
            fs::canonicalize(&existing).unwrap()
        );

        let virtual_path = temporary.path().join("virtual.nct");
        let virtual_uri = file_uri(&virtual_path);
        assert_eq!(
            DocumentPathResolver::new().resolve(&virtual_uri).unwrap(),
            fs::canonicalize(temporary.path())
                .unwrap()
                .join("virtual.nct")
        );
    }

    fn file_uri(path: &Path) -> DocumentUri {
        DocumentUri::new(format!("file://{}", path.display())).unwrap()
    }

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "nocter-language-server-{}-{id}",
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
