use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Installation-owned compiler facts used by every workspace generation.
#[derive(Clone, Debug)]
pub struct WorkspaceToolchain {
    target: nocter_model::CompilationTarget,
    nocter_home: PathBuf,
    standard: nocter_package::StandardPackage,
}

impl WorkspaceToolchain {
    #[must_use]
    pub fn new(
        target: nocter_model::CompilationTarget,
        nocter_home: impl Into<PathBuf>,
        standard: nocter_package::StandardPackage,
    ) -> Self {
        Self {
            target,
            nocter_home: nocter_home.into(),
            standard,
        }
    }

    #[must_use]
    pub(crate) const fn target(&self) -> nocter_model::CompilationTarget {
        self.target
    }

    #[must_use]
    pub(crate) fn nocter_home(&self) -> &Path {
        &self.nocter_home
    }

    #[must_use]
    pub(crate) const fn standard(&self) -> &nocter_package::StandardPackage {
        &self.standard
    }
}

/// Canonical workspace roots and exact toolchain facts for protocol-independent analysis.
#[derive(Clone, Debug)]
pub struct WorkspaceConfiguration {
    roots: Box<[PathBuf]>,
    toolchain: WorkspaceToolchain,
}

impl WorkspaceConfiguration {
    /// Canonicalizes and validates the complete set of workspace roots once.
    ///
    /// Duplicate physical roots collapse without changing the authored order of distinct roots.
    ///
    /// # Errors
    ///
    /// Returns the first filesystem or non-directory failure without publishing a partial
    /// configuration.
    pub fn resolve(
        roots: impl IntoIterator<Item = PathBuf>,
        toolchain: WorkspaceToolchain,
    ) -> Result<Self, WorkspaceConfigurationError> {
        let mut seen = BTreeSet::new();
        let mut canonical_roots = Vec::new();
        for root in roots {
            let canonical = fs::canonicalize(&root)
                .map_err(|error| WorkspaceConfigurationError::filesystem(root, error))?;
            if !canonical.is_dir() {
                return Err(WorkspaceConfigurationError::not_directory(canonical));
            }
            if seen.insert(canonical.clone()) {
                canonical_roots.push(canonical);
            }
        }
        Ok(Self {
            roots: canonical_roots.into_boxed_slice(),
            toolchain,
        })
    }

    #[must_use]
    pub const fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    #[must_use]
    pub(crate) const fn toolchain(&self) -> &WorkspaceToolchain {
        &self.toolchain
    }

    #[must_use]
    pub fn root_for_document(&self, document: &Path) -> Option<&Path> {
        self.roots
            .iter()
            .filter(|root| document.starts_with(root))
            .max_by_key(|root| root.components().count())
            .map(PathBuf::as_path)
    }
}

/// A workspace root that could not enter the canonical analysis configuration.
#[derive(Debug)]
pub struct WorkspaceConfigurationError {
    kind: WorkspaceConfigurationErrorKind,
}

#[derive(Debug)]
enum WorkspaceConfigurationErrorKind {
    Filesystem { path: PathBuf, error: io::Error },
    NotDirectory(PathBuf),
}

impl WorkspaceConfigurationError {
    fn filesystem(path: PathBuf, error: io::Error) -> Self {
        Self {
            kind: WorkspaceConfigurationErrorKind::Filesystem { path, error },
        }
    }

    fn not_directory(path: PathBuf) -> Self {
        Self {
            kind: WorkspaceConfigurationErrorKind::NotDirectory(path),
        }
    }
}

impl fmt::Display for WorkspaceConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            WorkspaceConfigurationErrorKind::Filesystem { path, error } => write!(
                formatter,
                "cannot canonicalize workspace root {}: {error}",
                path.display()
            ),
            WorkspaceConfigurationErrorKind::NotDirectory(path) => write!(
                formatter,
                "workspace root is not a directory: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for WorkspaceConfigurationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            WorkspaceConfigurationErrorKind::Filesystem { error, .. } => Some(error),
            WorkspaceConfigurationErrorKind::NotDirectory(_) => None,
        }
    }
}
