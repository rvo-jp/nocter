use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nocter_analysis::{AnalysisSnapshot, GenerationId};
use nocter_compile_input::ModuleIdentity;
use nocter_discovery::{DiscoveryRequest, discover};
use nocter_filesystem::SourceOverlay;
use nocter_package::{
    PackageGraphError, PackageResolutionError, PackageResolutionPolicy, PackageResolutionRequest,
    resolve_package_selection_with_source_overlay, resolve_standard_package_with_source_overlay,
};
use nocter_session::bundled_standard_toolchain;

use crate::{AcceptedDocumentGeneration, WorkspaceConfiguration};

/// The compiler input boundary selected for one document generation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AnalysisScope {
    Package(PathBuf),
    SingleFile(PathBuf),
}

impl AnalysisScope {
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Package(path) | Self::SingleFile(path) => path,
        }
    }
}

/// The complete outcome retained for one accepted document generation.
#[derive(Debug)]
pub struct WorkspaceAnalysisGeneration {
    document: PathBuf,
    scope: Option<AnalysisScope>,
    generation: GenerationId,
    state: WorkspaceAnalysisState,
}

impl WorkspaceAnalysisGeneration {
    #[must_use]
    pub fn document(&self) -> &Path {
        &self.document
    }

    #[must_use]
    pub const fn scope(&self) -> Option<&AnalysisScope> {
        self.scope.as_ref()
    }

    #[must_use]
    pub const fn generation(&self) -> GenerationId {
        self.generation
    }

    #[must_use]
    pub const fn snapshot(&self) -> Option<&AnalysisSnapshot> {
        match &self.state {
            WorkspaceAnalysisState::Complete(snapshot) => Some(snapshot),
            WorkspaceAnalysisState::PreparationFailed { .. } => None,
        }
    }

    #[must_use]
    pub const fn preparation_failure(&self) -> Option<&WorkspaceAnalysisError> {
        match &self.state {
            WorkspaceAnalysisState::PreparationFailed { error, .. } => Some(error),
            WorkspaceAnalysisState::Complete(_) => None,
        }
    }

    #[must_use]
    pub const fn source_overlay(&self) -> &SourceOverlay {
        match &self.state {
            WorkspaceAnalysisState::Complete(snapshot) => snapshot.source_overlay(),
            WorkspaceAnalysisState::PreparationFailed { source_overlay, .. } => source_overlay,
        }
    }
}

#[derive(Debug)]
enum WorkspaceAnalysisState {
    Complete(Box<AnalysisSnapshot>),
    PreparationFailed {
        source_overlay: SourceOverlay,
        error: WorkspaceAnalysisError,
    },
}

/// Sequential owner of the latest immutable analysis for each package or standalone file.
#[derive(Debug)]
pub struct WorkspaceAnalyses {
    configuration: WorkspaceConfiguration,
    latest: BTreeMap<AnalysisScope, Arc<WorkspaceAnalysisGeneration>>,
    document_scopes: BTreeMap<PathBuf, AnalysisScope>,
    unscoped: BTreeMap<PathBuf, Arc<WorkspaceAnalysisGeneration>>,
}

impl WorkspaceAnalyses {
    #[must_use]
    pub fn new(configuration: WorkspaceConfiguration) -> Self {
        Self {
            configuration,
            latest: BTreeMap::new(),
            document_scopes: BTreeMap::new(),
            unscoped: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn configuration(&self) -> &WorkspaceConfiguration {
        &self.configuration
    }

    #[must_use]
    pub fn latest(&self, scope: &AnalysisScope) -> Option<&WorkspaceAnalysisGeneration> {
        self.latest.get(scope).map(Arc::as_ref)
    }

    #[must_use]
    pub fn latest_for_document(&self, document: &Path) -> Option<&WorkspaceAnalysisGeneration> {
        self.document_scopes
            .get(document)
            .and_then(|scope| self.latest.get(scope))
            .or_else(|| self.unscoped.get(document))
            .map(Arc::as_ref)
    }

    /// Selects one bounded package or single-file scope and runs its exact accepted overlay through
    /// locked, offline, read-only compiler preparation and target checking.
    pub fn analyze(
        &mut self,
        accepted: AcceptedDocumentGeneration,
    ) -> Arc<WorkspaceAnalysisGeneration> {
        let (document, source) = accepted.into_parts();
        let generation = source.generation();
        let source_overlay = source.into_source_overlay();
        let selected = select_scope(&self.configuration, &source_overlay, &document);
        let (scope, state) = match selected {
            Ok(scope) => {
                let state = compile_scope(&self.configuration, &scope, generation, source_overlay);
                (Some(scope), state)
            }
            Err(error) => (
                None,
                WorkspaceAnalysisState::PreparationFailed {
                    source_overlay,
                    error,
                },
            ),
        };
        let result = Arc::new(WorkspaceAnalysisGeneration {
            document,
            scope: scope.clone(),
            generation,
            state,
        });
        if let Some(previous) = self.document_scopes.remove(&result.document)
            && result.scope.as_ref() != Some(&previous)
        {
            self.latest.remove(&previous);
        }
        self.unscoped.remove(&result.document);
        match scope {
            Some(scope) => {
                self.document_scopes
                    .insert(result.document.clone(), scope.clone());
                self.latest.insert(scope, Arc::clone(&result));
            }
            None => {
                self.unscoped
                    .insert(result.document.clone(), Arc::clone(&result));
            }
        }
        result
    }
}

fn select_scope(
    configuration: &WorkspaceConfiguration,
    source_overlay: &SourceOverlay,
    document: &Path,
) -> Result<AnalysisScope, WorkspaceAnalysisError> {
    if document
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("nct")
    {
        return Err(WorkspaceAnalysisError::UnsupportedSource(
            document.to_path_buf(),
        ));
    }
    let workspace = configuration
        .root_for_document(document)
        .ok_or_else(|| WorkspaceAnalysisError::OutsideWorkspace(document.to_path_buf()))?;
    let mut directory = document
        .parent()
        .ok_or_else(|| WorkspaceAnalysisError::OutsideWorkspace(document.to_path_buf()))?;
    loop {
        let declaration = directory.join("nocter.nct");
        if source_overlay.is_file(&declaration).map_err(|error| {
            WorkspaceAnalysisError::Filesystem {
                operation: "inspect package declaration",
                path: declaration,
                error,
            }
        })? {
            return Ok(AnalysisScope::Package(directory.to_path_buf()));
        }
        if directory == workspace {
            break;
        }
        let Some(parent) = directory.parent() else {
            break;
        };
        directory = parent;
    }
    Ok(AnalysisScope::SingleFile(document.to_path_buf()))
}

fn compile_scope(
    configuration: &WorkspaceConfiguration,
    scope: &AnalysisScope,
    generation: GenerationId,
    source_overlay: SourceOverlay,
) -> WorkspaceAnalysisState {
    let discovered = match scope {
        AnalysisScope::Package(root) => {
            discover_package(configuration, root, source_overlay.clone())
        }
        AnalysisScope::SingleFile(source) => {
            discover_single_file(configuration, source, source_overlay.clone())
        }
    };
    match discovered {
        Ok(unit) => {
            WorkspaceAnalysisState::Complete(Box::new(AnalysisSnapshot::compile(generation, unit)))
        }
        Err(AnalysisPreparationFailure::Discovery(failure)) => {
            WorkspaceAnalysisState::Complete(Box::new(AnalysisSnapshot::from_discovery_failure(
                generation, failure,
            )))
        }
        Err(AnalysisPreparationFailure::Preparation(error)) => {
            WorkspaceAnalysisState::PreparationFailed {
                source_overlay,
                error,
            }
        }
    }
}

fn discover_package(
    configuration: &WorkspaceConfiguration,
    root: &Path,
    source_overlay: SourceOverlay,
) -> Result<nocter_discovery::DiscoveredUnit, AnalysisPreparationFailure> {
    let toolchain = configuration.toolchain();
    let selected = resolve_package_selection_with_source_overlay(
        PackageResolutionRequest::new(
            root,
            toolchain.nocter_home(),
            toolchain.standard().clone(),
            PackageResolutionPolicy::new(true, true),
        ),
        source_overlay,
    )
    .map_err(|error| AnalysisPreparationFailure::Preparation(error.into()))?;
    let root_package = selected.root().clone();
    let standard = selected.standard().clone();
    let package = selected
        .graph()
        .packages()
        .iter()
        .find(|package| package.identity() == &root_package)
        .ok_or_else(|| {
            AnalysisPreparationFailure::Preparation(WorkspaceAnalysisError::MissingRootPackage(
                root_package.clone(),
            ))
        })?;
    let mut roots = BTreeSet::new();
    roots.insert(ModuleIdentity::new(
        root_package.clone(),
        Vec::<Box<str>>::new(),
    ));
    if let Some(declaration) = package.declaration() {
        roots.extend(declaration.targets().iter().map(|target| {
            ModuleIdentity::new(root_package.clone(), target.module().iter().cloned())
        }));
    }
    let (packages, _, _) = selected.into_parts();
    discover(DiscoveryRequest::declared(
        toolchain.target(),
        packages,
        roots.into_iter().collect(),
        bundled_standard_toolchain(&standard),
    ))
    .map_err(AnalysisPreparationFailure::Discovery)
}

fn discover_single_file(
    configuration: &WorkspaceConfiguration,
    source: &Path,
    source_overlay: SourceOverlay,
) -> Result<nocter_discovery::DiscoveredUnit, AnalysisPreparationFailure> {
    let toolchain = configuration.toolchain();
    let standard = toolchain.standard().identity().clone();
    let packages =
        resolve_standard_package_with_source_overlay(toolchain.standard().clone(), source_overlay)
            .map_err(|error| AnalysisPreparationFailure::Preparation(error.into()))?;
    discover(DiscoveryRequest::single_file(
        toolchain.target(),
        source,
        packages,
        bundled_standard_toolchain(&standard),
    ))
    .map_err(AnalysisPreparationFailure::Discovery)
}

enum AnalysisPreparationFailure {
    Preparation(WorkspaceAnalysisError),
    Discovery(nocter_discovery::DiscoveryFailure),
}

#[derive(Debug)]
pub enum WorkspaceAnalysisError {
    OutsideWorkspace(PathBuf),
    UnsupportedSource(PathBuf),
    MissingRootPackage(nocter_model::PackageIdentity),
    Package(PackageResolutionError),
    StandardPackage(PackageGraphError),
    Filesystem {
        operation: &'static str,
        path: PathBuf,
        error: io::Error,
    },
}

impl From<PackageResolutionError> for WorkspaceAnalysisError {
    fn from(error: PackageResolutionError) -> Self {
        Self::Package(error)
    }
}

impl From<PackageGraphError> for WorkspaceAnalysisError {
    fn from(error: PackageGraphError) -> Self {
        Self::StandardPackage(error)
    }
}

impl fmt::Display for WorkspaceAnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideWorkspace(path) => write!(
                formatter,
                "document is outside every initialized workspace root: {}",
                path.display()
            ),
            Self::UnsupportedSource(path) => {
                write!(
                    formatter,
                    "document is not a Nocter source file: {}",
                    path.display()
                )
            }
            Self::MissingRootPackage(package) => write!(
                formatter,
                "resolved package graph is missing root package {}",
                package.as_str()
            ),
            Self::Package(error) => error.fmt(formatter),
            Self::StandardPackage(error) => {
                write!(formatter, "standard package is invalid: {error}")
            }
            Self::Filesystem {
                operation,
                path,
                error,
            } => write!(formatter, "cannot {operation} {}: {error}", path.display()),
        }
    }
}

impl std::error::Error for WorkspaceAnalysisError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Package(error) => Some(error),
            Self::StandardPackage(error) => Some(error),
            Self::Filesystem { error, .. } => Some(error),
            Self::OutsideWorkspace(_)
            | Self::UnsupportedSource(_)
            | Self::MissingRootPackage(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use nocter_analysis::AnalysisStatus;
    use nocter_json::parse;
    use nocter_lsp::{DidOpenParams, InitializeParams};
    use nocter_model::{CompilationTarget, PackageIdentity};
    use nocter_package::StandardPackage;

    use super::*;
    use crate::{DocumentWorkspace, LanguageServerEnvironment, LanguageServerToolchain};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn package_generation_uses_overlay_bytes_and_reaches_compiler_analysis() {
        let temporary = TemporaryDirectory::new();
        fs::write(temporary.path().join("nocter.nct"), "#name: \"app\"\n").unwrap();
        let source = temporary.path().join("index.nct");
        fs::write(&source, "pub func answer(): i32 { return 42 }\n").unwrap();
        let configuration = configuration(temporary.path());
        let mut documents = DocumentWorkspace::new();
        let accepted = documents
            .open(&open_params(
                &source,
                7,
                "pub func answer(): i32 { return }\n",
            ))
            .unwrap();
        let canonical_source = accepted.path().to_path_buf();
        let mut analyses = WorkspaceAnalyses::new(configuration);

        let analyzed = analyses.analyze(accepted);

        assert_eq!(analyzed.document(), canonical_source);
        assert!(matches!(analyzed.scope(), Some(AnalysisScope::Package(_))));
        let snapshot = analyzed.snapshot().expect("discovery reaches analysis");
        assert_eq!(snapshot.status(), AnalysisStatus::CompilationFailed);
        assert_eq!(
            snapshot
                .source_overlay()
                .document(&canonical_source)
                .unwrap()
                .bytes(),
            b"pub func answer(): i32 { return }\n"
        );
        assert_eq!(
            analyses
                .latest(analyzed.scope().unwrap())
                .unwrap()
                .generation(),
            analyzed.generation()
        );
    }

    #[test]
    fn source_without_a_bounded_manifest_uses_single_file_mode() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("standalone.nct");
        let configuration = configuration(temporary.path());
        let mut documents = DocumentWorkspace::new();
        let accepted = documents
            .open(&open_params(&source, 1, "func main(): void { return }\n"))
            .unwrap();
        let canonical_source = accepted.path().to_path_buf();

        let analyzed = WorkspaceAnalyses::new(configuration).analyze(accepted);

        assert_eq!(
            analyzed.scope(),
            Some(&AnalysisScope::SingleFile(canonical_source))
        );
        assert_eq!(
            analyzed.snapshot().unwrap().status(),
            AnalysisStatus::Complete
        );
    }

    fn configuration(root: &Path) -> WorkspaceConfiguration {
        let standard_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std");
        let environment = LanguageServerEnvironment::new(
            root,
            LanguageServerToolchain::new(
                CompilationTarget::Arm64Darwin,
                root,
                StandardPackage::new(PackageIdentity::new("toolchain:std"), standard_root),
            ),
        );
        let params = InitializeParams::decode(Some(
            parse(&format!(
                "{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}",
                root.display()
            ))
            .unwrap(),
        ))
        .unwrap();
        WorkspaceConfiguration::resolve(&environment, &params).unwrap()
    }

    fn open_params(path: &Path, version: i32, text: &str) -> DidOpenParams {
        let escaped = text.replace('\\', "\\\\").replace('\n', "\\n");
        DidOpenParams::decode(Some(
            parse(&format!(
                concat!(
                    "{{\"textDocument\":{{\"uri\":\"file://{}\",",
                    "\"languageId\":\"nocter\",\"version\":{},\"text\":\"{}\"}}}}"
                ),
                path.display(),
                version,
                escaped
            ))
            .unwrap(),
        ))
        .unwrap()
    }

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "nocter-language-server-analysis-{}-{id}",
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
