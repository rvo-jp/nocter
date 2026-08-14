//! Expression-valued control flow normalized into ordinary MIR blocks.

use super::super::context::LoweringContext;
use crate::mir::{LocalId, ScalarType, ScopeId, Terminator, ValueRepresentation};

pub(in crate::mir::lower) fn lower_conditional_to_place(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    conditional: &crate::ast::IfStmt,
    ty: crate::semantic::TyId,
    representation: ValueRepresentation,
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
        join_target: Some(join_target),
    })?;

    context.control_flow.select_block(then_target)?;
    let then_returns = super::super::statements::lower_value_block(
        context,
        &conditional.then_block,
        destination,
        ty,
        representation,
        then_scope,
        false,
    )?;
    if !then_returns {
        context.control_flow.terminate(Terminator::Goto {
            target: join_target,
        })?;
    }

    context.control_flow.select_block(else_target)?;
    let else_returns = super::super::statements::lower_value_block(
        context,
        else_block,
        destination,
        ty,
        representation,
        else_scope,
        false,
    )?;
    if !else_returns {
        context.control_flow.terminate(Terminator::Goto {
            target: join_target,
        })?;
    }
    context.control_flow.select_block(join_target)
}

pub(in crate::mir::lower) fn lower_match_to_place(
    context: &mut LoweringContext<'_>,
    match_: &crate::ast::SwitchStmt,
    destination: LocalId,
    ty: crate::semantic::TyId,
    representation: ValueRepresentation,
    parent_scope: ScopeId,
) -> Result<(), super::super::BuildError> {
    let source_ty = super::super::coverage::known_expression_type(
        &match_.expression,
        context.semantic.typed_hir,
    )
    .ok_or(super::super::BuildError::MissingTypedExpression)?;
    let source_scalar = super::super::coverage::value_scalar_type(source_ty, context.semantic)
        .filter(|scalar| *scalar == ScalarType::U8)
        .ok_or(super::super::BuildError::UnsupportedClaimedExpression)?;
    let source =
        context.lower_operand(&match_.expression, source_ty, source_scalar, parent_scope)?;
    let bool_ty = context
        .semantic
        .typed_hir
        .type_id(&crate::ast::TypeExpr::Reference(
            crate::ast::TypeReference {
                span: match_.span,
                name: "bool".to_string(),
            },
        ))
        .ok_or(super::super::BuildError::MissingTypedExpression)?;
    let join_target = context.control_flow.reserve_block(parent_scope);
    let arm_scopes = match_
        .arms
        .iter()
        .map(|arm| context.child_scope(parent_scope, arm.body.span))
        .collect::<Vec<_>>();
    let arm_targets = arm_scopes
        .iter()
        .map(|scope| context.control_flow.reserve_block(*scope))
        .collect::<Vec<_>>();
    let wildcard = match_.wildcard_arm.as_ref().map(|wildcard| {
        let scope = context.child_scope(parent_scope, wildcard.body.span);
        let target = context.control_flow.reserve_block(scope);
        (wildcard, scope, target)
    });
    if arm_targets.is_empty() {
        return Err(super::super::BuildError::UnsupportedClaimedExpression);
    }

    let compared_arms = if wildcard.is_some() {
        match_.arms.len()
    } else {
        match_.arms.len().saturating_sub(1)
    };
    for (index, arm) in match_.arms.iter().take(compared_arms).enumerate() {
        let tag = super::super::coverage::payloadless_enum_variant_tag_at(
            arm.variant_name_span,
            context.semantic,
        )
        .ok_or(super::super::BuildError::UnsupportedClaimedExpression)?;
        let condition = LocalId::from_index(context.locals.len());
        context.locals.push(crate::mir::Local::scalar(
            bool_ty,
            ScalarType::Bool,
            crate::mir::LocalStorage::Local,
            crate::mir::LocalOrigin::Desugared(arm.variant_name_span),
            parent_scope,
        ));
        context
            .control_flow
            .push_statement(crate::mir::Statement::Assign {
                destination: crate::mir::Place::local(condition),
                value: crate::mir::Rvalue::Compare {
                    operator: crate::mir::ComparisonOperator::Equal,
                    left: source.clone(),
                    right: crate::mir::Operand::Constant(crate::mir::Constant {
                        ty: source_ty,
                        scalar: ScalarType::U8,
                        value: u128::from(tag),
                    }),
                    operand_ty: source_ty,
                    operand_scalar: ScalarType::U8,
                    result_ty: bool_ty,
                },
                origin: crate::mir::Origin::Desugared(arm.variant_name_span),
            })?;
        let else_target = if index + 1 < compared_arms {
            context.control_flow.reserve_block(parent_scope)
        } else if let Some((_, _, target)) = wildcard {
            target
        } else {
            arm_targets[index + 1]
        };
        context.control_flow.terminate(Terminator::Switch {
            condition: crate::mir::Operand::Copy(crate::mir::Place::local(condition)),
            then_target: arm_targets[index],
            else_target,
            join_target: Some(join_target),
        })?;
        if index + 1 < compared_arms {
            context.control_flow.select_block(else_target)?;
        }
    }
    if compared_arms == 0 {
        context.control_flow.terminate(Terminator::Goto {
            target: wildcard.map_or(arm_targets[0], |(_, _, target)| target),
        })?;
    }

    for ((arm, scope), target) in match_.arms.iter().zip(arm_scopes).zip(arm_targets) {
        context.control_flow.select_block(target)?;
        let returns = super::super::statements::lower_value_block(
            context,
            &arm.body,
            destination,
            ty,
            representation,
            scope,
            false,
        )?;
        if !returns {
            context.control_flow.terminate(Terminator::Goto {
                target: join_target,
            })?;
        }
    }
    if let Some((wildcard, scope, target)) = wildcard {
        context.control_flow.select_block(target)?;
        let returns = super::super::statements::lower_value_block(
            context,
            &wildcard.body,
            destination,
            ty,
            representation,
            scope,
            false,
        )?;
        if !returns {
            context.control_flow.terminate(Terminator::Goto {
                target: join_target,
            })?;
        }
    }
    context.control_flow.select_block(join_target)
}
