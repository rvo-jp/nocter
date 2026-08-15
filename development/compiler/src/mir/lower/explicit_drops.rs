//! Explicit source `drop` statements projected onto checked MIR ownership edges.

use super::{BuildError, LoweringContext, SemanticInputs};
use crate::ast::DropStmt;

pub(super) fn is_supported(statement: &DropStmt, semantic: SemanticInputs<'_>) -> bool {
    semantic
        .resolved
        .local_symbol_id_for_reference_span(statement.name_span)
        .and_then(|symbol| semantic.typed_hir.binding_type_expr(symbol))
        .is_some_and(|ty| {
            super::super::drop_plans::is_copy(ty, semantic.resolved, semantic.resolved_sources)
                == Some(false)
                && super::super::drop_plans::is_supported(
                    ty,
                    semantic.resolved,
                    semantic.resolved_sources,
                    semantic.typed_hir,
                )
        })
}

pub(super) fn lower(
    context: &mut LoweringContext<'_>,
    statement: &DropStmt,
) -> Result<(), BuildError> {
    let symbol = context
        .semantic
        .resolved
        .local_symbol_id_for_reference_span(statement.name_span)
        .ok_or(BuildError::MissingLocalSymbol)?;
    let place = context
        .places_by_symbol
        .get(&symbol)
        .copied()
        .ok_or(BuildError::MissingLocalSymbol)?;
    let plan = context.locals[place.local.index()]
        .drop_plan
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
    context.control_flow.emit_drop(place, plan)
}
