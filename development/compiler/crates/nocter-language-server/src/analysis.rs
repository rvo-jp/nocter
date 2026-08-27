use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::ops::Deref;
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

use crate::{AcceptedDocumentRevision, WorkspaceConfiguration};

mod compilation_input;

use compilation_input::ScopeCompilationInput;

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

/// The complete outcome retained for one scope in an accepted workspace generation.
#[derive(Debug)]
pub struct WorkspaceAnalysisGeneration {
    scope: Option<AnalysisScope>,
    invalidated: Box<[AnalysisScope]>,
    generation: GenerationId,
    state: WorkspaceAnalysisState,
}

impl WorkspaceAnalysisGeneration {
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
            WorkspaceAnalysisState::PreparationFailed { .. }
            | WorkspaceAnalysisState::InvalidationOnly { .. } => None,
        }
    }

    #[must_use]
    pub const fn preparation_failure(&self) -> Option<&WorkspaceAnalysisError> {
        match &self.state {
            WorkspaceAnalysisState::PreparationFailed { error, .. } => Some(error),
            WorkspaceAnalysisState::Complete(_)
            | WorkspaceAnalysisState::InvalidationOnly { .. } => None,
        }
    }

    #[must_use]
    pub const fn source_overlay(&self) -> &SourceOverlay {
        match &self.state {
            WorkspaceAnalysisState::Complete(snapshot) => snapshot.source_overlay(),
            WorkspaceAnalysisState::PreparationFailed { source_overlay, .. }
            | WorkspaceAnalysisState::InvalidationOnly { source_overlay } => source_overlay,
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
            WorkspaceAnalysisState::PreparationFailed { .. }
            | WorkspaceAnalysisState::InvalidationOnly { .. } => None,
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
            WorkspaceAnalysisState::PreparationFailed { .. }
            | WorkspaceAnalysisState::InvalidationOnly { .. } => &[],
        }
    }
}

/// One atomic workspace-analysis transition and every scope generation it refreshed.
#[derive(Debug)]
pub struct WorkspaceAnalysisBatch {
    primary: Arc<WorkspaceAnalysisGeneration>,
    related: Box<[Arc<WorkspaceAnalysisGeneration>]>,
}

impl WorkspaceAnalysisBatch {
    #[must_use]
    pub const fn primary(&self) -> &Arc<WorkspaceAnalysisGeneration> {
        &self.primary
    }

    pub fn generations(&self) -> impl Iterator<Item = &Arc<WorkspaceAnalysisGeneration>> {
        std::iter::once(&self.primary).chain(self.related.iter())
    }

    pub fn publication_order(&self) -> impl Iterator<Item = &Arc<WorkspaceAnalysisGeneration>> {
        self.related.iter().chain(std::iter::once(&self.primary))
    }

    #[must_use]
    pub fn into_generations(self) -> Box<[Arc<WorkspaceAnalysisGeneration>]> {
        std::iter::once(self.primary)
            .chain(self.related)
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }
}

impl Deref for WorkspaceAnalysisBatch {
    type Target = WorkspaceAnalysisGeneration;

    fn deref(&self) -> &Self::Target {
        &self.primary
    }
}

#[derive(Debug)]
enum WorkspaceAnalysisState {
    Complete(Box<AnalysisSnapshot>),
    PreparationFailed {
        source_overlay: SourceOverlay,
        error: WorkspaceAnalysisError,
    },
    InvalidationOnly {
        source_overlay: SourceOverlay,
    },
}

/// Sequential owner of the latest immutable analysis for each package, toolchain standard, or
/// standalone file.
#[derive(Debug)]
pub struct WorkspaceAnalyses {
    configuration: WorkspaceConfiguration,
    latest: BTreeMap<AnalysisScope, Arc<WorkspaceAnalysisGeneration>>,
    document_scopes: BTreeMap<PathBuf, AnalysisScope>,
    source_scopes: BTreeMap<PathBuf, BTreeSet<AnalysisScope>>,
    unscoped: BTreeMap<PathBuf, Arc<WorkspaceAnalysisGeneration>>,
}

/// More than one current package context can answer a source request and none is authoritative.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmbiguousDocumentAnalysis {
    document: PathBuf,
    candidates: Box<[AnalysisScope]>,
}

impl AmbiguousDocumentAnalysis {
    #[must_use]
    pub fn document(&self) -> &Path {
        &self.document
    }

    #[must_use]
    pub const fn candidates(&self) -> &[AnalysisScope] {
        &self.candidates
    }
}

impl fmt::Display for AmbiguousDocumentAnalysis {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} belongs to multiple current analysis contexts",
            self.document.display()
        )
    }
}

impl std::error::Error for AmbiguousDocumentAnalysis {}

struct ScopeTransition {
    documents: BTreeSet<PathBuf>,
    selected: BTreeMap<PathBuf, AnalysisScope>,
    active_selected: BTreeMap<PathBuf, AnalysisScope>,
    failures: BTreeMap<PathBuf, WorkspaceAnalysisError>,
    affected: BTreeSet<AnalysisScope>,
    invalidated: Vec<AnalysisScope>,
    primary_scope: Option<AnalysisScope>,
}

impl WorkspaceAnalyses {
    #[must_use]
    pub fn new(configuration: WorkspaceConfiguration) -> Self {
        Self {
            configuration,
            latest: BTreeMap::new(),
            document_scopes: BTreeMap::new(),
            source_scopes: BTreeMap::new(),
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

    ///
    /// # Errors
    ///
    /// Returns [`AmbiguousDocumentAnalysis`] when multiple current contexts reach `document` and
    /// no exact selected scope or unique physical owner can supply authority.
    pub fn latest_for_document(
        &self,
        document: &Path,
    ) -> Result<Option<&WorkspaceAnalysisGeneration>, AmbiguousDocumentAnalysis> {
        if let Some(generation) = self
            .document_scopes
            .get(document)
            .and_then(|scope| self.latest.get(scope))
            .or_else(|| self.unscoped.get(document))
        {
            return Ok(Some(generation));
        }
        let Some(scopes) = self.source_scopes.get(document) else {
            return Ok(None);
        };
        let owned = scopes
            .iter()
            .filter(|scope| scope_owns_document(scope, document))
            .collect::<Vec<_>>();
        let candidates = if owned.is_empty() {
            scopes.iter().collect::<Vec<_>>()
        } else {
            owned
        };
        if candidates.len() == 1 {
            return Ok(self.latest.get(candidates[0]).map(Arc::as_ref));
        }
        Err(AmbiguousDocumentAnalysis {
            document: document.to_path_buf(),
            candidates: candidates
                .into_iter()
                .cloned()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        })
    }

    /// Selects one bounded package or single-file scope and runs its exact accepted overlay through
    /// locked, offline, read-only compiler preparation and target checking.
    ///
    /// # Panics
    ///
    /// Panics only when the internally planned primary transition fails to publish its generation.
    /// That condition indicates a broken workspace-transition invariant, not invalid user source.
    pub fn analyze(&mut self, accepted: AcceptedDocumentRevision) -> WorkspaceAnalysisBatch {
        let (document, source) = accepted.into_parts();
        let generation = source.generation();
        let open_documents = source
            .open_documents()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let changed_documents = source
            .changes()
            .iter()
            .map(|change| change.path().to_path_buf())
            .collect::<BTreeSet<_>>();
        let source_overlay = source.into_source_overlay();
        let mut transition = self.plan_transition(
            &document,
            &open_documents,
            &changed_documents,
            &source_overlay,
        );
        for scope in &transition.invalidated {
            self.latest.remove(scope);
        }
        let mut scoped_results = self.refresh_scoped(generation, &source_overlay, &transition);
        let mut related =
            self.refresh_unscoped(&document, generation, &source_overlay, &mut transition);
        self.document_scopes = transition.active_selected.clone();
        let primary = match transition.primary_scope {
            Some(scope) => scoped_results
                .remove(&scope)
                .expect("primary scope is always affected"),
            None => self
                .unscoped
                .get(&document)
                .cloned()
                .expect("primary unscoped generation"),
        };
        related.extend(scoped_results.into_values());
        let active_scopes = transition
            .active_selected
            .values()
            .cloned()
            .collect::<BTreeSet<_>>();
        self.latest.retain(|scope, _| active_scopes.contains(scope));
        self.unscoped
            .retain(|path, _| open_documents.contains(path));
        self.rebuild_source_scopes();
        WorkspaceAnalysisBatch {
            primary,
            related: related.into_boxed_slice(),
        }
    }

    fn plan_transition(
        &self,
        document: &Path,
        open_documents: &BTreeSet<PathBuf>,
        changed_documents: &BTreeSet<PathBuf>,
        source_overlay: &SourceOverlay,
    ) -> ScopeTransition {
        let documents = open_documents
            .union(changed_documents)
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut selected = BTreeMap::new();
        let mut failures = BTreeMap::new();
        for candidate in &documents {
            match select_scope(&self.configuration, source_overlay, candidate) {
                Ok(scope) => {
                    selected.insert(candidate.clone(), scope);
                }
                Err(error) => {
                    failures.insert(candidate.clone(), error);
                }
            }
        }
        let active_selected = selected
            .iter()
            .filter(|(path, _)| open_documents.contains(*path))
            .map(|(path, scope)| (path.clone(), scope.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut affected = self.changed_scopes(open_documents, &active_selected);
        if let Some(scope) = selected.get(document) {
            affected.insert(scope.clone());
        }
        for changed in changed_documents {
            affected.extend(selected.get(changed).cloned());
            affected.extend(
                self.latest
                    .iter()
                    .filter(|(_, latest)| generation_reaches_document(latest, changed))
                    .map(|(scope, _)| scope.clone()),
            );
        }
        let active_scopes = active_selected.values().collect::<BTreeSet<_>>();
        let invalidated = affected
            .iter()
            .filter(|scope| !active_scopes.contains(scope))
            .cloned()
            .collect();
        let primary_scope = selected.get(document).cloned();
        ScopeTransition {
            documents,
            selected,
            active_selected,
            failures,
            affected,
            invalidated,
            primary_scope,
        }
    }

    fn changed_scopes(
        &self,
        documents: &BTreeSet<PathBuf>,
        selected: &BTreeMap<PathBuf, AnalysisScope>,
    ) -> BTreeSet<AnalysisScope> {
        let mut affected = BTreeSet::new();
        for candidate in documents {
            let previous = self.document_scopes.get(candidate);
            let next = selected.get(candidate);
            if previous != next {
                affected.extend(previous.cloned());
                affected.extend(next.cloned());
            }
        }
        affected
    }

    fn refresh_scoped(
        &mut self,
        generation: GenerationId,
        source_overlay: &SourceOverlay,
        transition: &ScopeTransition,
    ) -> BTreeMap<AnalysisScope, Arc<WorkspaceAnalysisGeneration>> {
        transition
            .affected
            .iter()
            .map(|scope| {
                let scope_members = transition
                    .active_selected
                    .iter()
                    .filter(|(_, selected)| *selected == scope)
                    .map(|(source, _)| source.clone());
                let primary = transition.primary_scope.as_ref() == Some(scope);
                let input = ScopeCompilationInput::new(scope, scope_members);
                let active = transition
                    .active_selected
                    .values()
                    .any(|selected| selected == scope);
                let result = Arc::new(WorkspaceAnalysisGeneration {
                    scope: active.then(|| scope.clone()),
                    invalidated: if primary {
                        transition.invalidated.clone().into_boxed_slice()
                    } else {
                        Box::new([])
                    },
                    generation,
                    state: if active {
                        compile_scope(
                            &self.configuration,
                            &input,
                            generation,
                            source_overlay.clone(),
                        )
                    } else {
                        WorkspaceAnalysisState::InvalidationOnly {
                            source_overlay: source_overlay.clone(),
                        }
                    },
                });
                if active {
                    self.latest.insert(scope.clone(), Arc::clone(&result));
                }
                (scope.clone(), result)
            })
            .collect()
    }

    fn refresh_unscoped(
        &mut self,
        document: &Path,
        generation: GenerationId,
        source_overlay: &SourceOverlay,
        transition: &mut ScopeTransition,
    ) -> Vec<Arc<WorkspaceAnalysisGeneration>> {
        let mut related = Vec::new();
        for candidate in &transition.documents {
            if transition.selected.contains_key(candidate) {
                self.unscoped.remove(candidate);
                continue;
            }
            let changed = self.document_scopes.contains_key(candidate) || candidate == document;
            if !changed {
                continue;
            }
            let error = transition
                .failures
                .remove(candidate)
                .expect("every unscoped document retains its selection failure");
            let result = Arc::new(WorkspaceAnalysisGeneration {
                scope: None,
                invalidated: if candidate == document {
                    transition.invalidated.clone().into_boxed_slice()
                } else {
                    Box::new([])
                },
                generation,
                state: WorkspaceAnalysisState::PreparationFailed {
                    source_overlay: source_overlay.clone(),
                    error,
                },
            });
            self.unscoped.insert(candidate.clone(), Arc::clone(&result));
            if candidate != document {
                related.push(result);
            }
        }
        related
    }

    /// Compiles a speculative overlay without publishing or replacing an accepted generation.
    ///
    /// Mutation features use this as a transaction preflight. The candidate travels through the
    /// same package resolution, discovery, and compiler pipeline as accepted editor state.
    pub(crate) fn compile_candidate(
        &self,
        scope: &AnalysisScope,
        document: &Path,
        generation: GenerationId,
        source_overlay: SourceOverlay,
    ) -> Option<Box<AnalysisSnapshot>> {
        let requested_sources = self
            .document_scopes
            .iter()
            .filter(|(_, selected)| *selected == scope)
            .map(|(source, _)| source.clone())
            .chain(std::iter::once(document.to_path_buf()));
        let input = ScopeCompilationInput::new(scope, requested_sources);
        match compile_scope(&self.configuration, &input, generation, source_overlay) {
            WorkspaceAnalysisState::Complete(snapshot) => Some(snapshot),
            WorkspaceAnalysisState::PreparationFailed { .. }
            | WorkspaceAnalysisState::InvalidationOnly { .. } => None,
        }
    }

    fn rebuild_source_scopes(&mut self) {
        self.source_scopes.clear();
        for (scope, generation) in &self.latest {
            let Some(sources) = generation.reached_sources() else {
                continue;
            };
            for source in sources.iter() {
                self.source_scopes
                    .entry(PathBuf::from(source.name().as_str()))
                    .or_default()
                    .insert(scope.clone());
            }
        }
    }
}

fn scope_owns_document(scope: &AnalysisScope, document: &Path) -> bool {
    match scope {
        AnalysisScope::Package(root) | AnalysisScope::ToolchainStandard(root) => {
            document.starts_with(root)
        }
        AnalysisScope::SingleFile(source) => document == source,
    }
}

fn generation_reaches_document(generation: &WorkspaceAnalysisGeneration, document: &Path) -> bool {
    let Some(name) = document.to_str() else {
        return false;
    };
    generation
        .reached_sources()
        .is_some_and(|sources| sources.find_by_name(name).is_some())
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
        if nocter_package::has_package_declaration(source_overlay, directory)
            .map_err(WorkspaceAnalysisError::PackageRootProbe)?
        {
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
    input: &ScopeCompilationInput,
    generation: GenerationId,
    source_overlay: SourceOverlay,
) -> WorkspaceAnalysisState {
    let discovered = match input {
        ScopeCompilationInput::Package {
            root,
            requested_sources,
        } => discover_package(
            configuration,
            root,
            requested_sources,
            source_overlay.clone(),
        ),
        ScopeCompilationInput::ToolchainStandard => {
            discover_toolchain_standard(configuration, source_overlay.clone())
        }
        ScopeCompilationInput::SingleFile(source) => {
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
    requested_sources: &[PathBuf],
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
    for source in requested_sources {
        roots.insert(
            nocter_discovery::module_for_source(
                &root_package,
                root,
                source,
                selected.graph().source_overlay(),
            )
            .map_err(|error| {
                AnalysisPreparationFailure::Preparation(WorkspaceAnalysisError::ModuleOwner(error))
            })?,
        );
    }
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
    PackageRootProbe(nocter_package::PackageRootProbeError),
    ModuleOwner(nocter_discovery::DiscoveryError),
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
            Self::OutsideWorkspace(_)
            | Self::UnsupportedSource(_)
            | Self::Filesystem { .. }
            | Self::ModuleOwner(_) => "E0702",
            Self::Package(_) | Self::PackageRootProbe(_) => "E0800",
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
            Self::PackageRootProbe(error) => error.fmt(formatter),
            Self::ModuleOwner(error) => {
                write!(formatter, "cannot determine source module: {error}")
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
            Self::PackageRootProbe(error) => Some(error),
            Self::ModuleOwner(error) => Some(error),
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
    use nocter_lsp::{
        DidChangeParams, DidCloseParams, DidOpenParams, DocumentUri, InitializeParams,
    };
    use nocter_model::{CompilationTarget, PackageIdentity};
    use nocter_package::StandardPackage;

    use super::*;
    use crate::{
        DocumentWorkspace, DocumentWorkspaceChange, LanguageServerEnvironment,
        LanguageServerToolchain,
    };

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn package_generation_uses_overlay_bytes_and_reaches_compiler_analysis() {
        let temporary = TemporaryDirectory::new();
        fs::write(
            temporary.path().join("index.nct"),
            concat!(
                "#package: { name: \"app\", version: \"0.0.0\", }\n",
                "pub func answer(): i32 { return 42 }\n",
            ),
        )
        .unwrap();
        let source = temporary.path().join("index.nct");
        let configuration = configuration(temporary.path());
        let mut documents = DocumentWorkspace::new();
        let accepted = documents
            .open(&open_params(
                &source,
                7,
                concat!(
                    "#package: { name: \"app\", version: \"0.0.0\", }\n",
                    "pub func answer(): i32 { return }\n",
                ),
            ))
            .unwrap();
        let canonical_source = accepted.path().to_path_buf();
        let mut analyses = WorkspaceAnalyses::new(configuration);

        let analyzed = analyses.analyze(accepted);

        assert!(matches!(analyzed.scope(), Some(AnalysisScope::Package(_))));
        let snapshot = analyzed.snapshot().expect("discovery reaches analysis");
        assert_eq!(snapshot.status(), AnalysisStatus::CompilationFailed);
        assert_eq!(
            snapshot
                .source_overlay()
                .document(&canonical_source)
                .unwrap()
                .bytes(),
            concat!(
                "#package: { name: \"app\", version: \"0.0.0\", }\n",
                "pub func answer(): i32 { return }\n",
            )
            .as_bytes()
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
    fn package_generation_retains_every_open_module_root_without_a_representative_source() {
        let temporary = TemporaryDirectory::new();
        fs::write(
            temporary.path().join("index.nct"),
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
        )
        .unwrap();
        let first_directory = temporary.path().join("first");
        let second_directory = temporary.path().join("second");
        fs::create_dir(&first_directory).unwrap();
        fs::create_dir(&second_directory).unwrap();
        let first = first_directory.join("index.nct");
        let second = second_directory.join("index.nct");
        let first_text = "pub func first(): i32 { return 1 }\n";
        let second_text = "pub func second(): i32 { return 2 }\n";
        fs::write(&first, first_text).unwrap();
        fs::write(&second, second_text).unwrap();

        let configuration = configuration(temporary.path());
        let mut documents = DocumentWorkspace::new();
        let mut analyses = WorkspaceAnalyses::new(configuration.clone());
        let first_revision = documents.open(&open_params(&first, 1, first_text)).unwrap();
        let canonical_first = first_revision.path().to_path_buf();
        analyses.analyze(first_revision);
        let second_revision = documents
            .open(&open_params(&second, 1, second_text))
            .unwrap();
        let canonical_second = second_revision.path().to_path_buf();
        let generation = analyses.analyze(second_revision);
        let snapshot = generation.snapshot().expect("package analysis snapshot");

        assert!(
            snapshot
                .sources()
                .find_by_name(canonical_first.to_str().unwrap())
                .is_some()
        );
        assert!(
            snapshot
                .sources()
                .find_by_name(canonical_second.to_str().unwrap())
                .is_some()
        );
        assert_eq!(
            analyses
                .latest_for_document(&canonical_first)
                .unwrap()
                .unwrap()
                .generation(),
            generation.generation()
        );

        let mut reverse_documents = DocumentWorkspace::new();
        let mut reverse_analyses = WorkspaceAnalyses::new(configuration);
        reverse_analyses.analyze(
            reverse_documents
                .open(&open_params(&second, 1, second_text))
                .unwrap(),
        );
        let reverse_generation = reverse_analyses.analyze(
            reverse_documents
                .open(&open_params(&first, 1, first_text))
                .unwrap(),
        );
        let reverse_sources = reverse_generation.snapshot().unwrap().sources();
        assert!(
            reverse_sources
                .find_by_name(canonical_first.to_str().unwrap())
                .is_some()
        );
        assert!(
            reverse_sources
                .find_by_name(canonical_second.to_str().unwrap())
                .is_some()
        );
    }

    #[test]
    fn watched_source_change_does_not_add_a_closed_module_to_package_demand() {
        let temporary = TemporaryDirectory::new();
        let root = temporary.path().join("index.nct");
        let root_text = concat!(
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
            "func main(): i32 { return 0 }\n",
        );
        fs::write(&root, root_text).unwrap();
        let closed_directory = temporary.path().join("closed");
        fs::create_dir(&closed_directory).unwrap();
        let closed = closed_directory.join("index.nct");
        fs::write(&closed, "pub func closed(): i32 { return 1 }\n").unwrap();

        let mut documents = DocumentWorkspace::new();
        let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));
        let opened = documents.open(&open_params(&root, 1, root_text)).unwrap();
        let first = analyses.analyze(opened);
        let canonical_closed = fs::canonicalize(&closed).unwrap();
        assert!(
            first
                .snapshot()
                .unwrap()
                .sources()
                .find_by_name(canonical_closed.to_str().unwrap())
                .is_none()
        );

        let uri = DocumentUri::from_file_path(&closed).unwrap();
        let refreshed = documents.refresh(&[uri]).unwrap();
        let generation = analyses.analyze(refreshed);

        assert!(
            generation
                .snapshot()
                .unwrap()
                .sources()
                .find_by_name(canonical_closed.to_str().unwrap())
                .is_none(),
            "a change invalidates current demand but does not become semantic demand itself"
        );
    }

    #[test]
    fn source_without_a_bounded_package_root_uses_single_file_mode() {
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
            AnalysisStatus::Complete,
            "{:?}",
            analyzed.snapshot().unwrap().diagnostics()
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
            AnalysisStatus::Complete,
            "diagnostics={:?}, failure={:?}",
            analyzed.snapshot().unwrap().diagnostics(),
            analyzed.snapshot().unwrap().compilation_failure()
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
        assert_eq!(
            first.snapshot().unwrap().status(),
            AnalysisStatus::Complete,
            "diagnostics={:?}, failure={:?}",
            first.snapshot().unwrap().diagnostics(),
            first.snapshot().unwrap().compilation_failure()
        );

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
                .unwrap()
                .generation(),
            second.generation()
        );
    }

    #[test]
    fn a_shared_dependency_source_never_selects_a_package_context_by_sort_order() {
        let temporary = TemporaryDirectory::new();
        let standard_root = standard_root();
        let configuration = configuration_with_standard(temporary.path(), &standard_root);
        let mut documents = DocumentWorkspace::new();
        let mut analyses = WorkspaceAnalyses::new(configuration);

        for (directory, name) in [("first", "first"), ("second", "second")] {
            let root = temporary.path().join(directory);
            fs::create_dir(&root).unwrap();
            let source = root.join("index.nct");
            let text =
                format!("#package: {{ name: \"{name}\", version: \"0.0.0\", }}\nuse std/fs\n");
            fs::write(&source, &text).unwrap();
            analyses.analyze(documents.open(&open_params(&source, 1, &text)).unwrap());
        }

        let dependency_source = standard_root.join("fs/index.nct");
        let ambiguity = analyses
            .latest_for_document(&dependency_source)
            .expect_err("a dependency source shared by two packages has no implicit authority");

        assert_eq!(ambiguity.document(), dependency_source);
        assert_eq!(ambiguity.candidates().len(), 2);
        assert!(
            ambiguity
                .candidates()
                .iter()
                .all(|scope| matches!(scope, AnalysisScope::Package(_)))
        );
    }

    #[test]
    fn package_topology_change_reassigns_every_known_document_atomically() {
        let temporary = TemporaryDirectory::new();
        let index = temporary.path().join("index.nct");
        let helper = temporary.path().join("helper.nct");
        let package_text = concat!(
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
            "func main(): i32 { return 0 }\n",
        );
        let helper_text = "func helper(): i32 { return 1 }\n";
        fs::write(&index, package_text).unwrap();
        fs::write(&helper, helper_text).unwrap();
        let configuration = configuration(temporary.path());
        let mut documents = DocumentWorkspace::new();
        let mut analyses = WorkspaceAnalyses::new(configuration);

        let index_generation = documents
            .open(&open_params(&index, 1, package_text))
            .unwrap();
        let canonical_index = index_generation.path().to_path_buf();
        let first = analyses.analyze(index_generation);
        let package_scope = first.scope().unwrap().clone();

        let helper_generation = documents
            .open(&open_params(&helper, 1, helper_text))
            .unwrap();
        let canonical_helper = helper_generation.path().to_path_buf();
        let second = analyses.analyze(helper_generation);
        assert_eq!(second.scope(), Some(&package_scope));

        let DocumentWorkspaceChange::Accepted(changed) = documents
            .change(&change_params(&index, 2, "func main(): i32 { return 0 }\n"))
            .unwrap()
        else {
            panic!("current topology change was ignored")
        };
        let batch = analyses.analyze(changed);

        assert_eq!(
            batch.scope(),
            Some(&AnalysisScope::SingleFile(canonical_index.clone()))
        );
        assert_eq!(batch.invalidated_scopes(), &[package_scope]);
        assert!(batch.generations().any(|generation| {
            generation.scope() == Some(&AnalysisScope::SingleFile(canonical_helper.clone()))
        }));
        assert_eq!(
            analyses
                .latest_for_document(&canonical_index)
                .unwrap()
                .and_then(WorkspaceAnalysisGeneration::scope),
            Some(&AnalysisScope::SingleFile(canonical_index))
        );
        assert_eq!(
            analyses
                .latest_for_document(&canonical_helper)
                .unwrap()
                .and_then(WorkspaceAnalysisGeneration::scope),
            Some(&AnalysisScope::SingleFile(canonical_helper))
        );
    }

    #[test]
    fn closing_a_document_removes_it_from_the_current_workspace_domain() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("standalone.nct");
        fs::write(&source, "func main(): void { return }\n").unwrap();
        let configuration = configuration(temporary.path());
        let mut documents = DocumentWorkspace::new();
        let mut analyses = WorkspaceAnalyses::new(configuration);
        let opened = documents
            .open(&open_params(&source, 1, "func main(): void { return }\n"))
            .unwrap();
        let canonical = opened.path().to_path_buf();
        let active = analyses.analyze(opened);
        let active_scope = active.scope().unwrap().clone();
        assert!(analyses.latest_for_document(&canonical).unwrap().is_some());

        let closed = documents.close(&close_params(&source)).unwrap();
        let invalidation = analyses.analyze(closed);

        assert!(invalidation.scope().is_none());
        assert!(invalidation.snapshot().is_none());
        assert_eq!(invalidation.invalidated_scopes(), &[active_scope]);
        assert!(analyses.latest_for_document(&canonical).unwrap().is_none());
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

    fn change_params(path: &Path, version: i32, text: &str) -> DidChangeParams {
        let escaped = text
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\r', "\\r")
            .replace('\n', "\\n")
            .replace('\t', "\\t");
        DidChangeParams::decode(Some(
            parse(&format!(
                concat!(
                    "{{\"textDocument\":{{\"uri\":\"file://{}\",\"version\":{}}},",
                    "\"contentChanges\":[{{\"text\":\"{}\"}}]}}"
                ),
                path.display(),
                version,
                escaped
            ))
            .unwrap(),
        ))
        .unwrap()
    }

    fn close_params(path: &Path) -> DidCloseParams {
        DidCloseParams::decode(Some(
            parse(&format!(
                "{{\"textDocument\":{{\"uri\":\"file://{}\"}}}}",
                path.display()
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
