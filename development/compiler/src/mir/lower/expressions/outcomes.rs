//! Scalar outcome recovery normalized into explicit MIR success/failure edges.

use super::super::context::LoweringContext;
use crate::ast::Expr;
use crate::mir::{LocalId, ScalarType, ScopeId, Terminator};

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
        destination,
        call,
        &otherwise.fallback,
        ty,
        scalar,
        parent_scope,
    )
}

pub(super) fn lower_discard_catch_to_place(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    catch: &crate::ast::CatchExpr,
    ty: crate::semantic::TyId,
    scalar: ScalarType,
    parent_scope: ScopeId,
) -> Result<(), super::super::BuildError> {
    if !matches!(catch.binding, crate::ast::CatchBinding::Discard { .. }) {
        return Err(super::super::BuildError::UnsupportedClaimedExpression);
    }
    let Expr::Call(call) = catch.expression.without_groups() else {
        return Err(super::super::BuildError::UnsupportedClaimedExpression);
    };
    lower_recovery_to_place(
        context,
        destination,
        call,
        &catch.catch_block,
        ty,
        scalar,
        parent_scope,
    )
}

fn lower_recovery_to_place(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    call: &crate::ast::CallExpr,
    fallback_block: &crate::ast::Block,
    ty: crate::semantic::TyId,
    scalar: ScalarType,
    parent_scope: ScopeId,
) -> Result<(), super::super::BuildError> {
    if fallback_block.result.is_none() {
        return Err(super::super::BuildError::UnsupportedClaimedExpression);
    }
    let call_source = context
        .semantic
        .typed_hir
        .expression(call.span)
        .ok_or(super::super::BuildError::MissingTypedExpression)?
        .id;
    let (callee, arguments, returns_never) = context.lower_call(call, parent_scope)?;
    if returns_never {
        return Err(super::super::BuildError::UnsupportedClaimedExpression);
    }
    let fallback_scope = context.child_scope(parent_scope, fallback_block.span);
    let success = context.control_flow.begin_handled_outcome_call(
        call_source,
        callee,
        arguments,
        destination,
        fallback_scope,
    )?;
    super::super::statements::lower_value_block(
        context,
        fallback_block,
        destination,
        ty,
        scalar,
        fallback_scope,
    )?;
    context
        .control_flow
        .terminate(Terminator::Goto { target: success })?;
    context.control_flow.select_block(success)
}
