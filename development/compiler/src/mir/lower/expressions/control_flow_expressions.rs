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
    preserve_explicit_return: bool,
) -> Result<(), super::super::BuildError> {
    let source_ty =
        super::super::coverage::handled_outcome_success_type(&match_.expression, context.semantic)
            .or_else(|| {
                super::super::coverage::known_expression_type(
                    &match_.expression,
                    context.semantic.typed_hir,
                )
            })
            .ok_or_else(|| {
                super::super::BuildError::MissingTypedExpression
                    .context("resolve match source type")
            })?;
    let (source, tag_ty, payload_source) =
        if super::super::coverage::value_scalar_type(source_ty, context.semantic)
            == Some(ScalarType::U8)
        {
            (
                context.lower_operand(
                    &match_.expression,
                    source_ty,
                    ScalarType::U8,
                    parent_scope,
                )?,
                source_ty,
                None,
            )
        } else if super::super::coverage::value_representation(source_ty, context.semantic)
            == Some(ValueRepresentation::Aggregate)
        {
            let u8_ty = context
                .semantic
                .typed_hir
                .type_id(&crate::ast::TypeExpr::Reference(
                    crate::ast::TypeReference {
                        span: match_.expression.span(),
                        name: "u8".to_string(),
                    },
                ))
                .ok_or_else(|| {
                    super::super::BuildError::MissingTypedExpression
                        .context("resolve match tag type")
                })?;
            let discriminant = LocalId::from_index(context.locals.len());
            context.locals.push(crate::mir::Local::scalar(
                u8_ty,
                ScalarType::U8,
                crate::mir::LocalStorage::Local,
                crate::mir::LocalOrigin::Desugared(match_.expression.span()),
                parent_scope,
            ));
            let enum_source = match_source_operand(context, &match_.expression, parent_scope)
                .map_err(|error| error.context("lower match source"))?;
            context
                .control_flow
                .push_statement(crate::mir::Statement::Assign {
                    destination: crate::mir::Place::local(discriminant),
                    value: crate::mir::Rvalue::Discriminant {
                        source: crate::mir::Operand::Copy(enum_source),
                        enum_ty: source_ty,
                        result_ty: u8_ty,
                    },
                    origin: crate::mir::Origin::Desugared(match_.expression.span()),
                })?;
            (
                crate::mir::Operand::Copy(crate::mir::Place::local(discriminant)),
                u8_ty,
                Some(enum_source),
            )
        } else {
            return Err(super::super::BuildError::UnsupportedClaimedExpression);
        };
    let bool_ty = context
        .semantic
        .typed_hir
        .type_id(&crate::ast::TypeExpr::Reference(
            crate::ast::TypeReference {
                span: match_.span,
                name: "bool".to_string(),
            },
        ))
        .ok_or_else(|| {
            super::super::BuildError::MissingTypedExpression.context("resolve match condition type")
        })?;
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
    if arm_targets.is_empty() && wildcard.is_none() {
        return Err(super::super::BuildError::UnsupportedClaimedExpression);
    }

    let compared_arms = if wildcard.is_some() {
        match_.arms.len()
    } else {
        match_.arms.len().saturating_sub(1)
    };
    for (index, arm) in match_.arms.iter().take(compared_arms).enumerate() {
        let tag =
            super::super::coverage::enum_variant_tag_at(arm.variant_name_span, context.semantic)
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
                        ty: tag_ty,
                        scalar: ScalarType::U8,
                        value: u128::from(tag),
                    }),
                    operand_ty: tag_ty,
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
            target: wildcard
                .map(|(_, _, target)| target)
                .or_else(|| arm_targets.first().copied())
                .ok_or(super::super::BuildError::UnsupportedClaimedExpression)?,
        })?;
    }

    for ((arm, scope), target) in match_.arms.iter().zip(arm_scopes).zip(arm_targets) {
        context.control_flow.select_block(target)?;
        if let Some(source) = payload_source {
            bind_payload(context, arm, source, source_ty, scope).map_err(|source| {
                super::super::BuildError::Context {
                    operation: "bind match payload",
                    source: Box::new(source),
                }
            })?;
        }
        let returns = super::super::statements::lower_value_block(
            context,
            &arm.body,
            destination,
            ty,
            representation,
            scope,
            preserve_explicit_return,
        )
        .map_err(|error| error.context("lower match arm body"))?;
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
            preserve_explicit_return,
        )?;
        if !returns {
            context.control_flow.terminate(Terminator::Goto {
                target: join_target,
            })?;
        }
    }
    context.control_flow.select_block(join_target)
}

fn match_source_operand(
    context: &mut LoweringContext<'_>,
    expression: &crate::ast::Expr,
    scope: ScopeId,
) -> Result<crate::mir::Place, super::super::BuildError> {
    let expression = match expression.without_groups() {
        crate::ast::Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
            &unary.operand
        }
        _ => expression,
    };
    let place = match expression.without_groups() {
        crate::ast::Expr::Identifier(identifier) => {
            let symbol = context
                .semantic
                .resolved
                .local_symbol_for_identifier(identifier)
                .ok_or(super::super::BuildError::MissingLocalSymbol)?;
            *context
                .places_by_symbol
                .get(&symbol.id)
                .ok_or(super::super::BuildError::MissingLocalSymbol)?
        }
        crate::ast::Expr::Member(member)
            if context
                .semantic
                .typed_hir
                .enum_variant_target(member.member_span)
                .is_none() =>
        {
            let (place, representation) = super::super::projections::lower_field_place(
                member,
                context.semantic,
                &context.places_by_symbol,
                &mut context.projections,
                &mut context.drop_plans,
            )?;
            if representation != ValueRepresentation::Aggregate {
                return Err(super::super::BuildError::UnsupportedClaimedExpression);
            }
            place
        }
        crate::ast::Expr::Index(index) => {
            let (place, representation) =
                super::super::indexes::lower_place(context, index, scope)?;
            if representation != ValueRepresentation::Aggregate {
                return Err(super::super::BuildError::UnsupportedClaimedExpression);
            }
            place
        }
        expression => {
            let ty =
                super::super::coverage::handled_outcome_success_type(expression, context.semantic)
                    .or_else(|| {
                        super::super::coverage::known_expression_type(
                            expression,
                            context.semantic.typed_hir,
                        )
                    })
                    .ok_or(super::super::BuildError::MissingTypedExpression)?;
            if super::super::coverage::value_representation(ty, context.semantic)
                != Some(ValueRepresentation::Aggregate)
            {
                return Err(super::super::BuildError::UnsupportedClaimedExpression);
            }
            let origin = context
                .semantic
                .typed_hir
                .expression(expression.span())
                .map_or(
                    crate::mir::LocalOrigin::Desugared(expression.span()),
                    |expression| crate::mir::LocalOrigin::Temporary(expression.id),
                );
            let local = context.local_for_type(ty, origin, scope)?;
            context.lower_value_to_place(
                local,
                expression,
                ty,
                ValueRepresentation::Aggregate,
                scope,
            )?;
            crate::mir::Place::local(local)
        }
    };
    Ok(place)
}

fn bind_payload(
    context: &mut LoweringContext<'_>,
    arm: &crate::ast::SwitchArm,
    source: crate::mir::Place,
    source_ty: crate::semantic::TyId,
    scope: ScopeId,
) -> Result<(), super::super::BuildError> {
    let Some(binding) = arm.payload.as_ref().and_then(|payload| payload.binding()) else {
        return Ok(());
    };
    let symbol = context
        .semantic
        .resolved
        .local_symbol_id_at_name_span(binding.span)
        .ok_or(super::super::BuildError::MissingLocalSymbol)?;
    let binding_type = context
        .semantic
        .typed_hir
        .binding_type_expr(symbol)
        .ok_or(super::super::BuildError::MissingTypedExpression)?;
    let ty = context
        .semantic
        .typed_hir
        .type_id(binding_type)
        .ok_or(super::super::BuildError::MissingTypedExpression)?;
    let source_type = context
        .semantic
        .typed_hir
        .type_expr_by_id(source_ty)
        .ok_or(super::super::BuildError::MissingTypedExpression)?;
    let source_abi = crate::abi::abi_value_from_type_expr_with_resolver(
        source_type,
        context.semantic.resolved,
        |source| context.semantic.resolver_for(source),
    )
    .map_err(|_| super::super::BuildError::UnsupportedClaimedExpression)?;
    let crate::abi::AbiType::Enum(enum_) = source_abi.ty else {
        return Err(super::super::BuildError::UnsupportedClaimedExpression);
    };
    let payload_offset = u32::try_from(enum_.payload_offset)
        .map_err(|_| super::super::BuildError::UnsupportedClaimedExpression)?;
    let representation = super::super::coverage::value_representation(ty, context.semantic)
        .ok_or(super::super::BuildError::MissingTypedExpression)?;
    let ownership = match context.semantic.typed_hir.payload_binding_mode(symbol) {
        Some(crate::typecheck::TypecheckPayloadBindingMode::Move) => {
            crate::mir::OwnershipKind::Move
        }
        Some(crate::typecheck::TypecheckPayloadBindingMode::Copy) => {
            crate::mir::OwnershipKind::Copy
        }
        None => return Err(super::super::BuildError::MissingTypedExpression),
    };
    let drop_plan = if representation == ValueRepresentation::Aggregate
        && ownership == crate::mir::OwnershipKind::Move
    {
        Some(
            super::super::super::drop_plans::build(
                binding_type,
                context.semantic.resolved,
                context.semantic.resolved_sources,
                context.semantic.typed_hir,
                &mut context.drop_plans,
            )
            .ok_or(super::super::BuildError::UnsupportedClaimedExpression)?,
        )
    } else {
        None
    };
    let projection = crate::mir::ProjectionPathId::from_index(context.projections.len());
    context.projections.push(crate::mir::ProjectionPath {
        id: projection,
        base: source.local,
        parent: source.projection,
        element: crate::mir::ProjectionElement::Field {
            offset: payload_offset,
        },
        ty,
        representation,
        ownership,
        drop_plan,
    });
    let local = LocalId::from_index(context.locals.len());
    let origin = crate::mir::LocalOrigin::Binding(symbol);
    let storage = crate::mir::LocalStorage::Local;
    let mut declaration = match representation {
        ValueRepresentation::Scalar(scalar) => {
            crate::mir::Local::scalar(ty, scalar, storage, origin, scope)
        }
        ValueRepresentation::View(kind) => {
            crate::mir::Local::view(ty, kind, storage, origin, scope)
        }
        ValueRepresentation::Borrow => {
            let readwrite = crate::typecheck::type_expr_borrow_readwrite(
                binding_type,
                context.semantic.resolved,
            )
            .ok_or(super::super::BuildError::MissingTypedExpression)?;
            crate::mir::Local::borrow(ty, readwrite, storage, origin, scope)
        }
        ValueRepresentation::Aggregate => {
            crate::mir::Local::aggregate(ty, ownership, storage, origin, scope)
        }
        ValueRepresentation::Unit | ValueRepresentation::Error => {
            return Err(super::super::BuildError::UnsupportedClaimedExpression);
        }
    };
    declaration.drop_plan = drop_plan;
    context.locals.push(declaration);
    context
        .places_by_symbol
        .insert(symbol, crate::mir::Place::local(local));
    if representation == ValueRepresentation::Aggregate {
        context
            .control_flow
            .push_statement(crate::mir::Statement::BeginAggregate {
                destination: crate::mir::Place::local(local),
                origin: crate::mir::Origin::Desugared(binding.span),
            })?;
    }
    context
        .control_flow
        .push_statement(crate::mir::Statement::Assign {
            destination: crate::mir::Place::local(local),
            value: crate::mir::Rvalue::Use(if ownership == crate::mir::OwnershipKind::Move {
                crate::mir::Operand::Move(crate::mir::Place::projected(source.local, projection))
            } else {
                crate::mir::Operand::Copy(crate::mir::Place::projected(source.local, projection))
            }),
            origin: crate::mir::Origin::Desugared(binding.span),
        })
}

pub(in crate::mir::lower) fn lower_match_statement(
    context: &mut LoweringContext<'_>,
    match_: &crate::ast::SwitchStmt,
    parent_scope: ScopeId,
) -> Result<bool, super::super::BuildError> {
    let exits = match_
        .arms
        .iter()
        .all(|arm| block_has_terminal_return(&arm.body))
        && match_
            .wildcard_arm
            .as_ref()
            .is_none_or(|arm| block_has_terminal_return(&arm.body));
    let ty = context
        .semantic
        .typed_hir
        .type_id(&crate::ast::TypeExpr::Reference(
            crate::ast::TypeReference {
                span: match_.span,
                name: "void".to_string(),
            },
        ))
        .ok_or(super::super::BuildError::MissingTypedExpression)?;
    let destination = LocalId::from_index(context.locals.len());
    context.locals.push(crate::mir::Local::unit(
        ty,
        crate::mir::LocalStorage::Local,
        crate::mir::LocalOrigin::Desugared(match_.span),
        parent_scope,
    ));
    lower_match_to_place(
        context,
        match_,
        destination,
        ty,
        ValueRepresentation::Unit,
        parent_scope,
        true,
    )
    .map_err(|error| error.context("lower match pattern"))?;
    if exits {
        context.control_flow.terminate(Terminator::Trap)?;
    }
    Ok(exits)
}

pub(in crate::mir::lower) fn lower_if_is_statement(
    context: &mut LoweringContext<'_>,
    statement: &crate::ast::IfIsStmt,
    parent_scope: ScopeId,
) -> Result<bool, super::super::BuildError> {
    let wildcard = statement.else_block.clone().unwrap_or(crate::ast::Block {
        span: statement.span,
        statements: Vec::new(),
        result: None,
    });
    let match_ = crate::ast::SwitchStmt {
        span: statement.span,
        expression: statement.expression.clone(),
        arms: vec![crate::ast::SwitchArm {
            span: statement.pattern_span,
            enum_name: statement.enum_name.clone(),
            enum_name_span: statement.enum_name_span,
            variant_name: statement.variant_name.clone(),
            variant_name_span: statement.variant_name_span,
            payload: statement.payload.clone(),
            body: statement.then_block.clone(),
        }],
        wildcard_arm: Some(crate::ast::SwitchWildcardArm {
            span: wildcard.span,
            body: wildcard,
        }),
    };
    let exits = block_has_terminal_return(&statement.then_block)
        && statement
            .else_block
            .as_ref()
            .is_some_and(block_has_terminal_return);
    let ty = context
        .semantic
        .typed_hir
        .type_id(&crate::ast::TypeExpr::Reference(
            crate::ast::TypeReference {
                span: statement.span,
                name: "void".to_string(),
            },
        ))
        .ok_or(super::super::BuildError::MissingTypedExpression)?;
    let destination = LocalId::from_index(context.locals.len());
    context.locals.push(crate::mir::Local::unit(
        ty,
        crate::mir::LocalStorage::Local,
        crate::mir::LocalOrigin::Desugared(statement.span),
        parent_scope,
    ));
    lower_match_to_place(
        context,
        &match_,
        destination,
        ty,
        ValueRepresentation::Unit,
        parent_scope,
        true,
    )
    .map_err(|error| error.context("lower if-is pattern"))?;
    if exits {
        context.control_flow.terminate(Terminator::Trap)?;
    }
    Ok(exits)
}

fn block_has_terminal_return(block: &crate::ast::Block) -> bool {
    matches!(block.statements.last(), Some(crate::ast::Stmt::Return(_)))
}
