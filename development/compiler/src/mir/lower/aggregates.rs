//! Aggregate literal classification and construction.
//!
//! MIR retains semantic field/index paths, not ABI offsets. The backend is the
//! only layer that projects those paths onto a concrete layout.

use super::context::LoweringContext;
use super::source_model::{known_expression_type, scalar_expression_is_supported, scalar_type};
use super::{BuildError, SemanticInputs};
use crate::abi::AbiType;
use crate::ast::Expr;
use crate::mir::{
    AggregateElement, AggregateLeaf, LocalId, Origin, Place, Rvalue, ScopeId, Statement,
};

pub(super) fn literal_is_supported(expression: &Expr, semantic: SemanticInputs<'_>) -> bool {
    let Some(abi) = aggregate_abi_type(expression, semantic) else {
        return false;
    };
    closure_matches_abi(expression.without_groups(), &abi, semantic)
        || literal_matches_abi(expression.without_groups(), &abi, semantic)
        || variant_matches_abi(expression.without_groups(), &abi, semantic)
}

fn closure_matches_abi(expression: &Expr, abi: &AbiType, semantic: SemanticInputs<'_>) -> bool {
    let Expr::Closure(closure) = expression else {
        return false;
    };
    let Some(plan) = semantic.typed_hir.closure_plan(closure.span) else {
        return false;
    };
    let AbiType::Struct(fields) = abi else {
        return false;
    };
    closure.captures.len() == plan.ty.captures.len()
        && closure.captures.len() == fields.len()
        && closure
            .captures
            .iter()
            .zip(&plan.ty.captures)
            .zip(fields)
            .all(|((capture, capture_ty), field)| {
                capture.name == capture_ty.name
                    && capture.name == field.name
                    && semantic
                        .resolved
                        .local_symbol_id_for_reference_span(capture.name_span)
                        .is_some()
                    && semantic.typed_hir.type_id(&capture_ty.ty).is_some()
            })
}

fn literal_matches_abi(expression: &Expr, abi: &AbiType, semantic: SemanticInputs<'_>) -> bool {
    match (expression, abi) {
        (Expr::StructLiteral(literal), AbiType::Struct(fields)) => {
            literal.fields.iter().all(|field| {
                let Some(abi_field) = fields.iter().find(|candidate| candidate.name == field.name)
                else {
                    return false;
                };
                value_matches_abi(&field.value, &abi_field.ty, semantic)
            })
        }
        (Expr::ArrayLiteral(literal), AbiType::Array { element, length }) => {
            usize::try_from(*length).ok() == Some(literal.elements.len())
                && literal
                    .elements
                    .iter()
                    .all(|value| value_matches_abi(value, element, semantic))
        }
        _ => false,
    }
}

fn variant_matches_abi(expression: &Expr, abi: &AbiType, semantic: SemanticInputs<'_>) -> bool {
    let AbiType::Enum(enum_) = abi else {
        return false;
    };
    let Some((member, arguments)) = variant_member_and_arguments(expression) else {
        return false;
    };
    if semantic
        .typed_hir
        .enum_variant_target(member.member_span)
        .is_none()
    {
        return false;
    }
    let Some(variant) = enum_
        .variants
        .iter()
        .find(|variant| variant.name == member.member)
    else {
        return false;
    };
    variant_payload_values(arguments, variant.payload.as_ref()).is_some_and(|values| {
        values
            .into_iter()
            .all(|(value, abi)| value_matches_abi(value, abi, semantic))
    })
}

fn variant_member_and_arguments(expression: &Expr) -> Option<(&crate::ast::MemberExpr, &[Expr])> {
    match expression.without_groups() {
        Expr::Call(call) => match call.callee.without_groups() {
            Expr::Member(member) => Some((member, call.arguments.as_slice())),
            _ => None,
        },
        Expr::Member(member) => Some((member, &[])),
        _ => None,
    }
}

fn variant_payload_values<'a>(
    arguments: &'a [Expr],
    payload: Option<&'a AbiType>,
) -> Option<Vec<(&'a Expr, &'a AbiType)>> {
    match (arguments, payload) {
        ([], None) => Some(Vec::new()),
        ([argument], Some(payload)) => Some(vec![(argument, payload)]),
        (arguments, Some(AbiType::Struct(fields))) if arguments.len() == fields.len() => Some(
            arguments
                .iter()
                .zip(fields.iter())
                .map(|(argument, field)| (argument, &field.ty))
                .collect(),
        ),
        _ => None,
    }
}

fn value_matches_abi(expression: &Expr, abi: &AbiType, semantic: SemanticInputs<'_>) -> bool {
    if let Some(ty) = known_expression_type(expression, semantic.typed_hir)
        && scalar_type(ty, semantic.typed_hir).is_some()
    {
        return scalar_expression_is_supported(
            expression,
            semantic.resolved,
            semantic.resolved_sources,
            semantic.typed_hir,
        );
    }
    let view_representation = match abi {
        AbiType::StrView => Some(crate::mir::ValueRepresentation::View(
            crate::mir::ViewKind::Str,
        )),
        AbiType::SliceView => Some(crate::mir::ValueRepresentation::View(
            crate::mir::ViewKind::Slice,
        )),
        AbiType::Borrow => Some(crate::mir::ValueRepresentation::Borrow),
        _ => None,
    };
    if let Some(representation) = view_representation {
        return super::source_model::value_expression_is_supported(
            expression,
            representation,
            semantic,
        );
    }
    if matches!(
        abi,
        AbiType::Struct(_) | AbiType::Array { .. } | AbiType::Enum(_) | AbiType::Outcome { .. }
    ) && (super::source_model::aggregate_operand_is_supported(
        expression,
        semantic.resolved,
        semantic.resolved_sources,
        semantic.typed_hir,
    ) || matches!(expression.without_groups(), Expr::Call(_))
        && super::source_model::value_expression_is_supported(
            expression,
            crate::mir::ValueRepresentation::Aggregate,
            semantic,
        )
        || matches!(expression.without_groups(), Expr::Member(member)
            if super::projections::aggregate_value_field_is_supported(member, semantic)))
    {
        return true;
    }
    literal_matches_abi(expression.without_groups(), abi, semantic)
}

pub(super) fn lower_literal(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    expression: &Expr,
    scope: ScopeId,
) -> Result<(), BuildError> {
    let semantic = context.semantic;
    let destination_ty = context
        .locals
        .get(destination.index())
        .ok_or(BuildError::MissingTypedExpression)?
        .ty;
    let destination_ty = semantic
        .typed_hir
        .type_expr_by_id(destination_ty)
        .ok_or(BuildError::MissingTypedExpression)?;
    let abi = crate::abi::abi_value_from_type_expr_with_resolver(
        destination_ty,
        semantic.resolved,
        |source| semantic.resolver_for(source),
    )
    .map_err(|_| BuildError::UnsupportedClaimedExpression.context("resolve aggregate ABI"))?
    .ty;
    if let Expr::Closure(closure) = expression.without_groups() {
        return lower_closure(context, destination, closure, &abi, scope);
    }
    let mut leaves = Vec::new();
    let value = if let AbiType::Enum(enum_) = &abi {
        let (member, arguments) = variant_member_and_arguments(expression)
            .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let variant = context
            .semantic
            .typed_hir
            .enum_variant_target(member.member_span)
            .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let abi_variant = enum_
            .variants
            .iter()
            .find(|variant| variant.name == member.member)
            .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let values = variant_payload_values(arguments, abi_variant.payload.as_ref())
            .ok_or(BuildError::UnsupportedClaimedExpression)?;
        for (index, (argument, payload_abi)) in values.into_iter().enumerate() {
            let mut path = vec![AggregateElement::VariantPayload(index)];
            lower_literal_leaves(
                context,
                argument,
                payload_abi,
                scope,
                &mut path,
                &mut leaves,
            )?;
        }
        Rvalue::Variant { variant, leaves }
    } else {
        let origin = context
            .semantic
            .typed_hir
            .expression(expression.span())
            .map(|expression| Origin::Expression(expression.id))
            .ok_or(BuildError::MissingTypedExpression)?;
        context
            .control_flow
            .push_statement(Statement::BeginAggregate {
                destination: Place::local(destination),
                origin,
            })?;
        lower_staged_aggregate(
            context,
            destination,
            None,
            expression.without_groups(),
            &abi,
            scope,
        )
        .map_err(|error| error.context("lower staged aggregate"))?;
        return Ok(());
    };
    let origin = context
        .semantic
        .typed_hir
        .expression(expression.span())
        .map(|expression| Origin::Expression(expression.id))
        .ok_or(BuildError::MissingTypedExpression)?;
    context.control_flow.push_statement(Statement::Assign {
        destination: Place::local(destination),
        value,
        origin,
    })
}

fn lower_closure(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    closure: &crate::ast::ClosureExpr,
    abi: &AbiType,
    scope: ScopeId,
) -> Result<(), BuildError> {
    let plan = context
        .semantic
        .typed_hir
        .closure_plan(closure.span)
        .cloned()
        .ok_or(BuildError::MissingTypedExpression)?;
    let AbiType::Struct(fields) = abi else {
        return Err(BuildError::UnsupportedClaimedExpression);
    };
    if closure.captures.len() != plan.ty.captures.len() || closure.captures.len() != fields.len() {
        return Err(BuildError::UnsupportedClaimedExpression);
    }
    let expression = context
        .semantic
        .typed_hir
        .expression(closure.span)
        .ok_or(BuildError::MissingTypedExpression)?;
    let origin = Origin::Expression(expression.id);
    context
        .control_flow
        .push_statement(Statement::BeginAggregate {
            destination: Place::local(destination),
            origin,
        })?;
    let layout =
        crate::abi::layout_struct(fields).map_err(|_| BuildError::UnsupportedClaimedExpression)?;
    let mut children = Vec::with_capacity(closure.captures.len());
    for (index, (capture, capture_ty)) in closure.captures.iter().zip(&plan.ty.captures).enumerate()
    {
        if capture.name != capture_ty.name || fields[index].name != capture.name {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        let offset = layout
            .fields
            .get(index)
            .and_then(|field| u32::try_from(field.offset).ok())
            .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let child = push_projection_for_type(
            context,
            destination,
            None,
            crate::mir::ProjectionElement::Field { offset },
            &capture_ty.ty,
        )?;
        let source_symbol = context
            .semantic
            .resolved
            .local_symbol_id_for_reference_span(capture.name_span)
            .ok_or(BuildError::MissingLocalSymbol)?;
        let source = *context
            .places_by_symbol
            .get(&source_symbol)
            .ok_or(BuildError::MissingLocalSymbol)?;
        let value = match capture.mode {
            crate::ast::ClosureCaptureMode::ReadonlyBorrow
            | crate::ast::ClosureCaptureMode::ReadwriteBorrow => {
                let readwrite = capture.mode == crate::ast::ClosureCaptureMode::ReadwriteBorrow;
                let borrow_ty = context
                    .semantic
                    .typed_hir
                    .type_id(&capture_ty.ty)
                    .ok_or(BuildError::MissingTypedExpression)?;
                let temporary = LocalId::from_index(context.locals.len());
                context.locals.push(crate::mir::Local::borrow(
                    borrow_ty,
                    readwrite,
                    crate::mir::LocalStorage::Local,
                    crate::mir::LocalOrigin::Temporary(expression.id),
                    scope,
                ));
                super::borrows::lower_symbol_to_local(
                    context,
                    temporary,
                    source_symbol,
                    readwrite,
                    scope,
                    origin,
                )?;
                if readwrite {
                    crate::mir::Operand::Move(Place::local(temporary))
                } else {
                    crate::mir::Operand::Copy(Place::local(temporary))
                }
            }
            crate::ast::ClosureCaptureMode::Move => {
                if super::super::drop_plans::is_copy(
                    &capture_ty.ty,
                    context.semantic.resolved,
                    context.semantic.resolved_sources,
                ) == Some(true)
                {
                    crate::mir::Operand::Copy(source)
                } else {
                    crate::mir::Operand::Move(source)
                }
            }
        };
        context.control_flow.push_statement(Statement::Assign {
            destination: Place::projected(destination, child),
            value: Rvalue::Use(value),
            origin,
        })?;
        children.push(child);
    }
    context
        .control_flow
        .push_statement(Statement::FinishAggregate {
            destination: Place::local(destination),
            fields: children,
            origin,
        })
}

fn lower_staged_aggregate(
    context: &mut LoweringContext<'_>,
    base: LocalId,
    parent: Option<crate::mir::ProjectionPathId>,
    expression: &Expr,
    abi: &AbiType,
    scope: ScopeId,
) -> Result<(), BuildError> {
    let children = match (expression.without_groups(), abi) {
        (Expr::StructLiteral(literal), AbiType::Struct(fields)) => {
            let layout = crate::abi::layout_struct(fields)
                .map_err(|_| BuildError::UnsupportedClaimedExpression)?;
            let mut children = Vec::with_capacity(literal.fields.len());
            for field in &literal.fields {
                let index = fields
                    .iter()
                    .position(|candidate| candidate.name == field.name)
                    .ok_or(BuildError::UnsupportedClaimedExpression)?;
                let offset = layout
                    .fields
                    .get(index)
                    .and_then(|field| u32::try_from(field.offset).ok())
                    .ok_or(BuildError::UnsupportedClaimedExpression)?;
                let element = crate::mir::ProjectionElement::Field { offset };
                let child = if let Some(field_ty) = context
                    .semantic
                    .typed_hir
                    .field_type_expr(field.name_span)
                    .cloned()
                {
                    push_projection_for_type(context, base, parent, element, &field_ty)?
                } else {
                    push_construction_projection(
                        context,
                        base,
                        parent,
                        element,
                        &field.value,
                        &fields[index].ty,
                    )?
                };
                lower_staged_value(context, base, child, &field.value, &fields[index].ty, scope)
                    .map_err(|error| error.context("lower aggregate field"))?;
                children.push(child);
            }
            children
        }
        (Expr::ArrayLiteral(literal), AbiType::Array { element, length }) => {
            if usize::try_from(*length).ok() != Some(literal.elements.len()) {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
            let stride = u32::try_from(
                crate::abi::array_element_stride(element)
                    .map_err(|_| BuildError::UnsupportedClaimedExpression)?,
            )
            .map_err(|_| BuildError::UnsupportedClaimedExpression)?;
            let usize_ty = context
                .semantic
                .typed_hir
                .type_id(&crate::ast::TypeExpr::Reference(
                    crate::ast::TypeReference {
                        span: literal.span,
                        name: "usize".to_string(),
                    },
                ))
                .ok_or(BuildError::MissingTypedExpression)?;
            let mut children = Vec::with_capacity(literal.elements.len());
            for (index, value) in literal.elements.iter().enumerate() {
                let child = push_construction_projection(
                    context,
                    base,
                    parent,
                    crate::mir::ProjectionElement::Index {
                        index: crate::mir::Operand::Constant(crate::mir::Constant {
                            ty: usize_ty,
                            scalar: crate::mir::ScalarType::Usize,
                            value: index as u128,
                        }),
                        length: *length,
                        stride,
                    },
                    value,
                    element,
                )?;
                lower_staged_value(context, base, child, value, element, scope)
                    .map_err(|error| error.context("lower aggregate element"))?;
                children.push(child);
            }
            children
        }
        _ => {
            return Err(
                BuildError::UnsupportedClaimedExpression.context("match aggregate literal to ABI")
            );
        }
    };
    let origin = context
        .semantic
        .typed_hir
        .expression(expression.span())
        .map(|expression| Origin::Expression(expression.id))
        .ok_or(BuildError::MissingTypedExpression)?;
    context
        .control_flow
        .push_statement(Statement::FinishAggregate {
            destination: parent
                .map(|projection| Place::projected(base, projection))
                .unwrap_or_else(|| Place::local(base)),
            fields: children,
            origin,
        })
}

fn lower_staged_value(
    context: &mut LoweringContext<'_>,
    base: LocalId,
    projection: crate::mir::ProjectionPathId,
    expression: &Expr,
    abi: &AbiType,
    scope: ScopeId,
) -> Result<(), BuildError> {
    let contract = &context.projections[projection.index()];
    if let crate::mir::ValueRepresentation::Scalar(scalar) = contract.representation {
        let ty = contract.ty;
        let operand = context.lower_operand(expression, ty, scalar, scope)?;
        let origin = context
            .semantic
            .typed_hir
            .expression(expression.span())
            .map(|expression| Origin::Expression(expression.id))
            .ok_or(BuildError::MissingTypedExpression)?;
        return context.control_flow.push_statement(Statement::Assign {
            destination: Place::projected(base, projection),
            value: Rvalue::Use(operand),
            origin,
        });
    }
    if matches!(
        contract.representation,
        crate::mir::ValueRepresentation::View(_) | crate::mir::ValueRepresentation::Borrow
    ) {
        let ty = contract.ty;
        let origin = context
            .semantic
            .typed_hir
            .expression(expression.span())
            .map_or(
                crate::mir::LocalOrigin::Desugared(expression.span()),
                |expression| crate::mir::LocalOrigin::Temporary(expression.id),
            );
        let temporary = crate::mir::LocalId::from_index(context.locals.len());
        let local = match contract.representation {
            crate::mir::ValueRepresentation::View(kind) => {
                crate::mir::Local::view(ty, kind, crate::mir::LocalStorage::Local, origin, scope)
            }
            crate::mir::ValueRepresentation::Borrow => {
                let readwrite = matches!(
                    contract.ownership,
                    crate::mir::OwnershipKind::Borrowed { readwrite: true }
                );
                crate::mir::Local::borrow(
                    ty,
                    readwrite,
                    crate::mir::LocalStorage::Local,
                    origin,
                    scope,
                )
            }
            _ => unreachable!(),
        };
        context.locals.push(local);
        context.lower_value_to_place(temporary, expression, ty, contract.representation, scope)?;
        return context.control_flow.push_statement(Statement::Assign {
            destination: Place::projected(base, projection),
            value: Rvalue::Use(crate::mir::Operand::Copy(Place::local(temporary))),
            origin: Origin::Desugared(expression.span()),
        });
    }
    if contract.representation == crate::mir::ValueRepresentation::Aggregate
        && !matches!(
            expression.without_groups(),
            Expr::StructLiteral(_) | Expr::ArrayLiteral(_)
        )
    {
        let ty = contract.ty;
        let operand = if matches!(
            expression.without_groups(),
            Expr::Call(_)
                | Expr::Force(_)
                | Expr::Propagate(_)
                | Expr::Otherwise(_)
                | Expr::Catch(_)
                | Expr::TypedSequenceLiteral(_)
                | Expr::TypedStringLiteral(_)
                | Expr::InterpolatedString(_)
        ) || context.type_has_outcome_layers(ty)
            || matches!(expression.without_groups(), Expr::Member(member)
            if context.semantic.typed_hir.enum_variant_target(member.member_span).is_some())
        {
            let origin = context
                .semantic
                .typed_hir
                .expression(expression.span())
                .map_or(
                    crate::mir::LocalOrigin::Desugared(expression.span()),
                    |expression| crate::mir::LocalOrigin::Temporary(expression.id),
                );
            let temporary = context.aggregate_temporary(ty, origin, scope)?;
            context.lower_value_to_place(
                temporary,
                expression,
                ty,
                crate::mir::ValueRepresentation::Aggregate,
                scope,
            )?;
            if context.locals[temporary.index()].ownership == crate::mir::OwnershipKind::Move {
                crate::mir::Operand::Move(Place::local(temporary))
            } else {
                crate::mir::Operand::Copy(Place::local(temporary))
            }
        } else if let Expr::Member(member) = expression.without_groups()
            && super::projections::aggregate_value_field_is_supported(member, context.semantic)
            && !super::source_model::aggregate_operand_is_supported(
                expression,
                context.semantic.resolved,
                context.semantic.resolved_sources,
                context.semantic.typed_hir,
            )
        {
            let source = context.lower_value_member_source(
                member,
                ty,
                crate::mir::ValueRepresentation::Aggregate,
                scope,
            )?;
            crate::mir::Operand::Copy(source)
        } else {
            context.lower_aggregate_operand(expression, scope)?
        };
        let origin = context
            .semantic
            .typed_hir
            .expression(expression.span())
            .map_or(Origin::Desugared(expression.span()), |expression| {
                Origin::Expression(expression.id)
            });
        return context.control_flow.push_statement(Statement::Assign {
            destination: Place::projected(base, projection),
            value: Rvalue::Use(operand),
            origin,
        });
    }
    lower_staged_aggregate(context, base, Some(projection), expression, abi, scope)
}

fn push_construction_projection(
    context: &mut LoweringContext<'_>,
    base: LocalId,
    parent: Option<crate::mir::ProjectionPathId>,
    element: crate::mir::ProjectionElement,
    expression: &Expr,
    abi: &AbiType,
) -> Result<crate::mir::ProjectionPathId, BuildError> {
    let ty =
        super::storage_types::scalar_type_id_from_abi(abi, expression.span(), context.semantic)
            .or_else(|| {
                super::source_model::conversion_plan_for_expression(
                    expression,
                    context.semantic.typed_hir,
                )
                .and_then(|conversion| context.semantic.typed_hir.type_id(&conversion.target_ty))
            })
            .or_else(|| known_expression_type(expression, context.semantic.typed_hir))
            .ok_or(BuildError::MissingTypedExpression)?;
    let type_expr = context
        .semantic
        .typed_hir
        .type_expr_by_id(ty)
        .ok_or(BuildError::MissingTypedExpression)?;
    push_projection(context, base, parent, element, ty, type_expr)
}

fn push_projection_for_type(
    context: &mut LoweringContext<'_>,
    base: LocalId,
    parent: Option<crate::mir::ProjectionPathId>,
    element: crate::mir::ProjectionElement,
    type_expr: &crate::ast::TypeExpr,
) -> Result<crate::mir::ProjectionPathId, BuildError> {
    let ty = context
        .semantic
        .typed_hir
        .type_id(type_expr)
        .ok_or(BuildError::MissingTypedExpression)?;
    push_projection(context, base, parent, element, ty, type_expr)
}

fn push_projection(
    context: &mut LoweringContext<'_>,
    base: LocalId,
    parent: Option<crate::mir::ProjectionPathId>,
    element: crate::mir::ProjectionElement,
    ty: crate::semantic::TyId,
    type_expr: &crate::ast::TypeExpr,
) -> Result<crate::mir::ProjectionPathId, BuildError> {
    let representation = super::source_model::value_representation(ty, context.semantic)
        .unwrap_or(crate::mir::ValueRepresentation::Aggregate);
    let ownership = if super::super::drop_plans::is_copy(
        type_expr,
        context.semantic.resolved,
        context.semantic.resolved_sources,
    ) == Some(true)
    {
        crate::mir::OwnershipKind::Copy
    } else if let crate::ast::TypeExpr::Borrow(borrow) = type_expr {
        crate::mir::OwnershipKind::Borrowed {
            readwrite: borrow.is_readwrite,
        }
    } else {
        crate::mir::OwnershipKind::Move
    };
    let drop_plan = if representation == crate::mir::ValueRepresentation::Aggregate
        && ownership == crate::mir::OwnershipKind::Move
    {
        Some(
            super::super::drop_plans::build(
                type_expr,
                context.semantic.resolved,
                context.semantic.resolved_sources,
                context.semantic.typed_hir,
                &mut context.drop_plans,
            )
            .ok_or(BuildError::UnsupportedClaimedExpression)?,
        )
    } else {
        None
    };
    let id = crate::mir::ProjectionPathId::from_index(context.projections.len());
    context.projections.push(crate::mir::ProjectionPath {
        id,
        base,
        parent,
        element,
        ty,
        representation,
        ownership,
        drop_plan,
    });
    Ok(id)
}

fn lower_literal_leaves(
    context: &mut LoweringContext<'_>,
    expression: &Expr,
    abi: &AbiType,
    scope: ScopeId,
    path: &mut Vec<AggregateElement>,
    leaves: &mut Vec<AggregateLeaf>,
) -> Result<(), BuildError> {
    if let Some(ty) = known_expression_type(expression, context.semantic.typed_hir)
        && let Some(scalar) = scalar_type(ty, context.semantic.typed_hir)
    {
        leaves.push(AggregateLeaf {
            path: path.clone(),
            ty,
            representation: crate::mir::ValueRepresentation::Scalar(scalar),
            operand: context.lower_operand(expression, ty, scalar, scope)?,
        });
        return Ok(());
    }

    let ty = known_expression_type(expression, context.semantic.typed_hir)
        .ok_or(BuildError::MissingTypedExpression)?;
    if let Some(representation) = super::source_model::value_representation(ty, context.semantic)
        && representation != crate::mir::ValueRepresentation::Aggregate
    {
        let mut argument = context.lower_call_argument(expression, scope)?;
        if matches!(
            argument.representation,
            crate::mir::ValueRepresentation::View(_)
        ) && matches!(argument.operand, crate::mir::Operand::StaticStr { .. })
        {
            let origin = context
                .semantic
                .typed_hir
                .expression(expression.span())
                .map_or(
                    crate::mir::LocalOrigin::Desugared(expression.span()),
                    |expression| crate::mir::LocalOrigin::Temporary(expression.id),
                );
            let local = context.local_for_type(argument.ty, origin, scope)?;
            context
                .control_flow
                .push_statement(crate::mir::Statement::Assign {
                    destination: crate::mir::Place::local(local),
                    value: crate::mir::Rvalue::Use(argument.operand),
                    origin: crate::mir::Origin::Desugared(expression.span()),
                })?;
            argument.operand = crate::mir::Operand::Copy(crate::mir::Place::local(local));
        }
        leaves.push(AggregateLeaf {
            path: path.clone(),
            ty: argument.ty,
            representation: argument.representation,
            operand: argument.operand,
        });
        return Ok(());
    }

    // An owned aggregate nested in a variant is evaluated into its own MIR
    // local before later payload expressions. Flattening the literal directly
    // into the eventual variant rvalue would make an earlier owned value
    // invisible to initialization analysis if a later expression branches or
    // fails.
    if matches!(
        abi,
        AbiType::Struct(_) | AbiType::Array { .. } | AbiType::Enum(_) | AbiType::Outcome { .. }
    ) {
        let argument = context.lower_call_argument(expression, scope)?;
        if argument.representation != crate::mir::ValueRepresentation::Aggregate {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        leaves.push(AggregateLeaf {
            path: path.clone(),
            ty: argument.ty,
            representation: argument.representation,
            operand: argument.operand,
        });
        return Ok(());
    }

    match (expression.without_groups(), abi) {
        (Expr::StructLiteral(literal), AbiType::Struct(fields)) => {
            for field in &literal.fields {
                let index = fields
                    .iter()
                    .position(|candidate| candidate.name == field.name)
                    .ok_or(BuildError::UnsupportedClaimedExpression)?;
                path.push(AggregateElement::Field(index));
                lower_literal_leaves(
                    context,
                    &field.value,
                    &fields[index].ty,
                    scope,
                    path,
                    leaves,
                )?;
                path.pop();
            }
        }
        (Expr::ArrayLiteral(literal), AbiType::Array { element, length }) => {
            if usize::try_from(*length).ok() != Some(literal.elements.len()) {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
            for (index, value) in literal.elements.iter().enumerate() {
                path.push(AggregateElement::Index(index));
                lower_literal_leaves(context, value, element, scope, path, leaves)?;
                path.pop();
            }
        }
        _ => return Err(BuildError::UnsupportedClaimedExpression),
    }
    Ok(())
}

fn aggregate_abi_type(expression: &Expr, semantic: SemanticInputs<'_>) -> Option<AbiType> {
    let ty = known_expression_type(expression, semantic.typed_hir)?;
    let ty = semantic.typed_hir.type_expr_by_id(ty)?;
    crate::abi::abi_value_from_type_expr_with_resolver(ty, semantic.resolved, |source| {
        semantic.resolver_for(source)
    })
    .ok()
    .map(|value| value.ty)
}
