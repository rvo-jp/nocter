use std::sync::Arc;

use nocter_computation::Database;
use nocter_semantic_computation::{
    DeclarationQueryOutcome, DeclarationQueryProduct, ProgramFinalizationProduct,
    ProgramPreparationOutcome, ProgramPreparationProduct, SemanticScopeKey,
};

use crate::WorkspaceAnalysisError;

pub(super) struct DemandedSemanticProducts {
    pub(super) declarations: Arc<DeclarationQueryProduct>,
    pub(super) preparation: Arc<ProgramPreparationProduct>,
    pub(super) finalization: Option<Arc<ProgramFinalizationProduct>>,
}

pub(super) fn demand_incomplete(
    computation: &Database,
    scope: SemanticScopeKey,
) -> Result<Arc<nocter_semantic_computation::IncompleteAnalysisProduct>, WorkspaceAnalysisError> {
    nocter_semantic_computation::incomplete_analysis(computation, scope)
        .map_err(WorkspaceAnalysisError::computation)
}

/// Demands the paired declaration and program-preparation products for one published scope.
pub(super) fn demand(
    computation: &Database,
    scope: &SemanticScopeKey,
) -> Result<DemandedSemanticProducts, WorkspaceAnalysisError> {
    let declarations = nocter_semantic_computation::declarations(computation, scope.clone())
        .map_err(WorkspaceAnalysisError::computation)?;
    let preparation = nocter_semantic_computation::prepared_program(computation, scope.clone())
        .map_err(WorkspaceAnalysisError::computation)?;
    let finalization = if matches!(
        (declarations.outcome(), preparation.outcome()),
        (
            DeclarationQueryOutcome::Accepted(_),
            ProgramPreparationOutcome::Prepared(_)
        )
    ) {
        Some(
            nocter_semantic_computation::finalized_program(computation, scope.clone())
                .map_err(WorkspaceAnalysisError::computation)?,
        )
    } else {
        None
    };
    Ok(DemandedSemanticProducts {
        declarations,
        preparation,
        finalization,
    })
}
