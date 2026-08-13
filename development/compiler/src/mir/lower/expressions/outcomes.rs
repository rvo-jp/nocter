//! Scalar outcome recovery normalized into explicit MIR success/failure edges.

use super::{lower_call, lower_expression_to_place};
use crate::ast::Expr;
use crate::mir::{LocalId, ScalarType, Scope, ScopeId, Terminator};
use crate::resolve::LocalSymbolId;
use std::collections::HashMap;

pub(super) fn lower_otherwise_to_place(
    destination: LocalId,
    otherwise: &crate::ast::OtherwiseExpr,
    ty: crate::semantic::TyId,
    scalar: ScalarType,
    semantic: super::super::SemanticInputs<'_>,
    locals: &HashMap<LocalSymbolId, LocalId>,
    local_declarations: &mut Vec<crate::mir::Local>,
    projections: &mut Vec<crate::mir::ProjectionPath>,
    control_flow: &mut super::super::body_builder::ControlFlowBuilder,
    scopes: &mut Vec<Scope>,
    parent_scope: ScopeId,
) -> Result<(), super::super::BuildError> {
    let Expr::Call(call) = otherwise.value.without_groups() else {
        return Err(super::super::BuildError::UnsupportedClaimedExpression);
    };
    let call_source = semantic
        .typed_hir
        .expression(call.span)
        .ok_or(super::super::BuildError::MissingTypedExpression)?
        .id;
    let (callee, arguments, returns_never) = lower_call(
        call,
        semantic,
        locals,
        local_declarations,
        projections,
        control_flow,
        scopes,
        parent_scope,
    )?;
    if returns_never {
        return Err(super::super::BuildError::UnsupportedClaimedExpression);
    }
    let fallback = super::super::coverage::scalar_branch_result(&otherwise.fallback)
        .ok_or(super::super::BuildError::UnsupportedClaimedExpression)?;
    let fallback_scope = ScopeId::from_index(scopes.len());
    scopes.push(Scope::child(parent_scope, otherwise.fallback.span));
    let success = control_flow.begin_handled_outcome_call(
        call_source,
        callee,
        arguments,
        destination,
        fallback_scope,
    )?;
    lower_expression_to_place(
        destination,
        fallback,
        ty,
        scalar,
        semantic,
        locals,
        local_declarations,
        projections,
        control_flow,
        scopes,
        fallback_scope,
    )?;
    control_flow.terminate(Terminator::Goto { target: success })?;
    control_flow.select_block(success)
}
