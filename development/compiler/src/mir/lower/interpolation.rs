//! Owned string interpolation normalized into constructor and formatter calls.
//!
//! The retained typecheck plan supplies every callable definition and concrete
//! receiver type. MIR owns the partially constructed String and the temporary
//! receiver/output loans, so ordinary failure cleanup drops partial results.

use super::BuildError;
use super::context::LoweringContext;
use super::coverage::value_representation;
use crate::ast::{Expr, InterpolatedStringExpr, InterpolatedStringPart, MethodReceiverMode};
use crate::mir::{
    CallArgument, LocalId, LocalOrigin, LocalStorage, Operand, Place, Rvalue, ScopeId, Statement,
};

pub(super) fn lower_to_place(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    interpolated: &InterpolatedStringExpr,
    result_ty: crate::semantic::TyId,
    scope: ScopeId,
) -> Result<(), BuildError> {
    let plan = context
        .semantic
        .typed_hir
        .interpolation_plan(interpolated.span)
        .cloned()
        .ok_or(BuildError::MissingCallTarget)?;
    if plan.parts.len() != interpolated.parts.len() {
        return Err(BuildError::UnsupportedClaimedExpression);
    }
    let origin = context
        .semantic
        .typed_hir
        .expression(interpolated.span)
        .ok_or(BuildError::MissingTypedExpression)?
        .id;
    // Return ABI storage is a transfer destination, not an addressable local.
    // Interpolation needs a stable address while formatter calls mutate the
    // partially constructed String, so build it in owned local storage and
    // transfer the finished value exactly once.
    let construction = if context.locals[destination.index()].storage == LocalStorage::Return {
        context.aggregate_temporary(result_ty, LocalOrigin::Temporary(origin), scope)?
    } else {
        destination
    };
    let usize_ty = context
        .semantic
        .typed_hir
        .type_id(&crate::ast::TypeExpr::Reference(
            crate::ast::TypeReference {
                span: interpolated.span,
                name: "usize".to_string(),
            },
        ))
        .ok_or(BuildError::MissingTypedExpression)?;
    context.control_flow.emit_returning_call(
        origin,
        crate::mir::CallInstance::direct(
            context
                .semantic
                .resolved
                .callable_bodies
                .canonical_definition(plan.constructor.definition),
        ),
        vec![CallArgument {
            operand: Operand::Constant(crate::mir::Constant {
                ty: usize_ty,
                scalar: crate::mir::ScalarType::Usize,
                value: 0,
            }),
            ty: usize_ty,
            representation: crate::mir::ValueRepresentation::Scalar(crate::mir::ScalarType::Usize),
        }],
        construction,
    )?;

    for (part, planned) in interpolated.parts.iter().zip(&plan.parts) {
        lower_part(
            context,
            construction,
            result_ty,
            part,
            planned,
            scope,
            origin,
        )?;
    }
    if construction != destination {
        context.control_flow.push_statement(Statement::Assign {
            destination: Place::local(destination),
            value: Rvalue::Use(Operand::Move(Place::local(construction))),
            origin: crate::mir::Origin::Expression(origin),
        })?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn lower_part(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    result_ty: crate::semantic::TyId,
    part: &InterpolatedStringPart,
    planned: &crate::typecheck::TypecheckInterpolationPart,
    parent_scope: ScopeId,
    origin: crate::semantic::ExprId,
) -> Result<(), BuildError> {
    let part_scope = context.child_scope(parent_scope, planned.span);
    let part_entry = context.control_flow.reserve_block(part_scope);
    let part_exit = context.control_flow.reserve_block(parent_scope);
    context
        .control_flow
        .terminate(crate::mir::Terminator::Goto { target: part_entry })?;
    context.control_flow.select_block(part_entry)?;

    let receiver = match part {
        InterpolatedStringPart::Text(text) => {
            let receiver_ty = context
                .semantic
                .typed_hir
                .type_id(&planned.accepted_type)
                .ok_or(BuildError::MissingMethodReceiverType)?;
            CallArgument {
                operand: Operand::StaticStr {
                    ty: receiver_ty,
                    bytes: text.value.as_bytes().to_vec(),
                },
                ty: receiver_ty,
                representation: crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str),
            }
        }
        InterpolatedStringPart::Expression(part) => lower_receiver(
            context,
            &planned.formatter,
            &part.expression,
            part_scope,
            origin,
        )?,
    };
    let output = borrow_output(context, destination, result_ty, part_scope, planned.span)?;
    let receiver_ty = context
        .semantic
        .typed_hir
        .type_id(&planned.formatter.self_ty)
        .ok_or(BuildError::MissingSpecializedReceiverType)?;
    context.control_flow.emit_effect_call(
        origin,
        crate::mir::CallInstance::specialized(
            context
                .semantic
                .resolved
                .callable_bodies
                .canonical_definition(planned.formatter.def_id),
            Some(receiver_ty),
            Vec::new(),
        ),
        vec![receiver, output],
    )?;
    context
        .control_flow
        .terminate(crate::mir::Terminator::Goto { target: part_exit })?;
    context.control_flow.select_block(part_exit)
}

fn lower_receiver(
    context: &mut LoweringContext<'_>,
    method: &crate::typecheck::TypecheckProtocolMethod,
    expression: &Expr,
    scope: ScopeId,
    origin: crate::semantic::ExprId,
) -> Result<CallArgument, BuildError> {
    let self_ty = context
        .semantic
        .typed_hir
        .type_id(&method.self_ty)
        .ok_or(BuildError::MissingSpecializedReceiverType)?;
    if matches!(
        value_representation(self_ty, context.semantic),
        Some(crate::mir::ValueRepresentation::View(_))
    ) || method.receiver_mode == MethodReceiverMode::Owned
    {
        return context.lower_call_argument(expression, scope);
    }
    if super::borrows::source_place_is_supported(expression, context.semantic) {
        return context.lower_protocol_receiver(
            method,
            expression,
            scope,
            crate::mir::Origin::Expression(origin),
        );
    }

    let representation = value_representation(self_ty, context.semantic)
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
    let temporary = match representation {
        crate::mir::ValueRepresentation::Scalar(scalar) => {
            let local = LocalId::from_index(context.locals.len());
            context.locals.push(crate::mir::Local::scalar(
                self_ty,
                scalar,
                LocalStorage::Local,
                LocalOrigin::Temporary(origin),
                scope,
            ));
            context.lower_expression_to_place(local, expression, self_ty, scalar, scope)?;
            local
        }
        crate::mir::ValueRepresentation::Aggregate => {
            let local =
                context.aggregate_temporary(self_ty, LocalOrigin::Temporary(origin), scope)?;
            context.lower_value_to_place(local, expression, self_ty, representation, scope)?;
            local
        }
        crate::mir::ValueRepresentation::View(kind) => {
            let local = LocalId::from_index(context.locals.len());
            context.locals.push(crate::mir::Local::view(
                self_ty,
                kind,
                LocalStorage::Local,
                LocalOrigin::Temporary(origin),
                scope,
            ));
            context.lower_view_expression_to_place(local, expression, self_ty, kind, scope)?;
            local
        }
        crate::mir::ValueRepresentation::Borrow | crate::mir::ValueRepresentation::Error => {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
    };
    borrow_place(
        context,
        Place::local(temporary),
        &method.self_ty,
        false,
        scope,
        planned_origin(origin),
    )
}

fn borrow_output(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    result_ty: crate::semantic::TyId,
    scope: ScopeId,
    span: crate::source::ByteSpan,
) -> Result<CallArgument, BuildError> {
    let result_type = context
        .semantic
        .typed_hir
        .type_expr_by_id(result_ty)
        .ok_or(BuildError::MissingTypedExpression)?;
    borrow_place(
        context,
        Place::local(destination),
        result_type,
        true,
        scope,
        crate::mir::Origin::Desugared(span),
    )
}

fn borrow_place(
    context: &mut LoweringContext<'_>,
    source: Place,
    inner: &crate::ast::TypeExpr,
    readwrite: bool,
    scope: ScopeId,
    origin: crate::mir::Origin,
) -> Result<CallArgument, BuildError> {
    let borrow_type = crate::ast::TypeExpr::Borrow(crate::ast::BorrowType {
        span: context.scopes[scope.index()].span,
        is_readwrite: readwrite,
        inner: Box::new(inner.clone()),
    });
    let ty = context
        .semantic
        .typed_hir
        .type_id(&borrow_type)
        .ok_or(BuildError::MissingMethodReceiverType)?;
    let local = LocalId::from_index(context.locals.len());
    context.locals.push(crate::mir::Local::borrow(
        ty,
        readwrite,
        LocalStorage::Local,
        LocalOrigin::Desugared(context.scopes[scope.index()].span),
        scope,
    ));
    super::borrows::lower_place_to_local(context, local, source, readwrite, scope, origin)?;
    Ok(CallArgument {
        operand: if readwrite {
            Operand::Move(Place::local(local))
        } else {
            Operand::Copy(Place::local(local))
        },
        ty,
        representation: crate::mir::ValueRepresentation::Borrow,
    })
}

fn planned_origin(origin: crate::semantic::ExprId) -> crate::mir::Origin {
    crate::mir::Origin::Expression(origin)
}
