use std::sync::Arc;

use nocter_computation::Database;
use nocter_semantic_computation::{
    BodyNameSet, DeclarationQueryOutcome, DeclarationQueryProduct, ProgramFinalizationProduct,
    ProgramPreparationOutcome, ProgramPreparationProduct, SemanticScopeKey, TypedBodySet,
};

use crate::WorkspaceAnalysisError;

pub(super) struct DemandedSemanticProducts {
    pub(super) declarations: Arc<DeclarationQueryProduct>,
    pub(super) preparation: Arc<ProgramPreparationProduct>,
    pub(super) body_names: Option<BodyNameSet>,
    pub(super) typed_bodies: Option<TypedBodySet>,
    pub(super) finalization: Option<Arc<ProgramFinalizationProduct>>,
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
    let typed_bodies = if body_names.is_some() {
        nocter_semantic_computation::typed_bodies(computation, scope)
            .map_err(WorkspaceAnalysisError::computation)?
    } else {
        None
    };
    let finalization = if typed_bodies.is_some() {
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
        body_names,
        typed_bodies,
        finalization,
    })
}
