//! Expression-valued control flow normalized into ordinary MIR blocks.

use super::super::context::LoweringContext;
use crate::mir::{LocalId, ScalarType, ScopeId, Terminator};

pub(in crate::mir::lower) fn lower_conditional_to_place(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    conditional: &crate::ast::IfStmt,
    ty: crate::semantic::TyId,
    scalar: ScalarType,
    parent_scope: ScopeId,
) -> Result<(), super::super::BuildError> {
    let condition_ty = super::super::coverage::known_expression_type(
        &conditional.condition,
        context.semantic.typed_hir,
    )
    .ok_or(super::super::BuildError::MissingTypedExpression)?;
    let condition = context.lower_operand(
        &conditional.condition,
        condition_ty,
        ScalarType::Bool,
        parent_scope,
    )?;
    let else_block = conditional
        .else_block
        .as_ref()
        .ok_or(super::super::BuildError::UnsupportedClaimedExpression)?;

    let then_scope = context.child_scope(parent_scope, conditional.then_block.span);
    let else_scope = context.child_scope(parent_scope, else_block.span);
    let then_target = context.control_flow.reserve_block(then_scope);
    let else_target = context.control_flow.reserve_block(else_scope);
    let join_target = context.control_flow.reserve_block(parent_scope);
    context.control_flow.terminate(Terminator::Switch {
        condition,
        then_target,
        else_target,
    })?;

    context.control_flow.select_block(then_target)?;
    super::super::statements::lower_value_block(
        context,
        &conditional.then_block,
        destination,
        ty,
        scalar,
        then_scope,
    )?;
    context.control_flow.terminate(Terminator::Goto {
        target: join_target,
    })?;

    context.control_flow.select_block(else_target)?;
    super::super::statements::lower_value_block(
        context,
        else_block,
        destination,
        ty,
        scalar,
        else_scope,
    )?;
    context.control_flow.terminate(Terminator::Goto {
        target: join_target,
    })?;
    context.control_flow.select_block(join_target)
}
