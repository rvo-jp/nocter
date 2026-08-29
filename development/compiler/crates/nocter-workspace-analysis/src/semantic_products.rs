use std::sync::Arc;

use nocter_computation::Database;
use nocter_semantic_computation::{
    DeclarationQueryOutcome, DeclarationQueryProduct, ProgramPreparationOutcome,
    ProgramPreparationProduct, ResolvedBodyNameSet, SemanticScopeKey,
};

use crate::WorkspaceAnalysisError;

pub(super) struct DemandedSemanticProducts {
    pub(super) declarations: Arc<DeclarationQueryProduct>,
    pub(super) preparation: Arc<ProgramPreparationProduct>,
    pub(super) body_names: Option<ResolvedBodyNameSet>,
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
    let body_names = match (declarations.outcome(), preparation.outcome()) {
        (DeclarationQueryOutcome::Accepted(_), ProgramPreparationOutcome::Prepared(_)) => {
            nocter_semantic_computation::resolved_body_names(computation, scope)
                .map_err(WorkspaceAnalysisError::computation)?
        }
        _ => None,
    };
    Ok(DemandedSemanticProducts {
        declarations,
        preparation,
        body_names,
    })
}
