use std::sync::Arc;

use nocter_computation::Database;
use nocter_semantic_computation::{
    DeclarationQueryProduct, ProgramPreparationProduct, SemanticScopeKey,
};

use crate::WorkspaceAnalysisError;

/// Demands the paired declaration and program-preparation products for one published scope.
pub(super) fn demand(
    computation: &Database,
    scope: SemanticScopeKey,
) -> Result<(Arc<DeclarationQueryProduct>, Arc<ProgramPreparationProduct>), WorkspaceAnalysisError>
{
    let declarations = nocter_semantic_computation::declarations(computation, scope.clone())
        .map_err(WorkspaceAnalysisError::computation)?;
    let preparation = nocter_semantic_computation::prepared_program(computation, scope)
        .map_err(WorkspaceAnalysisError::computation)?;
    Ok((declarations, preparation))
}
