use std::fmt;
use std::path::{Path, PathBuf};

use nocter_lsp::{DocumentUriError, InitializeParams};

use nocter_workspace_analysis::{
    WorkspaceConfiguration, WorkspaceToolchain as LanguageServerToolchain,
};

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

/// Resolves LSP workspace folders, the legacy root URI, or the process directory into the single
/// protocol-independent workspace configuration consumed by analysis. Duplicate physical roots
/// collapse inside the protocol-independent configuration without changing the authored order of
/// distinct roots.
///
/// # Errors
///
/// Returns a URI or workspace-configuration failure without partially initializing the server.
pub(crate) fn resolve_workspace_configuration(
    environment: &LanguageServerEnvironment,
    params: &InitializeParams,
) -> Result<WorkspaceConfiguration, WorkspaceConfigurationError> {
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

    WorkspaceConfiguration::resolve(candidates, environment.toolchain().clone())
        .map_err(WorkspaceConfigurationError::Analysis)
}

#[derive(Debug)]
pub enum WorkspaceConfigurationError {
    Uri(DocumentUriError),
    Analysis(nocter_workspace_analysis::WorkspaceConfigurationError),
}

impl fmt::Display for WorkspaceConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uri(error) => error.fmt(formatter),
            Self::Analysis(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for WorkspaceConfigurationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Uri(error) => Some(error),
            Self::Analysis(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use nocter_json::parse;
    use nocter_model::{CompilationTarget, PackageIdentity};
    use nocter_package::StandardPackage;

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
            resolve_workspace_configuration(&environment(temporary.path()), &params).unwrap();

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
            resolve_workspace_configuration(&environment(temporary.path()), &params).unwrap();
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
            resolve_workspace_configuration(&environment(temporary.path()), &params).unwrap_err();
        assert!(matches!(error, WorkspaceConfigurationError::Analysis(_)));
    }

    fn environment(current_directory: &Path) -> LanguageServerEnvironment {
        LanguageServerEnvironment::new(
            current_directory,
            LanguageServerToolchain::new(
                CompilationTarget::Arm64Darwin,
                current_directory,
                StandardPackage::new(
                    PackageIdentity::new("toolchain:std"),
                    current_directory,
                    "0.0.0",
                ),
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
