//! Protocol-independent ownership of workspace topology and compiler analysis generations.
//!
//! One accepted source revision becomes one topology decision and the minimum affected set of
//! package, toolchain-standard, or single-file compiler generations. Editor protocols consume the
//! resulting immutable products without importing package resolution, discovery, or session
//! internals.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nocter_analysis::{
    EvidenceIntegrityError, SemanticMutationCandidate, ValidatedSemanticMutation,
};
use nocter_filesystem::SourceOverlay;
use nocter_workspace_revision::{
    GenerationId, WorkspaceRevisionSequence, WorkspaceSourceChangeKind, WorkspaceSourceRevision,
};

mod compilation;
mod compilation_input;
mod configuration;
mod errors;
mod generation;
mod module_surface;
mod source_syntax;
mod topology;

use compilation::compile_scope;
use compilation_input::ScopeCompilationInput;
pub use configuration::{WorkspaceConfiguration, WorkspaceConfigurationError, WorkspaceToolchain};
use errors::preparation_diagnostics;
pub use errors::{WorkspaceAnalysisError, WorkspaceDiagnosticError};
use generation::WorkspaceAnalysisState;
pub use generation::{AnalysisScope, WorkspaceAnalysisBatch, WorkspaceAnalysisGeneration};
use topology::{DocumentScopeSelection, WorkspaceTopology};

/// Sequential owner of the latest immutable analysis for each package, toolchain standard, or
/// standalone file.
#[derive(Debug)]
pub struct WorkspaceAnalyses {
    configuration: WorkspaceConfiguration,
    revision_sequence: Option<WorkspaceRevisionSequence>,
    latest_generation: Option<GenerationId>,
    latest: BTreeMap<AnalysisScope, Arc<WorkspaceAnalysisGeneration>>,
    document_scopes: BTreeMap<PathBuf, AnalysisScope>,
    source_scopes: BTreeMap<PathBuf, BTreeSet<AnalysisScope>>,
    unscoped: BTreeMap<PathBuf, Arc<WorkspaceAnalysisGeneration>>,
    filesystem_epoch: u64,
    computation: nocter_computation::Database,
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

/// A source revision that cannot advance this workspace analysis owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceRevisionError {
    ForeignSequence,
    ComputationRevisionExhausted,
    NonIncreasing {
        current: GenerationId,
        received: GenerationId,
    },
}

impl fmt::Display for WorkspaceRevisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignSequence => {
                formatter.write_str("source revision belongs to another workspace sequence")
            }
            Self::ComputationRevisionExhausted => {
                formatter.write_str("workspace computation revision identity space is exhausted")
            }
            Self::NonIncreasing { current, received } => write!(
                formatter,
                "source revision generation {} does not advance current generation {}",
                received.get(),
                current.get()
            ),
        }
    }
}

impl std::error::Error for WorkspaceRevisionError {}

struct ScopeTransition {
    selections: BTreeMap<PathBuf, DocumentScopeSelection>,
    package_roots: nocter_package::PackageRootCatalog,
    active_selected: BTreeMap<PathBuf, AnalysisScope>,
    affected: BTreeSet<AnalysisScope>,
    invalidated: Vec<AnalysisScope>,
    primary_scope: Option<AnalysisScope>,
}

impl WorkspaceAnalyses {
    #[must_use]
    pub fn new(configuration: WorkspaceConfiguration) -> Self {
        Self {
            configuration,
            revision_sequence: None,
            latest_generation: None,
            latest: BTreeMap::new(),
            document_scopes: BTreeMap::new(),
            source_scopes: BTreeMap::new(),
            unscoped: BTreeMap::new(),
            filesystem_epoch: 0,
            computation: nocter_computation::Database::new(),
        }
    }

    #[cfg(test)]
    #[must_use]
    fn latest(&self, scope: &AnalysisScope) -> Option<&WorkspaceAnalysisGeneration> {
        self.latest.get(scope).map(Arc::as_ref)
    }

    #[cfg(test)]
    fn source_parse_counts(&self) -> (u64, u64) {
        (
            source_syntax::execution_count(&self.computation),
            source_syntax::reuse_count(&self.computation),
        )
    }

    #[cfg(test)]
    fn source_text_execution_count(&self) -> u64 {
        source_syntax::source_text_execution_count(&self.computation)
    }

    #[cfg(test)]
    fn declaration_surface_counts(&self) -> (u64, u64, u64) {
        (
            source_syntax::declaration_surface_execution_count(&self.computation),
            module_surface::execution_count(&self.computation),
            module_surface::reuse_count(&self.computation),
        )
    }

    #[cfg(test)]
    fn declaration_query_counts(&self) -> (u64, u64) {
        (
            nocter_semantic_computation::declaration_execution_count(&self.computation),
            nocter_semantic_computation::declaration_reuse_count(&self.computation),
        )
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
    /// # Errors
    ///
    /// Rejects a revision from another document owner or one that does not advance the accepted
    /// generation. No workspace analysis state changes on rejection.
    ///
    /// # Panics
    ///
    /// Panics only when an internally planned primary transition fails to publish its generation.
    /// That condition indicates a broken workspace-transition invariant, not invalid user source.
    pub fn analyze(
        &mut self,
        source: WorkspaceSourceRevision,
    ) -> Result<WorkspaceAnalysisBatch, WorkspaceRevisionError> {
        self.validate_revision(&source)?;
        let revision_sequence = source.sequence().clone();
        let document = source.primary_document().to_path_buf();
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
        let filesystem_epoch = if source
            .changes()
            .iter()
            .any(|change| change.kind() == WorkspaceSourceChangeKind::Filesystem)
        {
            self.filesystem_epoch
                .checked_add(1)
                .ok_or(WorkspaceRevisionError::ComputationRevisionExhausted)?
        } else {
            self.filesystem_epoch
        };
        let source_overlay = source.into_source_overlay();
        source_syntax::advance_revision(&mut self.computation, &source_overlay, filesystem_epoch)
            .map_err(|_| WorkspaceRevisionError::ComputationRevisionExhausted)?;
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
        self.revision_sequence = Some(revision_sequence);
        self.latest_generation = Some(generation);
        self.filesystem_epoch = filesystem_epoch;
        Ok(WorkspaceAnalysisBatch::new(
            primary,
            related.into_boxed_slice(),
        ))
    }

    fn validate_revision(
        &self,
        source: &WorkspaceSourceRevision,
    ) -> Result<(), WorkspaceRevisionError> {
        if let Some(sequence) = &self.revision_sequence
            && sequence != source.sequence()
        {
            return Err(WorkspaceRevisionError::ForeignSequence);
        }
        if let Some(current) = self.latest_generation
            && source.generation() <= current
        {
            return Err(WorkspaceRevisionError::NonIncreasing {
                current,
                received: source.generation(),
            });
        }
        Ok(())
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
        let mut source_syntax = source_syntax::ComputedSourceSyntax::new(&self.computation);
        let (selections, package_roots) = WorkspaceTopology::build_with_source_syntax(
            &self.configuration,
            source_overlay,
            documents,
            &mut source_syntax,
        )
        .into_parts();
        let selected = selections
            .iter()
            .filter_map(|(path, selection)| match selection {
                DocumentScopeSelection::Selected(scope) => Some((path.clone(), scope.clone())),
                DocumentScopeSelection::Rejected(_) => None,
            })
            .collect::<BTreeMap<_, _>>();
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
            selections,
            package_roots,
            active_selected,
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
                let result = Arc::new(WorkspaceAnalysisGeneration::new(
                    active.then(|| scope.clone()),
                    if primary {
                        transition.invalidated.clone().into_boxed_slice()
                    } else {
                        Box::new([])
                    },
                    generation,
                    if active {
                        compile_scope(
                            &self.configuration,
                            &input,
                            generation,
                            transition.package_roots.clone(),
                            &mut self.computation,
                        )
                    } else {
                        WorkspaceAnalysisState::InvalidationOnly {
                            source_overlay: source_overlay.clone(),
                        }
                    },
                ));
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
        for (candidate, selection) in std::mem::take(&mut transition.selections) {
            let DocumentScopeSelection::Rejected(error) = selection else {
                self.unscoped.remove(&candidate);
                continue;
            };
            let changed = self.document_scopes.contains_key(&candidate) || candidate == document;
            if !changed {
                continue;
            }
            let result = Arc::new(WorkspaceAnalysisGeneration::new(
                None,
                if candidate == document {
                    transition.invalidated.clone().into_boxed_slice()
                } else {
                    Box::new([])
                },
                generation,
                WorkspaceAnalysisState::PreparationFailed {
                    source_overlay: source_overlay.clone(),
                    diagnostics: preparation_diagnostics(&error),
                    error,
                },
            ));
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
    ///
    /// # Errors
    ///
    /// Returns an evidence-integrity failure when the compiled candidate cannot establish the
    /// complete semantic relation required by the mutation transaction.
    pub fn validate_candidate<'source>(
        &self,
        analysis: &WorkspaceAnalysisGeneration,
        candidate: SemanticMutationCandidate<'source>,
    ) -> Result<Option<ValidatedSemanticMutation<'source>>, EvidenceIntegrityError> {
        let Some(source) = analysis.snapshot() else {
            return Ok(None);
        };
        let Some(scope) = analysis.scope() else {
            return Ok(None);
        };
        if !std::ptr::eq(source, candidate.source()) {
            return Ok(None);
        }
        let requested_sources = self
            .document_scopes
            .iter()
            .filter(|(_, selected)| *selected == scope)
            .map(|(source, _)| source.clone());
        let input = ScopeCompilationInput::new(scope, requested_sources);
        let mut candidate_computation = nocter_computation::Database::new();
        if source_syntax::advance_revision(
            &mut candidate_computation,
            candidate.source_overlay(),
            self.filesystem_epoch,
        )
        .is_err()
        {
            return Ok(None);
        }
        match compile_scope(
            &self.configuration,
            &input,
            source.generation(),
            nocter_package::PackageRootCatalog::new(candidate.source_overlay().clone()),
            &mut candidate_computation,
        ) {
            WorkspaceAnalysisState::Complete(snapshot) => candidate.validate(snapshot),
            WorkspaceAnalysisState::PreparationFailed { .. }
            | WorkspaceAnalysisState::InvalidationOnly { .. } => Ok(None),
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use nocter_analysis::AnalysisStatus;
    use nocter_filesystem::DocumentVersion;
    use nocter_model::{CompilationTarget, PackageIdentity};
    use nocter_package::StandardPackage;
    use nocter_workspace_revision::{DocumentChange, WorkspaceDocuments, WorkspaceSourceRevision};

    use super::*;
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    enum DocumentWorkspaceChange {
        Accepted(WorkspaceSourceRevision),
        IgnoredStale,
    }

    #[derive(Default)]
    struct DocumentWorkspace {
        documents: WorkspaceDocuments,
    }

    impl DocumentWorkspace {
        fn new() -> Self {
            Self::default()
        }

        fn open(
            &mut self,
            path: &Path,
            version: i32,
            text: &str,
        ) -> Result<WorkspaceSourceRevision, Box<dyn std::error::Error>> {
            let path = canonical_document_path(path)?;
            let revision = self.documents.open(
                path.clone(),
                DocumentVersion::new(version),
                Arc::<[u8]>::from(text.as_bytes()),
            )?;
            Ok(revision)
        }

        fn change(
            &mut self,
            path: &Path,
            version: i32,
            text: &str,
        ) -> Result<DocumentWorkspaceChange, Box<dyn std::error::Error>> {
            let path = canonical_document_path(path)?;
            Ok(
                match self.documents.change(
                    &path,
                    DocumentVersion::new(version),
                    Arc::<[u8]>::from(text.as_bytes()),
                )? {
                    DocumentChange::Accepted(revision) => {
                        DocumentWorkspaceChange::Accepted(revision)
                    }
                    DocumentChange::IgnoredStale { .. } => DocumentWorkspaceChange::IgnoredStale,
                },
            )
        }

        fn close(
            &mut self,
            path: &Path,
        ) -> Result<WorkspaceSourceRevision, Box<dyn std::error::Error>> {
            let path = canonical_document_path(path)?;
            let revision = self.documents.close(&path)?;
            Ok(revision)
        }

        fn refresh(
            &mut self,
            sources: &[&Path],
        ) -> Result<WorkspaceSourceRevision, Box<dyn std::error::Error>> {
            let paths = sources
                .iter()
                .map(|path| canonical_document_path(path))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(self.documents.refresh(paths)?)
        }
    }

    fn canonical_document_path(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
        if path.exists() {
            return Ok(fs::canonicalize(path)?);
        }
        let parent = fs::canonicalize(path.parent().ok_or("document has no parent")?)?;
        Ok(parent.join(path.file_name().ok_or("document has no file name")?))
    }

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
            .open(
                &source,
                7,
                concat!(
                    "#package: { name: \"app\", version: \"0.0.0\", }\n",
                    "pub func answer(): i32 { return }\n",
                ),
            )
            .unwrap();
        let canonical_source = accepted.primary_document().to_path_buf();
        let mut analyses = WorkspaceAnalyses::new(configuration);

        let analyzed = analyses.analyze(accepted).unwrap();

        assert!(matches!(
            analyzed.primary().scope(),
            Some(AnalysisScope::Package(_))
        ));
        let snapshot = analyzed
            .primary()
            .snapshot()
            .expect("discovery reaches analysis");
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
                .latest(analyzed.primary().scope().unwrap())
                .unwrap()
                .generation(),
            analyzed.primary().generation()
        );
    }

    #[test]
    fn unchanged_source_text_reuses_parsing_across_workspace_revisions() {
        let temporary = TemporaryDirectory::new();
        let root = temporary.path().join("index.nct");
        let helper = temporary.path().join("helper.nct");
        let root_text = concat!(
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
            "func answer(): i32 { return helper() }\n",
        );
        let changed_root_text = concat!(
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
            "func answer(): i32 { return helper() + 1 }\n",
        );
        let helper_text = "func helper(): i32 { return 41 }\n";
        let changed_helper_text = "func helper(): i32 { return 42 }\n";
        fs::write(&root, root_text).unwrap();
        fs::write(&helper, helper_text).unwrap();
        let mut documents = DocumentWorkspace::new();
        let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));

        analyses
            .analyze(documents.open(&root, 1, root_text).unwrap())
            .unwrap();
        let after_initial = analyses.source_parse_counts();
        let source_text_after_initial = analyses.source_text_execution_count();
        let surfaces_after_initial = analyses.declaration_surface_counts();
        let declarations_after_initial = analyses.declaration_query_counts();
        assert!(after_initial.0 > 0);

        let DocumentWorkspaceChange::Accepted(root_revision) =
            documents.change(&root, 2, changed_root_text).unwrap()
        else {
            panic!("newer root text is accepted");
        };
        analyses.analyze(root_revision).unwrap();
        let after_root_change = analyses.source_parse_counts();
        let surfaces_after_root_change = analyses.declaration_surface_counts();
        let declarations_after_root_change = analyses.declaration_query_counts();
        assert_eq!(after_root_change.0, after_initial.0 + 1);
        assert_eq!(
            analyses.source_text_execution_count(),
            source_text_after_initial + 1
        );
        assert!(after_root_change.1 > after_initial.1);
        assert_eq!(surfaces_after_root_change.0, surfaces_after_initial.0 + 1);
        assert_eq!(surfaces_after_root_change.1, surfaces_after_initial.1);
        assert!(surfaces_after_root_change.2 > surfaces_after_initial.2);
        assert_eq!(
            declarations_after_root_change.0,
            declarations_after_initial.0
        );
        assert!(declarations_after_root_change.1 > declarations_after_initial.1);

        analyses
            .analyze(documents.open(&helper, 1, helper_text).unwrap())
            .unwrap();
        let before_helper_change = analyses.source_parse_counts();
        let source_text_before_helper_change = analyses.source_text_execution_count();
        let surfaces_before_helper_change = analyses.declaration_surface_counts();
        let declarations_before_helper_change = analyses.declaration_query_counts();
        let DocumentWorkspaceChange::Accepted(helper_revision) =
            documents.change(&helper, 2, changed_helper_text).unwrap()
        else {
            panic!("newer helper text is accepted");
        };
        let warm = analyses.analyze(helper_revision).unwrap();
        let after_helper_change = analyses.source_parse_counts();
        let surfaces_after_helper_change = analyses.declaration_surface_counts();
        let declarations_after_helper_change = analyses.declaration_query_counts();
        assert_eq!(after_helper_change.0, before_helper_change.0 + 1);
        assert_eq!(
            analyses.source_text_execution_count(),
            source_text_before_helper_change + 1
        );
        assert_eq!(
            surfaces_after_helper_change.0,
            surfaces_before_helper_change.0 + 1
        );
        assert_eq!(
            surfaces_after_helper_change.1,
            surfaces_before_helper_change.1
        );
        assert_eq!(
            declarations_after_helper_change.0,
            declarations_before_helper_change.0
        );
        assert!(declarations_after_helper_change.1 > declarations_before_helper_change.1);

        let mut fresh_documents = DocumentWorkspace::new();
        let mut fresh_analyses = WorkspaceAnalyses::new(configuration(temporary.path()));
        fresh_analyses
            .analyze(fresh_documents.open(&root, 1, changed_root_text).unwrap())
            .unwrap();
        let fresh = fresh_analyses
            .analyze(
                fresh_documents
                    .open(&helper, 1, changed_helper_text)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            analysis_signature(warm.primary()),
            analysis_signature(fresh.primary())
        );
    }

    #[test]
    fn rejected_declarations_depend_on_exact_current_source() {
        let temporary = TemporaryDirectory::new();
        let root = temporary.path().join("index.nct");
        let original = concat!(
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
            "struct Duplicate {}\n",
            "struct Duplicate {}\n",
            "func body(): i32 { return 1 }\n",
        );
        let changed = concat!(
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
            "struct Duplicate {}\n",
            "struct Duplicate {}\n",
            "func body(): i32 { return 2 }\n",
        );
        fs::write(&root, original).unwrap();
        let mut documents = DocumentWorkspace::new();
        let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));

        let initial = analyses
            .analyze(documents.open(&root, 1, original).unwrap())
            .unwrap();
        assert_eq!(
            initial.primary().snapshot().unwrap().status(),
            AnalysisStatus::CompilationFailed
        );
        let before = analyses.declaration_query_counts();
        let DocumentWorkspaceChange::Accepted(revision) =
            documents.change(&root, 2, changed).unwrap()
        else {
            panic!("newer root text is accepted");
        };
        analyses.analyze(revision).unwrap();
        let after = analyses.declaration_query_counts();

        assert_eq!(after.0, before.0 + 1);
    }

    #[derive(Debug, Eq, PartialEq)]
    struct AnalysisSignature {
        status: AnalysisStatus,
        diagnostics: Vec<nocter_diagnostics::SourceDiagnostic>,
        sources: Vec<(Box<str>, Box<str>)>,
    }

    fn analysis_signature(generation: &WorkspaceAnalysisGeneration) -> AnalysisSignature {
        let snapshot = generation.snapshot().expect("analysis reached discovery");
        let sources = snapshot
            .sources()
            .iter()
            .map(|source| (source.name().as_str().into(), source.text().into()))
            .collect();
        AnalysisSignature {
            status: snapshot.status(),
            diagnostics: snapshot.diagnostics().to_vec(),
            sources,
        }
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
        let first_revision = documents.open(&first, 1, first_text).unwrap();
        let canonical_first = first_revision.primary_document().to_path_buf();
        analyses.analyze(first_revision).unwrap();
        let second_revision = documents.open(&second, 1, second_text).unwrap();
        let canonical_second = second_revision.primary_document().to_path_buf();
        let generation = analyses.analyze(second_revision).unwrap();
        let snapshot = generation
            .primary()
            .snapshot()
            .expect("package analysis snapshot");

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
            generation.primary().generation()
        );

        let mut reverse_documents = DocumentWorkspace::new();
        let mut reverse_analyses = WorkspaceAnalyses::new(configuration);
        reverse_analyses
            .analyze(reverse_documents.open(&second, 1, second_text).unwrap())
            .unwrap();
        let reverse_generation = reverse_analyses
            .analyze(reverse_documents.open(&first, 1, first_text).unwrap())
            .unwrap();
        let reverse_sources = reverse_generation.primary().snapshot().unwrap().sources();
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
        let opened = documents.open(&root, 1, root_text).unwrap();
        let first = analyses.analyze(opened).unwrap();
        let canonical_closed = fs::canonicalize(&closed).unwrap();
        assert!(
            first
                .primary()
                .snapshot()
                .unwrap()
                .sources()
                .find_by_name(canonical_closed.to_str().unwrap())
                .is_none()
        );

        let refreshed = documents.refresh(&[&closed]).unwrap();
        let generation = analyses.analyze(refreshed).unwrap();

        assert!(
            generation
                .primary()
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
            .open(&source, 1, "func main(): void { return }\n")
            .unwrap();
        let canonical_source = accepted.primary_document().to_path_buf();

        let analyzed = WorkspaceAnalyses::new(configuration)
            .analyze(accepted)
            .unwrap();

        assert_eq!(
            analyzed.primary().scope(),
            Some(&AnalysisScope::SingleFile(canonical_source))
        );
        assert_eq!(
            analyzed.primary().snapshot().unwrap().status(),
            AnalysisStatus::Complete,
            "{:?}",
            analyzed.primary().snapshot().unwrap().diagnostics()
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
        let accepted = documents.open(&source, 1, &text).unwrap();

        let analyzed = WorkspaceAnalyses::new(configuration)
            .analyze(accepted)
            .unwrap();

        assert_eq!(
            analyzed.primary().scope(),
            Some(&AnalysisScope::ToolchainStandard(standard_root))
        );
        assert!(analyzed.primary().preparation_failure().is_none());
        assert_eq!(
            analyzed.primary().snapshot().unwrap().status(),
            AnalysisStatus::Complete,
            "diagnostics={:?}",
            analyzed.primary().snapshot().unwrap().diagnostics()
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
        let contract_generation = documents.open(&contract, 1, &contract_text).unwrap();
        let canonical_contract = contract_generation.primary_document().to_path_buf();
        let mut analyses = WorkspaceAnalyses::new(configuration);
        let first = analyses.analyze(contract_generation).unwrap();
        assert_eq!(
            first.primary().snapshot().unwrap().status(),
            AnalysisStatus::Complete,
            "diagnostics={:?}",
            first.primary().snapshot().unwrap().diagnostics()
        );

        let implementation_generation = documents
            .open(&implementation, 3, &implementation_text)
            .unwrap();
        let canonical_implementation = implementation_generation.primary_document().to_path_buf();
        let second = analyses.analyze(implementation_generation).unwrap();

        assert_eq!(first.primary().scope(), second.primary().scope());
        assert_eq!(
            second.primary().scope(),
            Some(&AnalysisScope::ToolchainStandard(standard_root))
        );
        assert_eq!(
            second.primary().snapshot().unwrap().status(),
            AnalysisStatus::Complete
        );
        assert_eq!(
            second
                .primary()
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
            second.primary().generation()
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
            analyses
                .analyze(documents.open(&source, 1, &text).unwrap())
                .unwrap();
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

        let index_generation = documents.open(&index, 1, package_text).unwrap();
        let canonical_index = index_generation.primary_document().to_path_buf();
        let first = analyses.analyze(index_generation).unwrap();
        let package_scope = first.primary().scope().unwrap().clone();

        let helper_generation = documents.open(&helper, 1, helper_text).unwrap();
        let canonical_helper = helper_generation.primary_document().to_path_buf();
        let second = analyses.analyze(helper_generation).unwrap();
        assert_eq!(second.primary().scope(), Some(&package_scope));

        let DocumentWorkspaceChange::Accepted(changed) = documents
            .change(&index, 2, "func main(): i32 { return 0 }\n")
            .unwrap()
        else {
            panic!("current topology change was ignored")
        };
        let batch = analyses.analyze(changed).unwrap();

        assert_eq!(
            batch.primary().scope(),
            Some(&AnalysisScope::SingleFile(canonical_index.clone()))
        );
        assert_eq!(batch.primary().invalidated_scopes(), &[package_scope]);
        assert!(batch.publication_order().any(|generation| {
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
            .open(&source, 1, "func main(): void { return }\n")
            .unwrap();
        let canonical = opened.primary_document().to_path_buf();
        let active = analyses.analyze(opened).unwrap();
        let active_scope = active.primary().scope().unwrap().clone();
        assert!(analyses.latest_for_document(&canonical).unwrap().is_some());

        let closed = documents.close(&source).unwrap();
        let invalidation = analyses.analyze(closed).unwrap();

        assert!(invalidation.primary().scope().is_none());
        assert!(invalidation.primary().snapshot().is_none());
        assert_eq!(invalidation.primary().invalidated_scopes(), &[active_scope]);
        assert!(analyses.latest_for_document(&canonical).unwrap().is_none());
    }

    #[test]
    fn analysis_rejects_out_of_order_and_foreign_revision_sequences() {
        let temporary = TemporaryDirectory::new();
        let first = temporary.path().join("first.nct");
        let second = temporary.path().join("second.nct");
        let text = "func main(): void { return }\n";
        fs::write(&first, text).unwrap();
        fs::write(&second, text).unwrap();

        let mut documents = DocumentWorkspace::new();
        let older = documents.open(&first, 1, text).unwrap();
        let newer = documents.open(&second, 1, text).unwrap();
        let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));
        analyses.analyze(newer).unwrap();
        assert_eq!(
            analyses.analyze(older).unwrap_err(),
            WorkspaceRevisionError::NonIncreasing {
                current: GenerationId::new(2),
                received: GenerationId::new(1),
            }
        );
        assert_eq!(analyses.latest_generation, Some(GenerationId::new(2)));

        let mut foreign_documents = DocumentWorkspace::new();
        let foreign = foreign_documents.open(&first, 2, text).unwrap();
        assert_eq!(
            analyses.analyze(foreign).unwrap_err(),
            WorkspaceRevisionError::ForeignSequence
        );
        assert_eq!(analyses.latest_generation, Some(GenerationId::new(2)));
    }

    pub(super) fn configuration(root: &Path) -> WorkspaceConfiguration {
        configuration_with_standard(root, &standard_root())
    }

    fn configuration_with_standard(root: &Path, standard_root: &Path) -> WorkspaceConfiguration {
        WorkspaceConfiguration::resolve(
            [fs::canonicalize(root).unwrap()],
            WorkspaceToolchain::new(
                CompilationTarget::Arm64Darwin,
                root,
                StandardPackage::new(PackageIdentity::new("toolchain:std"), standard_root),
            ),
        )
        .unwrap()
    }

    fn standard_root() -> PathBuf {
        fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std")).unwrap()
    }

    pub(super) struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        pub(super) fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "nocter-language-server-analysis-{}-{id}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        pub(super) fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
