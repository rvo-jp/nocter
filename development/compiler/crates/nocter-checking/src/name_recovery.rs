use nocter_declarations::DeclarationGraph;
use nocter_frontend_bindings::SourceOwnershipTable;
use nocter_model::{Arena, BodyId, TypeStore};
use nocter_source_index::SourceIndex;

use crate::{BodyNameEvidence, ResolvedBodyNames};

/// Exact-current authored lexical failure materialized from one complete queried name set.
#[derive(Clone, Debug)]
pub struct QueriedNameResolutionFailure {
    diagnostic: nocter_diagnostics::SourceDiagnostic,
    recovery: NameAnalysisRecovery,
}

impl QueriedNameResolutionFailure {
    /// Narrows one preparation failure to the query-owned lexical-rejection contract.
    ///
    /// # Errors
    ///
    /// Returns the original failure when it is not an authored name rejection with recovery.
    pub fn from_preparation_failure(
        failure: crate::PreparationFailure,
    ) -> Result<Self, crate::PreparationFailure> {
        let (error, evidence) = failure.into_parts();
        let diagnostic = match error.source_diagnostic() {
            Some(diagnostic) => diagnostic.clone(),
            None => return Err(error.into()),
        };
        let Some(crate::PreparationFailureEvidence::Names(recovery)) = evidence else {
            return Err(error.into());
        };
        Ok(Self {
            diagnostic,
            recovery: *recovery,
        })
    }

    #[must_use]
    pub const fn diagnostic(&self) -> &nocter_diagnostics::SourceDiagnostic {
        &self.diagnostic
    }

    #[must_use]
    pub fn current_recovery(&self) -> NameAnalysisRecovery {
        self.recovery.clone()
    }
}

/// Explicit lexical-name evidence for every declared body.
#[derive(Clone, Debug)]
pub struct BodyNameEvidenceTable {
    bodies: Arena<BodyId, BodyNameEvidence>,
}

impl BodyNameEvidenceTable {
    pub(crate) const fn new(bodies: Arena<BodyId, BodyNameEvidence>) -> Self {
        Self { bodies }
    }

    #[must_use]
    pub fn get(&self, body: BodyId) -> Option<&ResolvedBodyNames> {
        self.bodies.get(body)?.usable_names()
    }

    #[must_use]
    pub fn evidence(&self, body: BodyId) -> Option<&BodyNameEvidence> {
        self.bodies.get(body)
    }

    pub fn iter(&self) -> impl Iterator<Item = (BodyId, &ResolvedBodyNames)> {
        self.bodies
            .iter()
            .filter_map(|(body, evidence)| evidence.usable_names().map(|names| (body, names)))
    }

    #[must_use]
    pub fn evidence_iter(&self) -> impl ExactSizeIterator<Item = (BodyId, &BodyNameEvidence)> {
        self.bodies.iter()
    }

    pub fn rejection_diagnostics(
        &self,
    ) -> impl Iterator<Item = &nocter_diagnostics::SourceDiagnostic> {
        self.bodies
            .iter()
            .filter_map(|(_, evidence)| Some(evidence.rejection()?.diagnostic()))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }
}

/// Current-generation declaration and body-local lexical state retained after name rules.
///
/// Failing spellings have no invented targets. Each body owns an independent sparse recovery slot,
/// so a failure cannot hide valid scopes from bodies visited later. The table remains unsuitable as
/// checking input because failed bodies are intentionally incomplete.
#[derive(Clone, Debug)]
pub struct NameAnalysisRecovery {
    graph: DeclarationGraph,
    types: TypeStore,
    body_names: BodyNameEvidenceTable,
    source_ownership: SourceOwnershipTable,
    source_index: SourceIndex,
}

impl NameAnalysisRecovery {
    pub(crate) const fn new(
        graph: DeclarationGraph,
        types: TypeStore,
        body_names: Arena<BodyId, BodyNameEvidence>,
        source_ownership: SourceOwnershipTable,
        source_index: SourceIndex,
    ) -> Self {
        Self {
            graph,
            types,
            body_names: BodyNameEvidenceTable::new(body_names),
            source_ownership,
            source_index,
        }
    }

    #[must_use]
    pub const fn graph(&self) -> &DeclarationGraph {
        &self.graph
    }

    #[must_use]
    pub const fn types(&self) -> &TypeStore {
        &self.types
    }

    #[must_use]
    pub const fn source_ownership(&self) -> &SourceOwnershipTable {
        &self.source_ownership
    }

    #[must_use]
    pub const fn body_names(&self) -> &BodyNameEvidenceTable {
        &self.body_names
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }
}
