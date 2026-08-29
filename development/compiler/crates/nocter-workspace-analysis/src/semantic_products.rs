use std::sync::Arc;

use nocter_computation::Database;
use nocter_semantic_computation::{
    CheckedBodySet, DeclarationQueryOutcome, DeclarationQueryProduct, ProgramPreparationOutcome,
    ProgramPreparationProduct, ResolvedBodyNameSet, SemanticScopeKey,
};

use crate::WorkspaceAnalysisError;

pub(super) struct DemandedSemanticProducts {
    pub(super) declarations: Arc<DeclarationQueryProduct>,
    pub(super) preparation: Arc<ProgramPreparationProduct>,
    pub(super) body_names: Option<ResolvedBodyNameSet>,
    pub(super) checked_bodies: Option<CheckedBodySet>,
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
    let checked_bodies = if body_names.is_some() {
        nocter_semantic_computation::checked_bodies(computation, scope)
            .map_err(WorkspaceAnalysisError::computation)?
    } else {
        None
    };
    Ok(DemandedSemanticProducts {
        declarations,
        preparation,
        body_names,
        checked_bodies,
    })
}
