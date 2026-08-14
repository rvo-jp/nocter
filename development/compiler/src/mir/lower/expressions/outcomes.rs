//! Scalar outcome recovery normalized into explicit MIR success/failure edges.

use super::super::context::LoweringContext;
use crate::ast::Expr;
use crate::mir::{LocalId, ScalarType, ScopeId, Terminator};

#[derive(Clone, Copy)]
struct RecoveryDestination {
    local: LocalId,
    ty: crate::semantic::TyId,
    scalar: ScalarType,
    parent_scope: ScopeId,
}

pub(super) fn lower_otherwise_to_place(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    otherwise: &crate::ast::OtherwiseExpr,
    ty: crate::semantic::TyId,
    scalar: ScalarType,
    parent_scope: ScopeId,
) -> Result<(), super::super::BuildError> {
    let Expr::Call(call) = otherwise.value.without_groups() else {
        return Err(super::super::BuildError::UnsupportedClaimedExpression);
    };
    lower_recovery_to_place(
        context,
        RecoveryDestination {
            local: destination,
            ty,
            scalar,
            parent_scope,
        },
        call,
        &otherwise.fallback,
        None,
    )
}

pub(super) fn lower_catch_to_place(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    catch: &crate::ast::CatchExpr,
    ty: crate::semantic::TyId,
    scalar: ScalarType,
    parent_scope: ScopeId,
) -> Result<(), super::super::BuildError> {
    let Expr::Call(call) = catch.expression.without_groups() else {
        return Err(super::super::BuildError::UnsupportedClaimedExpression);
    };
    let fallback_scope = context.child_scope(parent_scope, catch.catch_block.span);
    let failure_payload = match &catch.binding {
        crate::ast::CatchBinding::Discard { .. } => None,
        crate::ast::CatchBinding::Named { span, .. } => {
            let symbol = context
                .semantic
                .resolved
                .local_symbol_id_at_name_span(*span)
                .ok_or(super::super::BuildError::MissingLocalSymbol)?;
            let type_expr = context
                .semantic
                .typed_hir
                .binding_type_expr(symbol)
                .ok_or(super::super::BuildError::MissingTypedExpression)?;
            let error_ty = context
                .semantic
                .typed_hir
                .type_id(type_expr)
                .ok_or(super::super::BuildError::MissingTypedExpression)?;
            let local = LocalId::from_index(context.locals.len());
            context.locals.push(crate::mir::Local::error(
                error_ty,
                crate::mir::LocalStorage::Local,
                crate::mir::LocalOrigin::Binding(symbol),
                fallback_scope,
            ));
            context.locals_by_symbol.insert(symbol, local);
            Some(local)
        }
    };
    lower_recovery_to_place_with_scope(
        context,
        RecoveryDestination {
            local: destination,
            ty,
            scalar,
            parent_scope,
        },
        call,
        &catch.catch_block,
        fallback_scope,
        failure_payload,
    )
}

fn lower_recovery_to_place(
    context: &mut LoweringContext<'_>,
    destination: RecoveryDestination,
    call: &crate::ast::CallExpr,
    fallback_block: &crate::ast::Block,
    failure_payload: Option<LocalId>,
) -> Result<(), super::super::BuildError> {
    let fallback_scope = context.child_scope(destination.parent_scope, fallback_block.span);
    lower_recovery_to_place_with_scope(
        context,
        destination,
        call,
        fallback_block,
        fallback_scope,
        failure_payload,
    )
}

fn lower_recovery_to_place_with_scope(
    context: &mut LoweringContext<'_>,
    destination: RecoveryDestination,
    call: &crate::ast::CallExpr,
    fallback_block: &crate::ast::Block,
    fallback_scope: ScopeId,
    failure_payload: Option<LocalId>,
) -> Result<(), super::super::BuildError> {
    let call_source = context
        .semantic
        .typed_hir
        .expression(call.span)
        .ok_or(super::super::BuildError::MissingTypedExpression)?
        .id;
    let (callee, arguments, returns_never) = context.lower_call(call, destination.parent_scope)?;
    if returns_never {
        return Err(super::super::BuildError::UnsupportedClaimedExpression);
    }
    let success = context.control_flow.begin_handled_outcome_call(
        call_source,
        callee,
        arguments,
        destination.local,
        fallback_scope,
        failure_payload,
    )?;
    let returns = super::super::statements::lower_value_block(
        context,
        fallback_block,
        destination.local,
        destination.ty,
        destination.scalar,
        fallback_scope,
        true,
    )?;
    if !returns {
        context
            .control_flow
            .terminate(Terminator::Goto { target: success })?;
    }
    context.control_flow.select_block(success)
}
