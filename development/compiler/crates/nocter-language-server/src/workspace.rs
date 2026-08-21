use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use nocter_lsp::{DocumentUriError, InitializeParams};
use nocter_model::CompilationTarget;
use nocter_package::StandardPackage;

/// Immutable compiler and process facts supplied by the validated executable boundary.
#[derive(Clone, Debug)]
pub struct LanguageServerEnvironment {
    current_directory: PathBuf,
    toolchain: LanguageServerToolchain,
}

impl LanguageServerEnvironment {
    #[must_use]
    pub fn new(current_directory: impl Into<PathBuf>, toolchain: LanguageServerToolchain) -> Self {
        Self {
            current_directory: current_directory.into(),
            toolchain,
        }
    }

    #[must_use]
    pub fn current_directory(&self) -> &Path {
        &self.current_directory
    }

    #[must_use]
    pub const fn toolchain(&self) -> &LanguageServerToolchain {
        &self.toolchain
    }
}

/// Exact installation-owned inputs used by every editor analysis generation.
#[derive(Clone, Debug)]
pub struct LanguageServerToolchain {
    target: CompilationTarget,
    nocter_home: PathBuf,
    standard: StandardPackage,
}

impl LanguageServerToolchain {
    #[must_use]
    pub fn new(
        target: CompilationTarget,
        nocter_home: impl Into<PathBuf>,
        standard: StandardPackage,
    ) -> Self {
        Self {
            target,
            nocter_home: nocter_home.into(),
            standard,
        }
    }

    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    #[must_use]
    pub fn nocter_home(&self) -> &Path {
        &self.nocter_home
    }

    #[must_use]
    pub const fn standard(&self) -> &StandardPackage {
        &self.standard
    }
}

/// Canonical workspace folders selected once by a successful initialize request.
#[derive(Clone, Debug)]
pub struct WorkspaceConfiguration {
    roots: Box<[PathBuf]>,
    toolchain: LanguageServerToolchain,
}

impl WorkspaceConfiguration {
    /// Resolves LSP workspace folders, the legacy root URI, or the process directory in that order.
    /// Duplicate physical roots collapse without changing the authored order of distinct roots.
    ///
    /// # Errors
    ///
    /// Returns a URI, filesystem, or non-directory failure without partially initializing the
    /// server.
    pub fn resolve(
        environment: &LanguageServerEnvironment,
        params: &InitializeParams,
    ) -> Result<Self, WorkspaceConfigurationError> {
        let candidates = if !params.workspace_folders().is_empty() {
            params
                .workspace_folders()
                .iter()
                .map(|folder| {
                    folder
                        .uri()
                        .file_path()
                        .map_err(WorkspaceConfigurationError::Uri)
                })
                .collect::<Result<Vec<_>, _>>()?
        } else if let Some(uri) = params.root_uri() {
            vec![uri.file_path().map_err(WorkspaceConfigurationError::Uri)?]
        } else {
            vec![environment.current_directory().to_path_buf()]
        };

        let mut seen = BTreeSet::new();
        let mut roots = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let canonical = fs::canonicalize(&candidate).map_err(|error| {
                WorkspaceConfigurationError::Filesystem {
                    path: candidate,
                    error,
                }
            })?;
            if !canonical.is_dir() {
                return Err(WorkspaceConfigurationError::NotDirectory(canonical));
            }
            if seen.insert(canonical.clone()) {
                roots.push(canonical);
            }
        }
        Ok(Self {
            roots: roots.into_boxed_slice(),
            toolchain: environment.toolchain().clone(),
        })
    }

    #[must_use]
    pub const fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    #[must_use]
    pub const fn toolchain(&self) -> &LanguageServerToolchain {
        &self.toolchain
    }

    /// Selects the deepest initialized workspace root containing a canonical document path.
    /// Nested workspace folders therefore retain independent ownership regardless of client order.
    #[must_use]
    pub fn root_for_document(&self, document: &Path) -> Option<&Path> {
        self.roots
            .iter()
            .filter(|root| document.starts_with(root))
            .max_by_key(|root| root.components().count())
            .map(PathBuf::as_path)
    }
}

#[derive(Debug)]
pub enum WorkspaceConfigurationError {
    Uri(DocumentUriError),
    Filesystem { path: PathBuf, error: io::Error },
    NotDirectory(PathBuf),
}

impl fmt::Display for WorkspaceConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uri(error) => error.fmt(formatter),
            Self::Filesystem { path, error } => write!(
                formatter,
                "cannot canonicalize language-server workspace root {}: {error}",
                path.display()
            ),
            Self::NotDirectory(path) => write!(
                formatter,
                "language-server workspace root is not a directory: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for WorkspaceConfigurationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Uri(error) => Some(error),
            Self::Filesystem { error, .. } => Some(error),
            Self::NotDirectory(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use nocter_json::parse;
    use nocter_model::PackageIdentity;

    use super::*;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn workspace_folders_override_fallback_and_physical_duplicates_collapse() {
        let temporary = TemporaryDirectory::new();
        let nested = temporary.path().join("nested");
        fs::create_dir(&nested).unwrap();
        let params = InitializeParams::decode(Some(
            parse(&format!(
                concat!(
                    "{{\"rootUri\":\"{}\",\"workspaceFolders\":[",
                    "{{\"uri\":\"{}\",\"name\":\"outer\"}},",
                    "{{\"uri\":\"{}\",\"name\":\"duplicate\"}},",
                    "{{\"uri\":\"{}\",\"name\":\"nested\"}}],",
                    "\"capabilities\":{{}}}}"
                ),
                file_uri(Path::new("/ignored")),
                file_uri(temporary.path()),
                file_uri(temporary.path()),
                file_uri(&nested),
            ))
            .unwrap(),
        ))
        .unwrap();
        let workspace =
            WorkspaceConfiguration::resolve(&environment(temporary.path()), &params).unwrap();

        assert_eq!(workspace.roots().len(), 2);
        let canonical_nested = fs::canonicalize(&nested).unwrap();
        assert_eq!(
            workspace.root_for_document(&canonical_nested.join("file.nct")),
            Some(canonical_nested.as_path())
        );
    }

    #[test]
    fn falls_back_to_the_process_directory_and_rejects_file_roots() {
        let temporary = TemporaryDirectory::new();
        let params =
            InitializeParams::decode(Some(parse(r#"{"capabilities":{}}"#).unwrap())).unwrap();
        let workspace =
            WorkspaceConfiguration::resolve(&environment(temporary.path()), &params).unwrap();
        assert_eq!(
            workspace.roots(),
            &[fs::canonicalize(temporary.path()).unwrap()]
        );

        let source = temporary.path().join("file.nct");
        fs::write(&source, b"").unwrap();
        let params = InitializeParams::decode(Some(
            parse(&format!(
                "{{\"rootUri\":\"{}\",\"capabilities\":{{}}}}",
                file_uri(&source)
            ))
            .unwrap(),
        ))
        .unwrap();
        let error =
            WorkspaceConfiguration::resolve(&environment(temporary.path()), &params).unwrap_err();
        assert!(matches!(
            error,
            WorkspaceConfigurationError::NotDirectory(_)
        ));
    }

    fn environment(current_directory: &Path) -> LanguageServerEnvironment {
        LanguageServerEnvironment::new(
            current_directory,
            LanguageServerToolchain::new(
                CompilationTarget::Arm64Darwin,
                current_directory,
                StandardPackage::new(PackageIdentity::new("toolchain:std"), current_directory),
            ),
        )
    }

    fn file_uri(path: &Path) -> String {
        format!("file://{}", path.display())
    }

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "nocter-language-server-workspace-{}-{id}",
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
