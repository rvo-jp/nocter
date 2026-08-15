//! Scalar outcome recovery normalized into explicit MIR success/failure edges.

use super::super::context::LoweringContext;
use crate::ast::Expr;
use crate::mir::{LocalId, ScalarType, ScopeId, Terminator, ValueRepresentation};

#[derive(Clone, Copy)]
struct RecoveryDestination {
    local: LocalId,
    ty: crate::semantic::TyId,
    representation: ValueRepresentation,
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
    lower_recovery_to_place(
        context,
        RecoveryDestination {
            local: destination,
            ty,
            representation: ValueRepresentation::Scalar(scalar),
            parent_scope,
        },
        &otherwise.value,
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
    let fallback_scope = context.child_scope(parent_scope, catch.catch_block.span);
    let failure_payload = catch_failure_payload(context, catch, fallback_scope)?;
    lower_recovery_to_place_with_scope(
        context,
        RecoveryDestination {
            local: destination,
            ty,
            representation: ValueRepresentation::Scalar(scalar),
            parent_scope,
        },
        &catch.expression,
        &catch.catch_block,
        fallback_scope,
        failure_payload,
    )
}

pub(super) fn lower_aggregate_otherwise_to_place(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    otherwise: &crate::ast::OtherwiseExpr,
    ty: crate::semantic::TyId,
    parent_scope: ScopeId,
) -> Result<(), super::super::BuildError> {
    lower_recovery_to_place(
        context,
        RecoveryDestination {
            local: destination,
            ty,
            representation: ValueRepresentation::Aggregate,
            parent_scope,
        },
        &otherwise.value,
        &otherwise.fallback,
        None,
    )
}

pub(super) fn lower_aggregate_catch_to_place(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    catch: &crate::ast::CatchExpr,
    ty: crate::semantic::TyId,
    parent_scope: ScopeId,
) -> Result<(), super::super::BuildError> {
    let fallback_scope = context.child_scope(parent_scope, catch.catch_block.span);
    let failure_payload = catch_failure_payload(context, catch, fallback_scope)?;
    lower_recovery_to_place_with_scope(
        context,
        RecoveryDestination {
            local: destination,
            ty,
            representation: ValueRepresentation::Aggregate,
            parent_scope,
        },
        &catch.expression,
        &catch.catch_block,
        fallback_scope,
        failure_payload,
    )
}

pub(super) fn lower_view_otherwise_to_place(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    otherwise: &crate::ast::OtherwiseExpr,
    ty: crate::semantic::TyId,
    kind: crate::mir::ViewKind,
    parent_scope: ScopeId,
) -> Result<(), super::super::BuildError> {
    lower_recovery_to_place(
        context,
        RecoveryDestination {
            local: destination,
            ty,
            representation: ValueRepresentation::View(kind),
            parent_scope,
        },
        &otherwise.value,
        &otherwise.fallback,
        None,
    )
}

pub(super) fn lower_view_catch_to_place(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    catch: &crate::ast::CatchExpr,
    ty: crate::semantic::TyId,
    kind: crate::mir::ViewKind,
    parent_scope: ScopeId,
) -> Result<(), super::super::BuildError> {
    let fallback_scope = context.child_scope(parent_scope, catch.catch_block.span);
    let failure_payload = catch_failure_payload(context, catch, fallback_scope)?;
    lower_recovery_to_place_with_scope(
        context,
        RecoveryDestination {
            local: destination,
            ty,
            representation: ValueRepresentation::View(kind),
            parent_scope,
        },
        &catch.expression,
        &catch.catch_block,
        fallback_scope,
        failure_payload,
    )
}

pub(super) fn lower_borrow_otherwise_to_place(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    otherwise: &crate::ast::OtherwiseExpr,
    ty: crate::semantic::TyId,
    parent_scope: ScopeId,
) -> Result<(), super::super::BuildError> {
    lower_recovery_to_place(
        context,
        RecoveryDestination {
            local: destination,
            ty,
            representation: ValueRepresentation::Borrow,
            parent_scope,
        },
        &otherwise.value,
        &otherwise.fallback,
        None,
    )
}

pub(super) fn lower_borrow_catch_to_place(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    catch: &crate::ast::CatchExpr,
    ty: crate::semantic::TyId,
    parent_scope: ScopeId,
) -> Result<(), super::super::BuildError> {
    let fallback_scope = context.child_scope(parent_scope, catch.catch_block.span);
    let failure_payload = catch_failure_payload(context, catch, fallback_scope)?;
    lower_recovery_to_place_with_scope(
        context,
        RecoveryDestination {
            local: destination,
            ty,
            representation: ValueRepresentation::Borrow,
            parent_scope,
        },
        &catch.expression,
        &catch.catch_block,
        fallback_scope,
        failure_payload,
    )
}

fn catch_failure_payload(
    context: &mut LoweringContext<'_>,
    catch: &crate::ast::CatchExpr,
    fallback_scope: ScopeId,
) -> Result<Option<LocalId>, super::super::BuildError> {
    Ok(match &catch.binding {
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
            context
                .places_by_symbol
                .insert(symbol, crate::mir::Place::local(local));
            Some(local)
        }
    })
}

fn lower_recovery_to_place(
    context: &mut LoweringContext<'_>,
    destination: RecoveryDestination,
    source_expression: &Expr,
    fallback_block: &crate::ast::Block,
    failure_payload: Option<LocalId>,
) -> Result<(), super::super::BuildError> {
    let fallback_scope = context.child_scope(destination.parent_scope, fallback_block.span);
    lower_recovery_to_place_with_scope(
        context,
        destination,
        source_expression,
        fallback_block,
        fallback_scope,
        failure_payload,
    )
}

fn lower_recovery_to_place_with_scope(
    context: &mut LoweringContext<'_>,
    destination: RecoveryDestination,
    source_expression: &Expr,
    fallback_block: &crate::ast::Block,
    fallback_scope: ScopeId,
    failure_payload: Option<LocalId>,
) -> Result<(), super::super::BuildError> {
    let source = context
        .semantic
        .typed_hir
        .expression(source_expression.span())
        .ok_or(super::super::BuildError::MissingTypedExpression)?
        .id;
    let source_ty = outcome_source_type(context, source_expression)
        .ok_or(super::super::BuildError::MissingTypedExpression)?;
    let source_shape = outcome_shape(context, source_ty)?;
    let success = match source_expression.without_groups() {
        Expr::Call(call) if source_shape.layers.len() == 1 => {
            let (callee, arguments, returns_never) =
                context.lower_call(call, destination.parent_scope)?;
            if returns_never {
                return Err(super::super::BuildError::UnsupportedClaimedExpression);
            }
            context.control_flow.begin_handled_outcome_call(
                source,
                callee,
                arguments,
                destination.local,
                fallback_scope,
                failure_payload,
            )?
        }
        Expr::Identifier(_) => {
            let (stored, layer) = stored_outcome_source(context, source_expression)?;
            context.control_flow.begin_stored_outcome_inspection(
                crate::mir::Origin::Expression(source),
                stored_outcome_operand(context, stored),
                layer,
                destination.local,
                fallback_scope,
                failure_payload,
            )?
        }
        _ => {
            let stored = context.aggregate_temporary(
                source_ty,
                crate::mir::LocalOrigin::Temporary(source),
                destination.parent_scope,
            )?;
            context
                .lower_value_to_place(
                    stored,
                    source_expression,
                    source_ty,
                    crate::mir::ValueRepresentation::Aggregate,
                    destination.parent_scope,
                )
                .map_err(|error| error.context("materialize recovered outcome source"))?;
            let layer = *source_shape
                .layers
                .first()
                .ok_or(super::super::BuildError::UnsupportedClaimedExpression)?;
            context.control_flow.begin_stored_outcome_inspection(
                crate::mir::Origin::Expression(source),
                stored_outcome_operand(context, crate::mir::Place::local(stored)),
                layer,
                destination.local,
                fallback_scope,
                failure_payload,
            )?
        }
    };
    let returns = super::super::statements::lower_value_block(
        context,
        fallback_block,
        destination.local,
        destination.ty,
        destination.representation,
        fallback_scope,
        true,
    )
    .map_err(|error| error.context("lower recovery fallback"))?;
    if !returns {
        context
            .control_flow
            .terminate(Terminator::Goto { target: success })?;
    }
    context.control_flow.select_block(success)
}

fn outcome_shape(
    context: &LoweringContext<'_>,
    ty: crate::semantic::TyId,
) -> Result<crate::outcomes::OutcomeShape, super::super::BuildError> {
    let type_expr = context
        .semantic
        .typed_hir
        .type_expr_by_id(ty)
        .ok_or(super::super::BuildError::MissingTypedExpression)?;
    let shape = crate::outcomes::outcome_shape_with_resolver(
        type_expr,
        context.semantic.resolved,
        |source| context.semantic.resolver_for(source),
    );
    (!shape.layers.is_empty())
        .then_some(shape)
        .ok_or(super::super::BuildError::UnsupportedClaimedExpression)
}

pub(super) fn lower_terminal_stored_outcome_to_place(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    expression: &Expr,
    failure: Terminator,
    scope: ScopeId,
) -> Result<(), super::super::BuildError> {
    let source = context
        .semantic
        .typed_hir
        .expression(expression.span())
        .ok_or(super::super::BuildError::MissingTypedExpression)?
        .id;
    let (stored, layer) = if matches!(expression.without_groups(), Expr::Identifier(_)) {
        stored_outcome_source(context, expression)?
    } else {
        let ty = outcome_source_type(context, expression)
            .ok_or(super::super::BuildError::MissingTypedExpression)?;
        let shape = outcome_shape(context, ty)?;
        let local =
            context.aggregate_temporary(ty, crate::mir::LocalOrigin::Temporary(source), scope)?;
        context.lower_value_to_place(
            local,
            expression,
            ty,
            ValueRepresentation::Aggregate,
            scope,
        )?;
        (
            crate::mir::Place::local(local),
            *shape
                .layers
                .first()
                .ok_or(super::super::BuildError::UnsupportedClaimedExpression)?,
        )
    };
    context.control_flow.emit_stored_outcome_inspection(
        crate::mir::Origin::Expression(source),
        stored_outcome_operand(context, stored),
        layer,
        destination,
        failure,
    )
}

fn outcome_source_type(
    context: &LoweringContext<'_>,
    expression: &Expr,
) -> Option<crate::semantic::TyId> {
    match expression.without_groups() {
        Expr::Call(call) => super::super::coverage::call_result_type(call, context.semantic),
        Expr::Identifier(identifier) => context
            .semantic
            .resolved
            .local_symbol_for_identifier(identifier)
            .and_then(|symbol| context.semantic.typed_hir.binding_type_expr(symbol.id))
            .and_then(|ty| context.semantic.typed_hir.type_id(ty)),
        _ => super::super::coverage::intrinsic_expression_type(
            expression.span(),
            context.semantic.typed_hir,
        ),
    }
}

fn stored_outcome_operand(
    context: &LoweringContext<'_>,
    source: crate::mir::Place,
) -> crate::mir::Operand {
    if context.locals[source.local.index()].ownership == crate::mir::OwnershipKind::Move {
        crate::mir::Operand::Move(source)
    } else {
        crate::mir::Operand::Copy(source)
    }
}

pub(super) fn stored_outcome_source(
    context: &LoweringContext<'_>,
    expression: &Expr,
) -> Result<(crate::mir::Place, crate::outcomes::OutcomeLayer), super::super::BuildError> {
    let Expr::Identifier(identifier) = expression.without_groups() else {
        return Err(super::super::BuildError::UnsupportedClaimedExpression);
    };
    let symbol = context
        .semantic
        .resolved
        .local_symbol_for_identifier(identifier)
        .ok_or(super::super::BuildError::MissingLocalSymbol)?;
    let place = *context
        .places_by_symbol
        .get(&symbol.id)
        .ok_or(super::super::BuildError::MissingLocalSymbol)?;
    let type_expr = context
        .semantic
        .typed_hir
        .binding_type_expr(symbol.id)
        .ok_or(super::super::BuildError::MissingTypedExpression)?;
    let shape = crate::outcomes::outcome_shape_with_resolver(
        type_expr,
        context.semantic.resolved,
        |source| context.semantic.resolver_for(source),
    );
    let Some(layer) = shape.layers.first() else {
        return Err(super::super::BuildError::UnsupportedClaimedExpression);
    };
    Ok((place, *layer))
}
