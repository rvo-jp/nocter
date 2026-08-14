//! Protocol collection iteration normalized into a natural MIR loop.
//!
//! Type checking selects the collection conversion and iterator step methods.
//! MIR retains only their semantic call identities, the owned iterator, the
//! optional step result, and the success/failure control-flow edges.

use super::BuildError;
use super::context::LoweringContext;
use super::statements::{LoopTargets, StatementLowerer};
use crate::ast::{CollectionForStmt, Expr, MethodReceiverMode};
use crate::mir::{
    CallArgument, LocalId, LocalOrigin, Operand, Origin, Place, ScopeId, Terminator,
    ValueRepresentation,
};

pub(super) fn lower(
    context: &mut LoweringContext<'_>,
    statement: &CollectionForStmt,
    parent_scope: ScopeId,
) -> Result<(), BuildError> {
    let plan = context
        .semantic
        .typed_hir
        .collection_for_plan(statement.span)
        .cloned()
        .ok_or(BuildError::MissingCallTarget)?;
    let iterator_ty = context
        .semantic
        .typed_hir
        .type_id(&plan.iterator_type)
        .ok_or(BuildError::MissingTypedExpression)?;
    let item_ty = context
        .semantic
        .typed_hir
        .type_id(&plan.item_type)
        .ok_or(BuildError::MissingTypedExpression)?;
    let optional_type = crate::ast::TypeExpr::Optional(crate::ast::OptionalType {
        span: statement.span,
        inner: Box::new(plan.item_type.clone()),
    });
    let optional_ty = context
        .semantic
        .typed_hir
        .type_id(&optional_type)
        .ok_or(BuildError::MissingTypedExpression)?;
    let source_origin = context
        .semantic
        .typed_hir
        .expression(statement.source.span())
        .ok_or(BuildError::MissingTypedExpression)?
        .id;

    let loop_scope = context.child_scope(parent_scope, statement.span);
    let iterator = context.aggregate_temporary(
        iterator_ty,
        LocalOrigin::Desugared(statement.source.span()),
        loop_scope,
    )?;
    materialize_iterator(
        context,
        iterator,
        iterator_ty,
        &plan,
        statement,
        loop_scope,
        source_origin,
    )?;

    let iteration_scope = context.child_scope(loop_scope, statement.body.span);
    let item_symbol = context
        .semantic
        .resolved
        .local_symbol_id_at_name_span(statement.name_span)
        .ok_or(BuildError::MissingLocalSymbol)?;
    let item =
        context.local_for_type(item_ty, LocalOrigin::Binding(item_symbol), iteration_scope)?;
    context
        .places_by_symbol
        .insert(item_symbol, Place::local(item));
    let outcome = context.aggregate_temporary(
        optional_ty,
        LocalOrigin::Desugared(statement.span),
        loop_scope,
    )?;

    let step_scope = context.child_scope(loop_scope, statement.name_span);
    let header = context.control_flow.reserve_block(step_scope);
    let body = context.control_flow.reserve_block(iteration_scope);
    let exit = context.control_flow.reserve_block(parent_scope);
    context
        .control_flow
        .terminate(Terminator::Goto { target: header })?;
    context.control_flow.select_block(header)?;

    let step_receiver = protocol_local_receiver(
        context,
        iterator,
        &plan.step,
        step_scope,
        Origin::Desugared(statement.name_span),
    )?;
    let step_instance = protocol_instance(context, &plan.step)?;
    context.control_flow.emit_returning_call(
        source_origin,
        step_instance,
        vec![step_receiver],
        outcome,
    )?;
    let condition = context.control_flow.current_block()?;
    let outcome_operand =
        if context.locals[outcome.index()].ownership == crate::mir::OwnershipKind::Move {
            Operand::Move(Place::local(outcome))
        } else {
            Operand::Copy(Place::local(outcome))
        };
    context.control_flow.terminate(Terminator::InspectOutcome {
        origin: Origin::Desugared(statement.name_span),
        source: outcome_operand,
        layer: crate::outcomes::OutcomeLayer::Optional,
        destination: Place::local(item),
        success: body,
        failure: exit,
        failure_payload: None,
    })?;

    context.control_flow.select_block(body)?;
    let body_statements = super::coverage::scalar_loop_block_statements(
        &statement.body,
        context.semantic.resolved,
        context.semantic.resolved_sources,
        context.semantic.typed_hir,
    )
    .ok_or(BuildError::UnsupportedClaimedExpression)?;
    let exits = StatementLowerer::new(context).lower_in_context(
        &body_statements,
        Some(LoopTargets {
            break_target: exit,
            continue_target: header,
        }),
        iteration_scope,
    )?;
    if !exits {
        context
            .control_flow
            .terminate(Terminator::Goto { target: header })?;
    }
    context.loop_regions.push(crate::mir::LoopRegion {
        header,
        condition,
        body,
        continue_target: header,
        exit,
    });
    context.control_flow.select_block(exit)
}

#[allow(clippy::too_many_arguments)]
fn materialize_iterator(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    iterator_ty: crate::semantic::TyId,
    plan: &crate::typecheck::TypecheckCollectionForPlan,
    statement: &CollectionForStmt,
    scope: ScopeId,
    origin: crate::semantic::ExprId,
) -> Result<(), BuildError> {
    match plan.source_mode {
        crate::typecheck::TypecheckCollectionForSourceMode::Direct => {
            if let Expr::Identifier(identifier) = statement.source.without_groups() {
                let symbol = context
                    .semantic
                    .resolved
                    .local_symbol_for_identifier(identifier)
                    .ok_or(BuildError::MissingLocalSymbol)?;
                let source = *context
                    .places_by_symbol
                    .get(&symbol.id)
                    .ok_or(BuildError::MissingLocalSymbol)?;
                let operand = if context.locals[source.local.index()].ownership
                    == crate::mir::OwnershipKind::Move
                {
                    Operand::Move(source)
                } else {
                    Operand::Copy(source)
                };
                context
                    .control_flow
                    .push_statement(crate::mir::Statement::Assign {
                        destination: Place::local(destination),
                        value: crate::mir::Rvalue::Use(operand),
                        origin: Origin::Expression(origin),
                    })
            } else {
                context.lower_value_to_place(
                    destination,
                    &statement.source,
                    iterator_ty,
                    ValueRepresentation::Aggregate,
                    scope,
                )
            }
        }
        crate::typecheck::TypecheckCollectionForSourceMode::ReadonlyConversion
        | crate::typecheck::TypecheckCollectionForSourceMode::ReadwriteConversion
        | crate::typecheck::TypecheckCollectionForSourceMode::OwnedConversion => {
            let conversion = plan
                .conversion
                .as_ref()
                .ok_or(BuildError::MissingCallTarget)?;
            let receiver = protocol_expression_receiver(
                context,
                conversion,
                &statement.source,
                scope,
                Origin::Expression(origin),
            )?;
            let instance = protocol_instance(context, conversion)?;
            context
                .control_flow
                .emit_returning_call(origin, instance, vec![receiver], destination)
        }
    }
}

fn protocol_expression_receiver(
    context: &mut LoweringContext<'_>,
    method: &crate::typecheck::TypecheckProtocolMethod,
    expression: &Expr,
    scope: ScopeId,
    origin: Origin,
) -> Result<CallArgument, BuildError> {
    if method.receiver_mode == MethodReceiverMode::Owned {
        context.lower_call_argument(expression, scope)
    } else {
        context.lower_protocol_receiver(method, expression, scope, origin)
    }
}

fn protocol_local_receiver(
    context: &mut LoweringContext<'_>,
    local: LocalId,
    method: &crate::typecheck::TypecheckProtocolMethod,
    scope: ScopeId,
    origin: Origin,
) -> Result<CallArgument, BuildError> {
    match method.receiver_mode {
        MethodReceiverMode::Owned => {
            let ty = context
                .semantic
                .typed_hir
                .type_id(&method.self_ty)
                .ok_or(BuildError::MissingSpecializedReceiverType)?;
            Ok(CallArgument {
                operand: Operand::Move(Place::local(local)),
                ty,
                representation: ValueRepresentation::Aggregate,
            })
        }
        MethodReceiverMode::ReadonlyBorrow | MethodReceiverMode::ReadwriteBorrow => {
            super::borrows::place_argument(
                context,
                Place::local(local),
                &method.self_ty,
                method.receiver_mode == MethodReceiverMode::ReadwriteBorrow,
                scope,
                origin,
            )
        }
    }
}

fn protocol_instance(
    context: &LoweringContext<'_>,
    method: &crate::typecheck::TypecheckProtocolMethod,
) -> Result<crate::mir::CallInstance, BuildError> {
    let self_ty = context
        .semantic
        .typed_hir
        .type_id(&method.self_ty)
        .ok_or(BuildError::MissingSpecializedReceiverType)?;
    Ok(crate::mir::CallInstance::specialized(
        context
            .semantic
            .resolved
            .callable_bodies
            .canonical_definition(method.def_id),
        Some(self_ty),
        Vec::new(),
    ))
}
