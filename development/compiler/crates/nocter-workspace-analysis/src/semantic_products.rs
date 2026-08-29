use std::sync::Arc;

use nocter_computation::Database;
use nocter_semantic_computation::SemanticScopeKey;

use crate::WorkspaceAnalysisError;

pub(super) fn demand_incomplete(
    computation: &Database,
    scope: SemanticScopeKey,
) -> Result<Arc<nocter_semantic_computation::IncompleteAnalysisProduct>, WorkspaceAnalysisError> {
    nocter_semantic_computation::incomplete_analysis(computation, scope)
        .map_err(WorkspaceAnalysisError::computation)
}

/// Demands the sole closed source-complete semantic product for one published scope.
pub(super) fn demand_complete(
    computation: &Database,
    scope: SemanticScopeKey,
) -> Result<Arc<nocter_semantic_computation::ProgramAnalysisProduct>, WorkspaceAnalysisError> {
    nocter_semantic_computation::analyzed_program(computation, scope)
        .map_err(WorkspaceAnalysisError::computation)
}
