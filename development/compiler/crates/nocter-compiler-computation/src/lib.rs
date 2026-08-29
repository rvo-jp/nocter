//! Shared compiler-domain query entry for commands and persistent workspace analysis.

mod body_inputs;
mod module_surface;
mod source_syntax;

use std::sync::Arc;

use nocter_computation::{ComputationError, ComputationRevision, Database};
use nocter_discovery::DiscoveredUnit;
use nocter_filesystem::SourceOverlay;

/// Sequential owner of compiler source and semantic query state.
#[derive(Debug, Default)]
pub struct CompilerComputation {
    database: Database,
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
    ) -> Result<ComputationRevision, ComputationError> {
        source_syntax::advance_revision(&mut self.database, overlay, filesystem_epoch)
    }

    /// Lends the sole syntax provider backed by this owner's source queries.
    pub fn source_syntax(&self) -> impl nocter_syntax::SourceSyntaxProvider + '_ {
        source_syntax::ComputedSourceSyntax::new(&self.database)
    }

    /// Publishes and demands the sole closed semantic product for one discovered source unit.
    ///
    /// # Errors
    ///
    /// Returns source-surface, discovery-domain, semantic-input, or computation failures.
    pub fn analyze(
        &mut self,
        unit: Arc<DiscoveredUnit>,
    ) -> Result<Arc<nocter_semantic_computation::UnitAnalysisProduct>, CompilerComputationError>
    {
        let module_surface = module_surface::fingerprint(&self.database, &unit)?;
        let body_inputs = body_inputs::collect(&self.database, &unit)?;
        let (scope, publication) =
            nocter_semantic_computation::ScopeInputPublication::for_unit(unit, module_surface)?;
        let mut revision = self.database.advance_revision()?;
        publication.publish(&mut revision, &scope);
        for body in body_inputs {
            body.publish(&mut revision);
        }
        let _ = revision.commit();
        nocter_semantic_computation::analyzed_unit(&self.database, scope)
            .map_err(CompilerComputationError::from)
    }

    #[must_use]
    pub fn statistics(&self) -> CompilerComputationStatistics {
        CompilerComputationStatistics {
            source_text_executions: source_syntax::source_text_execution_count(&self.database),
            parse_executions: source_syntax::execution_count(&self.database),
            parse_reuses: source_syntax::reuse_count(&self.database),
            declaration_surface_executions: source_syntax::declaration_surface_execution_count(
                &self.database,
            ),
            module_surface_executions: module_surface::execution_count(&self.database),
            module_surface_reuses: module_surface::reuse_count(&self.database),
            declaration_executions: nocter_semantic_computation::declaration_execution_count(
                &self.database,
            ),
            declaration_reuses: nocter_semantic_computation::declaration_reuse_count(
                &self.database,
            ),
            preparation_executions: nocter_semantic_computation::preparation_execution_count(
                &self.database,
            ),
            preparation_reuses: nocter_semantic_computation::preparation_reuse_count(
                &self.database,
            ),
            body_name_executions: nocter_semantic_computation::body_name_execution_count(
                &self.database,
            ),
            body_name_reuses: nocter_semantic_computation::body_name_reuse_count(&self.database),
            typed_body_executions: nocter_semantic_computation::typed_body_execution_count(
                &self.database,
            ),
            typed_body_reuses: nocter_semantic_computation::typed_body_reuse_count(&self.database),
            finalization_executions: nocter_semantic_computation::finalization_execution_count(
                &self.database,
            ),
            finalization_reuses: nocter_semantic_computation::finalization_reuse_count(
                &self.database,
            ),
            complete_analysis_executions:
                nocter_semantic_computation::program_analysis_execution_count(&self.database),
            complete_analysis_reuses: nocter_semantic_computation::program_analysis_reuse_count(
                &self.database,
            ),
            incomplete_analysis_executions:
                nocter_semantic_computation::incomplete_analysis_execution_count(&self.database),
            incomplete_analysis_reuses:
                nocter_semantic_computation::incomplete_analysis_reuse_count(&self.database),
            unit_analysis_executions: nocter_semantic_computation::unit_analysis_execution_count(
                &self.database,
            ),
            unit_analysis_reuses: nocter_semantic_computation::unit_analysis_reuse_count(
                &self.database,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompilerComputationStatistics {
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
    Scope(nocter_semantic_computation::ScopeInputError),
}

impl std::fmt::Display for CompilerComputationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Computation(error) => error.fmt(formatter),
            Self::Scope(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CompilerComputationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Computation(error) => Some(error),
            Self::Scope(error) => Some(error),
        }
    }
}

impl From<ComputationError> for CompilerComputationError {
    fn from(error: ComputationError) -> Self {
        Self::Computation(error)
    }
}

impl From<nocter_semantic_computation::ScopeInputError> for CompilerComputationError {
    fn from(error: nocter_semantic_computation::ScopeInputError) -> Self {
        Self::Scope(error)
    }
}
