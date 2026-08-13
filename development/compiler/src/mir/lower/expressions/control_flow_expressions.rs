//! Expression-valued control flow normalized into ordinary MIR blocks.

use super::{lower_expression_to_place, lower_operand};
use crate::mir::{LocalId, ScalarType, Scope, ScopeId, Terminator};
use crate::resolve::LocalSymbolId;
use std::collections::HashMap;

pub(in crate::mir::lower) fn lower_conditional_to_place(
    destination: LocalId,
    conditional: &crate::ast::IfStmt,
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
    let condition_ty =
        super::super::coverage::known_expression_type(&conditional.condition, semantic.typed_hir)
            .ok_or(super::super::BuildError::MissingTypedExpression)?;
    let condition = lower_operand(
        &conditional.condition,
        condition_ty,
        ScalarType::Bool,
        semantic,
        locals,
        local_declarations,
        projections,
        control_flow,
        scopes,
        parent_scope,
    )?;
    let then_result = super::super::coverage::scalar_branch_result(&conditional.then_block)
        .ok_or(super::super::BuildError::UnsupportedClaimedExpression)?;
    let else_block = conditional
        .else_block
        .as_ref()
        .ok_or(super::super::BuildError::UnsupportedClaimedExpression)?;
    let else_result = super::super::coverage::scalar_branch_result(else_block)
        .ok_or(super::super::BuildError::UnsupportedClaimedExpression)?;

    let then_scope = ScopeId::from_index(scopes.len());
    scopes.push(Scope::child(parent_scope, conditional.then_block.span));
    let else_scope = ScopeId::from_index(scopes.len());
    scopes.push(Scope::child(parent_scope, else_block.span));
    let then_target = control_flow.reserve_block(then_scope);
    let else_target = control_flow.reserve_block(else_scope);
    let join_target = control_flow.reserve_block(parent_scope);
    control_flow.terminate(Terminator::Switch {
        condition,
        then_target,
        else_target,
    })?;

    control_flow.select_block(then_target)?;
    lower_expression_to_place(
        destination,
        then_result,
        ty,
        scalar,
        semantic,
        locals,
        local_declarations,
        projections,
        control_flow,
        scopes,
        then_scope,
    )?;
    control_flow.terminate(Terminator::Goto {
        target: join_target,
    })?;

    control_flow.select_block(else_target)?;
    lower_expression_to_place(
        destination,
        else_result,
        ty,
        scalar,
        semantic,
        locals,
        local_declarations,
        projections,
        control_flow,
        scopes,
        else_scope,
    )?;
    control_flow.terminate(Terminator::Goto {
        target: join_target,
    })?;
    control_flow.select_block(join_target)
}
