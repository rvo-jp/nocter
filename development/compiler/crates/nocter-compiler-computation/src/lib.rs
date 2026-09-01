//! Shared compiler-domain query entry for commands and persistent workspace analysis.

mod body_inputs;
mod module_surface;
mod semantic;
mod source_syntax;

use std::collections::VecDeque;
use std::sync::Arc;

use nocter_computation::{ComputationError, ComputationRevision, Database, Fingerprint};
use nocter_discovery::{DiscoveredUnit, DiscoveryFailure, DiscoveryRequest};
use nocter_filesystem::SourceOverlay;

pub use semantic::{
    FinalizedProgram, IncompleteSemanticAnalysis, IncompleteSemanticError,
    IncompleteSemanticEvidence, IncompleteSemanticFailure, ProgramAnalysisOutcome,
    ProgramAnalysisProduct, SemanticInputError, SemanticQueryFailure, UnitAnalysisOutcome,
    UnitAnalysisProduct,
};

const RETAINED_SOURCE_REVISIONS: usize = 32;

/// Sequential owner of compiler source and semantic query state.
#[derive(Debug)]
pub struct CompilerComputation {
    database: Database,
    revision_owner: Arc<()>,
    source_revision: u64,
    source_view: Option<Fingerprint>,
    source_checkpoints: VecDeque<ComputationRevision>,
}

impl Default for CompilerComputation {
    fn default() -> Self {
        Self {
            database: Database::new(),
            revision_owner: Arc::new(()),
            source_revision: 0,
            source_view: None,
            source_checkpoints: VecDeque::new(),
        }
    }
}

impl CompilerComputation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Atomically publishes one accepted source view.
    ///
    /// # Errors
    ///
    /// Returns computation revision exhaustion or an input publication failure.
    pub fn advance_sources(
        &mut self,
        overlay: &SourceOverlay,
        filesystem_epoch: u64,
    ) -> Result<CompilerSourceRevision, CompilerComputationError> {
        let publication = source_syntax::SourceRevisionPublication::new(overlay, filesystem_epoch);
        let source_view = publication.fingerprint();
        let changed = self.source_view != Some(source_view);
        let source_revision = if changed {
            self.source_revision
                .checked_add(1)
                .ok_or(ComputationError::RevisionExhausted)?
        } else {
            self.source_revision
        };
        let checkpoint = publication.publish(&mut self.database)?;
        if changed {
            self.retain_source_checkpoint(checkpoint);
        }
        self.source_revision = source_revision;
        self.source_view = Some(source_view);
        Ok(CompilerSourceRevision {
            owner: Arc::clone(&self.revision_owner),
            revision: source_revision,
        })
    }

    fn retain_source_checkpoint(&mut self, checkpoint: ComputationRevision) {
        self.source_checkpoints.push_back(checkpoint);
        if self.source_checkpoints.len() <= RETAINED_SOURCE_REVISIONS {
            return;
        }
        let _expired = self.source_checkpoints.pop_front();
        let oldest_retained = *self
            .source_checkpoints
            .front()
            .expect("a nonzero source-retention window has a first checkpoint");
        let _ = self.database.collect_inactive(oldest_retained);
    }

    /// Lends the sole syntax provider backed by this owner's source queries.
    ///
    /// # Errors
    ///
    /// Rejects a token from another computation owner or an earlier source revision.
    pub fn source_syntax(
        &self,
        revision: &CompilerSourceRevision,
    ) -> Result<impl nocter_syntax::SourceSyntaxProvider + '_, CompilerComputationError> {
        self.validate_revision(revision)?;
        Ok(source_syntax::ComputedSourceSyntax::new(&self.database))
    }

    /// Discovers one physical source unit through this revision's sole syntax provider.
    ///
    /// # Errors
    ///
    /// Rejects a foreign or stale revision before discovery, then returns the exact discovery
    /// failure selected from the admitted source view.
    pub fn discover(
        &self,
        revision: &CompilerSourceRevision,
        request: DiscoveryRequest,
    ) -> Result<CompilerDiscoveredUnit, CompilerDiscoveryError> {
        self.validate_revision(revision)
            .map_err(CompilerDiscoveryError::Computation)?;
        let mut source_syntax = source_syntax::ComputedSourceSyntax::new(&self.database);
        let unit = nocter_discovery::discover_with_source_syntax(request, &mut source_syntax)
            .map_err(CompilerDiscoveryError::Discovery)?;
        Ok(CompilerDiscoveredUnit {
            revision: revision.clone(),
            unit: Arc::new(unit),
        })
    }

    /// Publishes and demands the sole closed semantic product for one discovered source unit.
    ///
    /// # Errors
    ///
    /// Returns source-surface, discovery-domain, semantic-input, or computation failures.
    pub fn analyze(
        &mut self,
        discovered: &CompilerDiscoveredUnit,
    ) -> Result<Arc<UnitAnalysisProduct>, CompilerComputationError> {
        self.validate_revision(&discovered.revision)?;
        let unit = Arc::clone(&discovered.unit);
        let module_surface = module_surface::fingerprint(&self.database, &unit)?;
        let body_inputs = body_inputs::collect(&self.database, &unit)?;
        semantic::analyze_unit(&mut self.database, unit, module_surface, body_inputs)
            .map_err(CompilerComputationError::from)
    }

    fn validate_revision(
        &self,
        revision: &CompilerSourceRevision,
    ) -> Result<(), CompilerComputationError> {
        if !Arc::ptr_eq(&self.revision_owner, &revision.owner) {
            return Err(CompilerComputationError::SourceRevision(
                CompilerSourceRevisionError::ForeignOwner,
            ));
        }
        let current = self.source_revision;
        if current != revision.revision {
            return Err(CompilerComputationError::SourceRevision(
                CompilerSourceRevisionError::Stale {
                    current,
                    received: revision.revision,
                },
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn statistics(&self) -> CompilerComputationStatistics {
        let semantic = semantic::statistics(&self.database);
        CompilerComputationStatistics {
            retained_entries: self.database.retained_entry_count(),
            source_text_executions: source_syntax::source_text_execution_count(&self.database),
            parse_executions: source_syntax::execution_count(&self.database),
            parse_reuses: source_syntax::reuse_count(&self.database),
            declaration_surface_executions: source_syntax::declaration_surface_execution_count(
                &self.database,
            ),
            module_surface_executions: module_surface::execution_count(&self.database),
            module_surface_reuses: module_surface::reuse_count(&self.database),
            declaration_executions: semantic.declaration_executions,
            declaration_reuses: semantic.declaration_reuses,
            preparation_executions: semantic.preparation_executions,
            preparation_reuses: semantic.preparation_reuses,
            body_name_executions: semantic.body_name_executions,
            body_name_reuses: semantic.body_name_reuses,
            typed_body_executions: semantic.typed_body_executions,
            typed_body_reuses: semantic.typed_body_reuses,
            finalization_executions: semantic.finalization_executions,
            finalization_reuses: semantic.finalization_reuses,
            complete_analysis_executions: semantic.complete_analysis_executions,
            complete_analysis_reuses: semantic.complete_analysis_reuses,
            incomplete_analysis_executions: semantic.incomplete_analysis_executions,
            incomplete_analysis_reuses: semantic.incomplete_analysis_reuses,
            unit_analysis_executions: semantic.unit_analysis_executions,
            unit_analysis_reuses: semantic.unit_analysis_reuses,
        }
    }
}

/// Unforgeable admission token for the source inputs currently owned by one computation.
#[derive(Clone, Debug)]
pub struct CompilerSourceRevision {
    owner: Arc<()>,
    revision: u64,
}

/// One discovered unit inseparably admitted by an exact compiler source revision.
#[derive(Debug)]
pub struct CompilerDiscoveredUnit {
    revision: CompilerSourceRevision,
    unit: Arc<DiscoveredUnit>,
}

impl CompilerDiscoveredUnit {
    #[must_use]
    pub const fn unit(&self) -> &Arc<DiscoveredUnit> {
        &self.unit
    }
}

impl std::ops::Deref for CompilerDiscoveredUnit {
    type Target = DiscoveredUnit;

    fn deref(&self) -> &Self::Target {
        &self.unit
    }
}

#[derive(Debug)]
pub enum CompilerDiscoveryError {
    Computation(CompilerComputationError),
    Discovery(DiscoveryFailure),
}

impl std::fmt::Display for CompilerDiscoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Computation(error) => error.fmt(formatter),
            Self::Discovery(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CompilerDiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Computation(error) => Some(error),
            Self::Discovery(error) => Some(error),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerSourceRevisionError {
    ForeignOwner,
    Stale { current: u64, received: u64 },
}

impl std::fmt::Display for CompilerSourceRevisionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ForeignOwner => {
                formatter.write_str("source revision belongs to another compiler computation")
            }
            Self::Stale { current, received } => write!(
                formatter,
                "source revision {received} is stale; current revision is {current}",
            ),
        }
    }
}

impl std::error::Error for CompilerSourceRevisionError {}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompilerComputationStatistics {
    pub retained_entries: usize,
    pub source_text_executions: u64,
    pub parse_executions: u64,
    pub parse_reuses: u64,
    pub declaration_surface_executions: u64,
    pub module_surface_executions: u64,
    pub module_surface_reuses: u64,
    pub declaration_executions: u64,
    pub declaration_reuses: u64,
    pub preparation_executions: u64,
    pub preparation_reuses: u64,
    pub body_name_executions: u64,
    pub body_name_reuses: u64,
    pub typed_body_executions: u64,
    pub typed_body_reuses: u64,
    pub finalization_executions: u64,
    pub finalization_reuses: u64,
    pub complete_analysis_executions: u64,
    pub complete_analysis_reuses: u64,
    pub incomplete_analysis_executions: u64,
    pub incomplete_analysis_reuses: u64,
    pub unit_analysis_executions: u64,
    pub unit_analysis_reuses: u64,
}

#[derive(Debug)]
pub enum CompilerComputationError {
    Computation(ComputationError),
    SemanticInput(SemanticInputError),
    SourceRevision(CompilerSourceRevisionError),
}

impl std::fmt::Display for CompilerComputationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Computation(error) => error.fmt(formatter),
            Self::SemanticInput(error) => error.fmt(formatter),
            Self::SourceRevision(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CompilerComputationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Computation(error) => Some(error),
            Self::SemanticInput(error) => Some(error),
            Self::SourceRevision(error) => Some(error),
        }
    }
}

impl From<ComputationError> for CompilerComputationError {
    fn from(error: ComputationError) -> Self {
        Self::Computation(error)
    }
}

impl From<semantic::SemanticAnalysisError> for CompilerComputationError {
    fn from(error: semantic::SemanticAnalysisError) -> Self {
        match error {
            semantic::SemanticAnalysisError::Computation(error) => Self::Computation(error),
            semantic::SemanticAnalysisError::Input(error) => Self::SemanticInput(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use nocter_filesystem::{SourceOverlay, SourceOverride};

    use super::{
        CompilerComputation, CompilerComputationError, CompilerSourceRevisionError,
        RETAINED_SOURCE_REVISIONS,
    };

    #[test]
    fn source_revision_rejects_an_earlier_revision() {
        let mut computation = CompilerComputation::new();
        let stale = computation
            .advance_sources(&SourceOverlay::empty(), 0)
            .unwrap();
        let _current = computation
            .advance_sources(&SourceOverlay::empty(), 1)
            .unwrap();

        let Err(error) = computation.source_syntax(&stale) else {
            panic!("stale source revision was accepted");
        };

        assert!(matches!(
            error,
            CompilerComputationError::SourceRevision(CompilerSourceRevisionError::Stale { .. })
        ));
    }

    #[test]
    fn equivalent_source_publication_retains_the_admitted_revision() {
        let mut computation = CompilerComputation::new();
        let admitted = computation
            .advance_sources(&SourceOverlay::empty(), 0)
            .unwrap();
        let _equivalent = computation
            .advance_sources(&SourceOverlay::empty(), 0)
            .unwrap();

        assert!(computation.source_syntax(&admitted).is_ok());
    }

    #[test]
    fn source_revision_rejects_another_computation_owner() {
        let mut first = CompilerComputation::new();
        let revision = first.advance_sources(&SourceOverlay::empty(), 0).unwrap();
        let mut second = CompilerComputation::new();
        let _ = second.advance_sources(&SourceOverlay::empty(), 0).unwrap();

        let Err(error) = second.source_syntax(&revision) else {
            panic!("foreign source revision was accepted");
        };

        assert!(matches!(
            error,
            CompilerComputationError::SourceRevision(CompilerSourceRevisionError::ForeignOwner)
        ));
    }

    #[test]
    fn source_retention_bounds_obsolete_overlay_inputs() {
        let mut computation = CompilerComputation::new();
        for revision in 0..(RETAINED_SOURCE_REVISIONS + 8) {
            let mut overlay = SourceOverlay::builder();
            overlay
                .insert_source(
                    PathBuf::from(format!("/virtual/revision-{revision}.nct")),
                    SourceOverride::new(
                        format!("func value(): usize {{ return {revision} }}\n").into_bytes(),
                    ),
                )
                .unwrap();
            computation
                .advance_sources(&overlay.finish(), revision as u64)
                .unwrap();
        }

        assert_eq!(
            computation.statistics().retained_entries,
            RETAINED_SOURCE_REVISIONS + 2
        );
    }
}
