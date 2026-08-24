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
    PackageGraphError, PackageResolutionFailure, PackageResolutionPolicy, PackageResolutionRequest,
    resolve_package_selection_with_source_snapshot, resolve_standard_package_with_source_overlay,
};
use nocter_session::bundled_standard_toolchain;

use crate::{AcceptedDocumentGeneration, WorkspaceConfiguration};

/// The compiler input boundary selected for one document generation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AnalysisScope {
    Package(PathBuf),
    ToolchainStandard(PathBuf),
    SingleFile(PathBuf),
}

impl AnalysisScope {
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Package(path) | Self::ToolchainStandard(path) | Self::SingleFile(path) => path,
        }
    }
}

/// The complete outcome retained for one accepted document generation.
#[derive(Debug)]
pub struct WorkspaceAnalysisGeneration {
    document: PathBuf,
    scope: Option<AnalysisScope>,
    invalidated: Box<[AnalysisScope]>,
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
    pub const fn invalidated_scopes(&self) -> &[AnalysisScope] {
        &self.invalidated
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

    #[must_use]
    pub fn reached_sources(&self) -> Option<&nocter_source::SourceMap> {
        match &self.state {
            WorkspaceAnalysisState::Complete(snapshot) => Some(snapshot.sources()),
            WorkspaceAnalysisState::PreparationFailed {
                error: WorkspaceAnalysisError::Package(failure),
                ..
            } => Some(failure.reached().sources()),
            WorkspaceAnalysisState::PreparationFailed { .. } => None,
        }
    }

    #[must_use]
    pub fn reached_syntax_trees(&self) -> &[nocter_syntax::SyntaxTree] {
        match &self.state {
            WorkspaceAnalysisState::Complete(snapshot) => snapshot.syntax_trees(),
            WorkspaceAnalysisState::PreparationFailed {
                error: WorkspaceAnalysisError::Package(failure),
                ..
            } => failure.reached().syntax_trees(),
            WorkspaceAnalysisState::PreparationFailed { .. } => &[],
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

/// Sequential owner of the latest immutable analysis for each package, toolchain standard, or
/// standalone file.
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
            .or_else(|| {
                let name = document.to_str()?;
                self.latest
                    .values()
                    .filter(|generation| {
                        generation
                            .reached_sources()
                            .is_some_and(|sources| sources.find_by_name(name).is_some())
                    })
                    .max_by_key(|generation| generation.generation())
            })
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
        let mut invalidated = Vec::new();
        if let Some(previous) = self.document_scopes.remove(&document)
            && scope.as_ref() != Some(&previous)
        {
            self.latest.remove(&previous);
            invalidated.push(previous);
        }
        let result = Arc::new(WorkspaceAnalysisGeneration {
            document,
            scope: scope.clone(),
            invalidated: invalidated.into_boxed_slice(),
            generation,
            state,
        });
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

    /// Compiles a speculative overlay without publishing or replacing an accepted generation.
    ///
    /// Mutation features use this as a transaction preflight. The candidate travels through the
    /// same package resolution, discovery, and compiler pipeline as accepted editor state.
    pub(crate) fn compile_candidate(
        &self,
        scope: &AnalysisScope,
        generation: GenerationId,
        source_overlay: SourceOverlay,
    ) -> Option<Box<AnalysisSnapshot>> {
        match compile_scope(&self.configuration, scope, generation, source_overlay) {
            WorkspaceAnalysisState::Complete(snapshot) => Some(snapshot),
            WorkspaceAnalysisState::PreparationFailed { .. } => None,
        }
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
    let standard_root = configuration.toolchain().standard().root();
    if document.starts_with(standard_root) {
        return Ok(AnalysisScope::ToolchainStandard(
            standard_root.to_path_buf(),
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
        AnalysisScope::ToolchainStandard(_) => {
            discover_toolchain_standard(configuration, source_overlay.clone())
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

fn discover_toolchain_standard(
    configuration: &WorkspaceConfiguration,
    source_overlay: SourceOverlay,
) -> Result<nocter_discovery::DiscoveredUnit, AnalysisPreparationFailure> {
    let toolchain = configuration.toolchain();
    let standard = toolchain.standard().identity().clone();
    let package =
        resolve_standard_package_with_source_overlay(toolchain.standard().clone(), source_overlay)
            .map_err(|error| AnalysisPreparationFailure::Preparation(error.into()))?;
    discover(DiscoveryRequest::toolchain_standard(
        toolchain.target(),
        package,
        bundled_standard_toolchain(&standard),
    ))
    .map_err(AnalysisPreparationFailure::Discovery)
}

fn discover_package(
    configuration: &WorkspaceConfiguration,
    root: &Path,
    source_overlay: SourceOverlay,
) -> Result<nocter_discovery::DiscoveredUnit, AnalysisPreparationFailure> {
    let toolchain = configuration.toolchain();
    let selected = resolve_package_selection_with_source_snapshot(
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
    Package(PackageResolutionFailure),
    StandardPackage(PackageGraphError),
    Filesystem {
        operation: &'static str,
        path: PathBuf,
        error: io::Error,
    },
}

impl WorkspaceAnalysisError {
    #[must_use]
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::OutsideWorkspace(_) | Self::UnsupportedSource(_) | Self::Filesystem { .. } => {
                "E0702"
            }
            Self::Package(_) => "E0800",
            Self::StandardPackage(_) => "E0703",
            Self::MissingRootPackage(_) => "E0900",
        }
    }
}

impl From<PackageResolutionFailure> for WorkspaceAnalysisError {
    fn from(error: PackageResolutionFailure) -> Self {
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

    #[test]
    fn toolchain_standard_inside_workspace_keeps_its_selected_identity() {
        let standard_root = standard_root();
        let workspace_root = standard_root.parent().unwrap();
        let source = standard_root.join("error/index.nct");
        let text = fs::read_to_string(&source).unwrap();
        let configuration = configuration_with_standard(workspace_root, &standard_root);
        let mut documents = DocumentWorkspace::new();
        let accepted = documents.open(&open_params(&source, 1, &text)).unwrap();

        let analyzed = WorkspaceAnalyses::new(configuration).analyze(accepted);

        assert_eq!(
            analyzed.scope(),
            Some(&AnalysisScope::ToolchainStandard(standard_root))
        );
        assert!(analyzed.preparation_failure().is_none());
        assert_eq!(
            analyzed.snapshot().unwrap().status(),
            AnalysisStatus::Complete
        );
    }

    #[test]
    fn toolchain_standard_outside_workspace_shares_one_complete_overlay_snapshot() {
        let temporary = TemporaryDirectory::new();
        let standard_root = standard_root();
        let contract = standard_root.join("error/index.nct");
        let implementation = standard_root.join("error/construction.nct");
        let contract_text = fs::read_to_string(&contract).unwrap();
        let implementation_text = format!(
            "{}\n// Accepted editor overlay.\n",
            fs::read_to_string(&implementation).unwrap()
        );
        let configuration = configuration_with_standard(temporary.path(), &standard_root);
        let mut documents = DocumentWorkspace::new();
        let contract_generation = documents
            .open(&open_params(&contract, 1, &contract_text))
            .unwrap();
        let canonical_contract = contract_generation.path().to_path_buf();
        let mut analyses = WorkspaceAnalyses::new(configuration);
        let first = analyses.analyze(contract_generation);
        assert_eq!(first.snapshot().unwrap().status(), AnalysisStatus::Complete);

        let implementation_generation = documents
            .open(&open_params(&implementation, 3, &implementation_text))
            .unwrap();
        let canonical_implementation = implementation_generation.path().to_path_buf();
        let second = analyses.analyze(implementation_generation);

        assert_eq!(first.scope(), second.scope());
        assert_eq!(
            second.scope(),
            Some(&AnalysisScope::ToolchainStandard(standard_root))
        );
        assert_eq!(
            second.snapshot().unwrap().status(),
            AnalysisStatus::Complete
        );
        assert_eq!(
            second
                .source_overlay()
                .document(&canonical_implementation)
                .unwrap()
                .bytes(),
            implementation_text.as_bytes()
        );
        assert_eq!(
            analyses
                .latest_for_document(&canonical_contract)
                .unwrap()
                .generation(),
            second.generation()
        );
    }

    fn configuration(root: &Path) -> WorkspaceConfiguration {
        configuration_with_standard(root, &standard_root())
    }

    fn configuration_with_standard(root: &Path, standard_root: &Path) -> WorkspaceConfiguration {
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

    fn standard_root() -> PathBuf {
        fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std")).unwrap()
    }

    fn open_params(path: &Path, version: i32, text: &str) -> DidOpenParams {
        let escaped = text
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\r', "\\r")
            .replace('\n', "\\n")
            .replace('\t', "\\t");
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
