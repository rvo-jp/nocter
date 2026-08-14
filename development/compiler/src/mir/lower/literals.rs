//! Typed literal expressions normalized into semantic hidden-call edges.
//!
//! The MIR identity retains the literal declaration, result type, shape, and
//! pack segment structure. Runtime specialization names remain a backend
//! projection of that identity.

use super::BuildError;
use super::context::LoweringContext;
use crate::ast::{Expr, LiteralShape};
use crate::mir::{CallArgument, LiteralSegment, Operand, ScopeId, ValueRepresentation};

pub(super) fn lower_to_place(
    context: &mut LoweringContext<'_>,
    destination: crate::mir::LocalId,
    expression: &Expr,
    result_ty: crate::semantic::TyId,
    scope: ScopeId,
) -> Result<(), BuildError> {
    let (span, shape, allocator) = match expression.without_groups() {
        Expr::TypedSequenceLiteral(literal) => (
            literal.span,
            LiteralShape::Sequence,
            literal.using.as_ref().map(|using| using.allocator.as_ref()),
        ),
        Expr::TypedStringLiteral(literal) => (
            literal.span,
            LiteralShape::String,
            literal.using.as_ref().map(|using| using.allocator.as_ref()),
        ),
        _ => return Err(BuildError::UnsupportedClaimedExpression),
    };
    let override_exit = allocator
        .map(|allocator| enter_allocation_override(context, allocator, scope, span))
        .transpose()?;
    let evaluation_scope = override_exit.map_or(scope, |(scope, _)| scope);
    let (arguments, segments) = match expression.without_groups() {
        Expr::TypedSequenceLiteral(literal) => {
            let mut arguments = Vec::with_capacity(literal.elements.len());
            let mut segments = Vec::with_capacity(literal.elements.len());
            for element in &literal.elements {
                if let Some(spread) = crate::typecheck::sequence_spread(element) {
                    let plan = context
                        .semantic
                        .typed_hir
                        .sequence_spread_plan(spread.span)
                        .cloned()
                        .ok_or(BuildError::MissingCallTarget)?;
                    let iterator_ty = context
                        .semantic
                        .typed_hir
                        .type_id(&plan.iterator_type)
                        .ok_or(BuildError::MissingTypedExpression)?;
                    let argument = match plan.source_mode {
                        crate::typecheck::TypecheckCollectionForSourceMode::Direct => {
                            context.lower_call_argument(&spread.operand, evaluation_scope)?
                        }
                        crate::typecheck::TypecheckCollectionForSourceMode::ReadonlyConversion
                        | crate::typecheck::TypecheckCollectionForSourceMode::ReadwriteConversion
                        | crate::typecheck::TypecheckCollectionForSourceMode::OwnedConversion => {
                            lower_spread_conversion(
                                context,
                                spread,
                                &plan,
                                iterator_ty,
                                evaluation_scope,
                            )?
                        }
                    };
                    if argument.ty != iterator_ty
                        || argument.representation != ValueRepresentation::Aggregate
                    {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    }
                    arguments.push(argument);
                    segments.push(LiteralSegment::Spread {
                        mode: plan.mode,
                        iterator: iterator_ty,
                    });
                    continue;
                }
                arguments.push(context.lower_call_argument(element, evaluation_scope)?);
                segments.push(LiteralSegment::Value);
            }
            (arguments, segments)
        }
        Expr::TypedStringLiteral(literal) => {
            let str_ty = readonly_str_type_id(literal.target.span(), context)?;
            let bytes = crate::literals::decode_string_literal_bytes(&literal.text.value)
                .map_err(|_| BuildError::InvalidScalarConstant)?;
            (
                vec![CallArgument {
                    operand: Operand::StaticStr { ty: str_ty, bytes },
                    ty: str_ty,
                    representation: ValueRepresentation::View(crate::mir::ViewKind::Str),
                }],
                Vec::new(),
            )
        }
        _ => return Err(BuildError::UnsupportedClaimedExpression),
    };
    let resolution = context
        .semantic
        .resolved
        .literal_resolution(span)
        .ok_or(BuildError::MissingCallTarget)?;
    let definition = context
        .semantic
        .resolved
        .callable_bodies
        .canonical_definition(resolution.literal_definition);
    let origin = context
        .semantic
        .typed_hir
        .expression(span)
        .ok_or(BuildError::MissingTypedExpression)?
        .id;
    context.control_flow.emit_returning_call(
        origin,
        crate::mir::CallInstance::literal(definition, shape, result_ty, segments),
        arguments,
        destination,
    )?;
    if let Some((_, exit)) = override_exit {
        context
            .control_flow
            .terminate(crate::mir::Terminator::Goto { target: exit })?;
        context.control_flow.select_block(exit)?;
    }
    Ok(())
}

fn enter_allocation_override(
    context: &mut LoweringContext<'_>,
    allocator: &Expr,
    parent_scope: ScopeId,
    span: crate::source::ByteSpan,
) -> Result<(ScopeId, crate::mir::BasicBlockId), BuildError> {
    let allocator = established_aggregate_place(context, allocator)?;
    let scope = context.child_scope(parent_scope, span);
    let body = context.control_flow.reserve_block(scope);
    let exit = context.control_flow.reserve_block(parent_scope);
    context
        .control_flow
        .terminate(crate::mir::Terminator::Goto { target: body })?;
    context.control_flow.select_block(body)?;
    let usize_ty = context
        .semantic
        .typed_hir
        .type_id(&crate::ast::TypeExpr::Reference(
            crate::ast::TypeReference {
                span,
                name: "usize".to_string(),
            },
        ))
        .ok_or(BuildError::MissingTypedExpression)?;
    let mut hidden = || {
        let local = crate::mir::LocalId::from_index(context.locals.len());
        context.locals.push(crate::mir::Local::scalar(
            usize_ty,
            crate::mir::ScalarType::Usize,
            crate::mir::LocalStorage::Local,
            crate::mir::LocalOrigin::Desugared(span),
            scope,
        ));
        local
    };
    let parent_state = hidden();
    let parent_kind = hidden();
    let selected_state = hidden();
    let selected_kind = hidden();
    let id = crate::mir::AllocationOverrideId::from_index(context.allocation_overrides.len());
    context
        .allocation_overrides
        .push(crate::mir::AllocationContextOverride {
            id,
            scope,
            allocator,
            parent_state,
            parent_kind,
            selected_state,
            selected_kind,
        });
    context
        .control_flow
        .push_statement(crate::mir::Statement::EnterAllocationContext {
            override_: id,
            origin: crate::mir::Origin::Desugared(span),
        })?;
    Ok((scope, exit))
}

fn established_aggregate_place(
    context: &mut LoweringContext<'_>,
    expression: &Expr,
) -> Result<crate::mir::Place, BuildError> {
    match expression.without_groups() {
        Expr::Identifier(identifier) => {
            let symbol = context
                .semantic
                .resolved
                .local_symbol_for_identifier(identifier)
                .ok_or(BuildError::MissingLocalSymbol)?;
            context
                .places_by_symbol
                .get(&symbol.id)
                .copied()
                .ok_or(BuildError::MissingLocalSymbol)
        }
        Expr::Member(member) => {
            let (place, representation) = super::projections::lower_field_place(
                member,
                context.semantic,
                &context.places_by_symbol,
                &mut context.projections,
                &mut context.drop_plans,
            )?;
            (representation == ValueRepresentation::Aggregate)
                .then_some(place)
                .ok_or(BuildError::UnsupportedClaimedExpression)
        }
        _ => Err(BuildError::UnsupportedClaimedExpression),
    }
}

fn lower_spread_conversion(
    context: &mut LoweringContext<'_>,
    spread: &crate::ast::UnaryExpr,
    plan: &crate::typecheck::TypecheckSequenceSpreadPlan,
    iterator_ty: crate::semantic::TyId,
    scope: ScopeId,
) -> Result<CallArgument, BuildError> {
    let method = plan
        .conversion
        .as_ref()
        .ok_or(BuildError::MissingCallTarget)?;
    let source_expression = match plan.mode {
        crate::typecheck::TypecheckSequenceSpreadMode::Copy => spread.operand.as_ref(),
        crate::typecheck::TypecheckSequenceSpreadMode::Readonly
        | crate::typecheck::TypecheckSequenceSpreadMode::Move => spread.operand.as_ref(),
    };
    let origin = context
        .semantic
        .typed_hir
        .expression(spread.span)
        .ok_or(BuildError::MissingTypedExpression)?
        .id;
    let receiver = context.lower_protocol_receiver(
        method,
        source_expression,
        scope,
        crate::mir::Origin::Expression(origin),
    )?;
    let destination = context.aggregate_temporary(
        iterator_ty,
        crate::mir::LocalOrigin::Temporary(origin),
        scope,
    )?;
    let receiver_ty = context
        .semantic
        .typed_hir
        .type_id(&method.self_ty)
        .ok_or(BuildError::MissingSpecializedReceiverType)?;
    let definition = context
        .semantic
        .resolved
        .callable_bodies
        .canonical_definition(method.def_id);
    context.control_flow.emit_returning_call(
        origin,
        crate::mir::CallInstance::specialized(definition, Some(receiver_ty), Vec::new()),
        vec![receiver],
        destination,
    )?;
    Ok(CallArgument {
        operand: Operand::Move(crate::mir::Place::local(destination)),
        ty: iterator_ty,
        representation: ValueRepresentation::Aggregate,
    })
}

fn readonly_str_type_id(
    span: crate::source::ByteSpan,
    context: &LoweringContext<'_>,
) -> Result<crate::semantic::TyId, BuildError> {
    let ty = crate::ast::TypeExpr::Borrow(crate::ast::BorrowType {
        span,
        is_readwrite: false,
        inner: Box::new(crate::ast::TypeExpr::Reference(crate::ast::TypeReference {
            span,
            name: "str".to_string(),
        })),
    });
    context
        .semantic
        .typed_hir
        .type_id(&ty)
        .ok_or(BuildError::MissingTypedExpression)
}
