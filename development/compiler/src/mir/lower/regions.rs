//! Construction of lexical allocation-region state and cleanup identity.

use super::BuildError;
use super::context::LoweringContext;
use crate::ast::{Expr, RegionStmt};
use crate::mir::{
    AllocationRegion, Local, LocalId, LocalOrigin, LocalStorage, Origin, OwnershipKind, Place,
    RegionId, ScopeId, Statement,
};

pub(super) struct EnteredRegion {
    pub(super) scope: ScopeId,
}

pub(super) fn enter(
    context: &mut LoweringContext<'_>,
    statement: &RegionStmt,
    parent_scope: ScopeId,
) -> Result<EnteredRegion, BuildError> {
    let Expr::Identifier(parent) = statement.allocator.without_groups() else {
        return Err(BuildError::UnsupportedClaimedExpression);
    };
    let parent_symbol = context
        .semantic
        .resolved
        .local_symbol_for_identifier(parent)
        .ok_or(BuildError::MissingLocalSymbol)?;
    let parent = *context
        .places_by_symbol
        .get(&parent_symbol.id)
        .ok_or(BuildError::MissingLocalSymbol)?;
    let symbol = context
        .semantic
        .resolved
        .local_symbol_id_at_name_span(statement.name_span)
        .ok_or(BuildError::MissingLocalSymbol)?;
    let type_expr = context
        .semantic
        .typed_hir
        .binding_type_expr(symbol)
        .ok_or(BuildError::MissingTypedExpression)?;
    let ty = context
        .semantic
        .typed_hir
        .type_id(type_expr)
        .ok_or(BuildError::MissingTypedExpression)?;
    let scope = context.child_scope(parent_scope, statement.body.span);
    let body = context.control_flow.reserve_block(scope);
    context
        .control_flow
        .terminate(crate::mir::Terminator::Goto { target: body })?;
    context.control_flow.select_block(body)?;

    let allocator = LocalId::from_index(context.locals.len());
    context.locals.push(Local::aggregate(
        ty,
        OwnershipKind::Copy,
        LocalStorage::Local,
        LocalOrigin::Binding(symbol),
        scope,
    ));
    context
        .places_by_symbol
        .insert(symbol, Place::local(allocator));
    let usize_ty = context
        .semantic
        .typed_hir
        .type_id(&crate::ast::TypeExpr::Reference(
            crate::ast::TypeReference {
                span: statement.keyword_span,
                name: "usize".to_string(),
            },
        ))
        .ok_or(BuildError::MissingTypedExpression)?;
    let mut hidden = || {
        let local = LocalId::from_index(context.locals.len());
        context.locals.push(Local::scalar(
            usize_ty,
            crate::mir::ScalarType::Usize,
            LocalStorage::Local,
            LocalOrigin::Desugared(statement.span),
            scope,
        ));
        local
    };
    let parent_state = hidden();
    let parent_kind = hidden();
    let state = hidden();
    let region = RegionId::from_index(context.allocation_regions.len());
    context.allocation_regions.push(AllocationRegion {
        id: region,
        scope,
        allocator,
        parent,
        state,
        parent_state,
        parent_kind,
    });
    context
        .control_flow
        .push_statement(Statement::EnterRegion {
            region,
            origin: Origin::Desugared(statement.keyword_span),
        })?;
    Ok(EnteredRegion { scope })
}
