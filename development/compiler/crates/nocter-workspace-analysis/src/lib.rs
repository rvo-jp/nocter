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
    computation: nocter_compiler_computation::CompilerComputation,
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
            computation: nocter_compiler_computation::CompilerComputation::new(),
        }
    }

    #[cfg(test)]
    #[must_use]
    fn latest(&self, scope: &AnalysisScope) -> Option<&WorkspaceAnalysisGeneration> {
        self.latest.get(scope).map(Arc::as_ref)
    }

    #[cfg(test)]
    fn source_parse_counts(&self) -> (u64, u64) {
        let statistics = self.computation.statistics();
        (statistics.parse_executions, statistics.parse_reuses)
    }

    #[cfg(test)]
    fn declaration_surface_counts(&self) -> (u64, u64, u64) {
        let statistics = self.computation.statistics();
        (
            statistics.declaration_surface_executions,
            statistics.module_surface_executions,
            statistics.module_surface_reuses,
        )
    }

    #[cfg(test)]
    fn declaration_query_counts(&self) -> (u64, u64) {
        let statistics = self.computation.statistics();
        (
            statistics.declaration_executions,
            statistics.declaration_reuses,
        )
    }

    #[cfg(test)]
    fn program_preparation_counts(&self) -> (u64, u64) {
        let statistics = self.computation.statistics();
        (
            statistics.preparation_executions,
            statistics.preparation_reuses,
        )
    }

    #[cfg(test)]
    fn body_name_query_counts(&self) -> (u64, u64) {
        let statistics = self.computation.statistics();
        (statistics.body_name_executions, statistics.body_name_reuses)
    }

    #[cfg(test)]
    fn typed_body_query_counts(&self) -> (u64, u64) {
        let statistics = self.computation.statistics();
        (
            statistics.typed_body_executions,
            statistics.typed_body_reuses,
        )
    }

    #[cfg(test)]
    fn program_finalization_counts(&self) -> (u64, u64) {
        let statistics = self.computation.statistics();
        (
            statistics.finalization_executions,
            statistics.finalization_reuses,
        )
    }

    #[cfg(test)]
    fn program_materialization_counts(&self) -> (u64, u64) {
        let statistics = self.computation.statistics();
        (
            statistics.materialization_executions,
            statistics.materialization_reuses,
        )
    }

    #[cfg(test)]
    fn program_relation_counts(&self) -> (u64, u64) {
        let statistics = self.computation.statistics();
        (statistics.relation_executions, statistics.relation_reuses)
    }

    #[cfg(test)]
    fn incomplete_analysis_counts(&self) -> (u64, u64) {
        let statistics = self.computation.statistics();
        (
            statistics.incomplete_analysis_executions,
            statistics.incomplete_analysis_reuses,
        )
    }

    #[cfg(test)]
    fn program_analysis_counts(&self) -> (u64, u64) {
        let statistics = self.computation.statistics();
        (
            statistics.complete_analysis_executions,
            statistics.complete_analysis_reuses,
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
        let computation_revision = self
            .computation
            .advance_sources(&source_overlay, filesystem_epoch)
            .map_err(|_| WorkspaceRevisionError::ComputationRevisionExhausted)?;
        let mut transition = self.plan_transition(
            &document,
            &open_documents,
            &changed_documents,
            &source_overlay,
            &computation_revision,
        );
        for scope in &transition.invalidated {
            self.latest.remove(scope);
        }
        let mut scoped_results = self.refresh_scoped(
            generation,
            &source_overlay,
            &transition,
            &computation_revision,
        );
        let mut updated =
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
        updated.extend(scoped_results.into_values());
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
        let current = self
            .latest
            .values()
            .chain(self.unscoped.values())
            .cloned()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(WorkspaceAnalysisBatch::new(
            primary,
            updated.into_boxed_slice(),
            current,
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
        revision: &nocter_compiler_computation::CompilerSourceRevision,
    ) -> ScopeTransition {
        let documents = open_documents
            .union(changed_documents)
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut source_syntax = self
            .computation
            .source_syntax(revision)
            .expect("workspace retains the current compiler source revision");
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
        revision: &nocter_compiler_computation::CompilerSourceRevision,
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
                let input = ScopeCompilationInput::new(scope, scope_members);
                let active = transition
                    .active_selected
                    .values()
                    .any(|selected| selected == scope);
                let result = Arc::new(WorkspaceAnalysisGeneration::new(
                    active.then(|| scope.clone()),
                    generation,
                    if active {
                        compile_scope(
                            &self.configuration,
                            &input,
                            generation,
                            transition.package_roots.clone(),
                            &mut self.computation,
                            revision,
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
        let mut updated = Vec::new();
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
                generation,
                WorkspaceAnalysisState::PreparationFailed {
                    source_overlay: source_overlay.clone(),
                    diagnostics: preparation_diagnostics(&error),
                    error,
                },
            ));
            self.unscoped.insert(candidate.clone(), Arc::clone(&result));
            if candidate != document {
                updated.push(result);
            }
        }
        updated
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
        let mut candidate_computation = nocter_compiler_computation::CompilerComputation::new();
        let Ok(candidate_revision) = candidate_computation
            .advance_sources(candidate.source_overlay(), self.filesystem_epoch)
        else {
            return Ok(None);
        };
        match compile_scope(
            &self.configuration,
            &input,
            source.generation(),
            nocter_package::PackageRootCatalog::new(candidate.source_overlay().clone()),
            &mut candidate_computation,
            &candidate_revision,
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
mod tests;
