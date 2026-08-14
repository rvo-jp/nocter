//! Construction of checked fixed-array index places.

use super::context::LoweringContext;
use super::{BuildError, SemanticInputs};
use crate::abi::{AbiType, abi_value_from_type_expr_with_resolver, array_element_stride};
use crate::ast::{Expr, IndexExpr};
use crate::mir::{
    LocalId, Operand, OwnershipKind, Place, ProjectionElement, ProjectionPath, ProjectionPathId,
    ScalarType, ScopeId, ValueRepresentation,
};

pub(super) fn is_supported(index: &IndexExpr, semantic: SemanticInputs<'_>) -> bool {
    let Some(plan) = semantic.typed_hir.index_plan(index.span) else {
        return false;
    };
    if plan.projection == crate::typecheck::TypecheckIndexProjection::Slice {
        return plan.conversion.is_none()
            && view_is_supported(index, semantic)
            && semantic
                .typed_hir
                .type_id(&plan.element_ty)
                .and_then(|ty| super::coverage::scalar_type(ty, semantic.typed_hir))
                .is_some();
    }
    if plan.projection != crate::typecheck::TypecheckIndexProjection::Array
        || plan.conversion.is_some()
        || !base_is_supported(&index.object, semantic)
    {
        return false;
    }
    let Some(index_ty) = super::coverage::known_expression_type(&index.index, semantic.typed_hir)
    else {
        return false;
    };
    super::coverage::scalar_type(index_ty, semantic.typed_hir) == Some(ScalarType::Usize)
        && super::coverage::scalar_expression_is_supported(
            &index.index,
            semantic.resolved,
            semantic.resolved_sources,
            semantic.typed_hir,
        )
        && array_contract(index, semantic).is_some()
}

pub(super) fn view_is_supported(index: &IndexExpr, semantic: SemanticInputs<'_>) -> bool {
    let Some(plan) = semantic.typed_hir.index_plan(index.span) else {
        return false;
    };
    let kind = match plan.projection {
        crate::typecheck::TypecheckIndexProjection::Str => crate::mir::ViewKind::Str,
        crate::typecheck::TypecheckIndexProjection::Slice => crate::mir::ViewKind::Slice,
        _ => return false,
    };
    if plan.conversion.is_some() {
        return false;
    }
    let Some(source_ty) = super::coverage::known_expression_type(&index.object, semantic.typed_hir)
    else {
        return false;
    };
    let Some(index_ty) = semantic.typed_hir.type_id(&plan.index_ty) else {
        return false;
    };
    let index_is_usize =
        super::coverage::scalar_type(index_ty, semantic.typed_hir) == Some(ScalarType::Usize);
    let index_is_contextual_literal = matches!(index.index.without_groups(), Expr::IntegerLiteral(literal)
        if crate::literals::decode_integer_literal_value(&literal.value)
            .is_some_and(|value| u64::try_from(value).is_ok()));
    super::coverage::value_representation(source_ty, semantic)
        == Some(ValueRepresentation::View(kind))
        && super::coverage::value_expression_is_supported(
            &index.object,
            ValueRepresentation::View(kind),
            semantic,
        )
        && (index_is_usize || index_is_contextual_literal)
        && super::coverage::scalar_expression_is_supported(
            &index.index,
            semantic.resolved,
            semantic.resolved_sources,
            semantic.typed_hir,
        )
}

pub(super) fn lower_place(
    context: &mut LoweringContext<'_>,
    index: &IndexExpr,
    scope: ScopeId,
) -> Result<(Place, ValueRepresentation), BuildError> {
    if !is_supported(index, context.semantic) {
        return Err(BuildError::UnsupportedClaimedExpression);
    }
    if context
        .semantic
        .typed_hir
        .index_plan(index.span)
        .is_some_and(|plan| plan.projection == crate::typecheck::TypecheckIndexProjection::Slice)
    {
        return lower_view_index_place(context, index, scope);
    }
    let (base, parent) = lower_base(context, &index.object)?;
    let contract =
        array_contract(index, context.semantic).ok_or(BuildError::UnsupportedClaimedExpression)?;
    let index_ty = super::coverage::known_expression_type(&index.index, context.semantic.typed_hir)
        .ok_or(BuildError::MissingTypedExpression)?;
    let index_operand = context.lower_operand(&index.index, index_ty, ScalarType::Usize, scope)?;
    let element = ProjectionElement::Index {
        index: index_operand,
        length: contract.length,
        stride: contract.stride,
    };
    let id = if let Some(existing) = context.projections.iter().find(|projection| {
        projection.base == base
            && projection.parent == parent
            && projection.element == element
            && projection.ty == contract.ty
    }) {
        existing.id
    } else {
        let id = ProjectionPathId::from_index(context.projections.len());
        context.projections.push(ProjectionPath {
            id,
            base,
            parent,
            element,
            ty: contract.ty,
            representation: contract.representation,
            ownership: contract.ownership,
            drop_plan: if contract.ownership == OwnershipKind::Move
                && contract.representation == ValueRepresentation::Aggregate
            {
                Some(
                    super::super::drop_plans::build(
                        &contract.type_expr,
                        context.semantic.resolved,
                        context.semantic.resolved_sources,
                        context.semantic.typed_hir,
                        &mut context.drop_plans,
                    )
                    .ok_or(BuildError::UnsupportedClaimedExpression)?,
                )
            } else {
                None
            },
        });
        id
    };
    Ok((Place::projected(base, id), contract.representation))
}

fn lower_view_index_place(
    context: &mut LoweringContext<'_>,
    index: &IndexExpr,
    scope: ScopeId,
) -> Result<(Place, ValueRepresentation), BuildError> {
    let plan = context
        .semantic
        .typed_hir
        .index_plan(index.span)
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
    let source_ty =
        super::coverage::known_expression_type(&index.object, context.semantic.typed_hir)
            .ok_or(BuildError::MissingTypedExpression)?;
    let Operand::Copy(source) =
        context.lower_view_operand(&index.object, source_ty, crate::mir::ViewKind::Slice, scope)?
    else {
        return Err(BuildError::UnsupportedClaimedExpression);
    };
    let checked_index_ty = context
        .semantic
        .typed_hir
        .type_id(&plan.index_ty)
        .ok_or(BuildError::MissingTypedExpression)?;
    let (index_ty, index_scalar) =
        if super::coverage::scalar_type(checked_index_ty, context.semantic.typed_hir)
            == Some(ScalarType::Usize)
        {
            (checked_index_ty, ScalarType::Usize)
        } else if matches!(index.index.without_groups(), Expr::IntegerLiteral(_)) {
            let usize_ty = context
                .semantic
                .typed_hir
                .type_id(&crate::ast::TypeExpr::Reference(
                    crate::ast::TypeReference {
                        span: index.index.span(),
                        name: "usize".to_string(),
                    },
                ))
                .ok_or(BuildError::MissingTypedExpression)?;
            (usize_ty, ScalarType::Usize)
        } else {
            return Err(BuildError::UnsupportedClaimedExpression);
        };
    let index_operand = context.lower_operand(&index.index, index_ty, index_scalar, scope)?;
    let ty = context
        .semantic
        .typed_hir
        .type_id(&plan.element_ty)
        .ok_or(BuildError::MissingTypedExpression)?;
    let scalar = super::coverage::scalar_type(ty, context.semantic.typed_hir)
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
    let id = ProjectionPathId::from_index(context.projections.len());
    context.projections.push(ProjectionPath {
        id,
        base: source.local,
        parent: source.projection,
        element: ProjectionElement::ViewIndex {
            index: index_operand,
        },
        ty,
        representation: ValueRepresentation::Scalar(scalar),
        ownership: OwnershipKind::Copy,
        drop_plan: None,
    });
    Ok((
        Place::projected(source.local, id),
        ValueRepresentation::Scalar(scalar),
    ))
}

fn base_is_supported(expression: &Expr, semantic: SemanticInputs<'_>) -> bool {
    match expression.without_groups() {
        Expr::Identifier(identifier) => semantic
            .resolved
            .local_symbol_for_identifier(identifier)
            .is_some(),
        Expr::Member(member) => super::projections::owned_field_is_supported(member, semantic),
        _ => false,
    }
}

fn lower_base(
    context: &mut LoweringContext<'_>,
    expression: &Expr,
) -> Result<(LocalId, Option<ProjectionPathId>), BuildError> {
    match expression.without_groups() {
        Expr::Identifier(identifier) => {
            let symbol = context
                .semantic
                .resolved
                .local_symbol_for_identifier(identifier)
                .ok_or(BuildError::MissingLocalSymbol)?;
            let place = *context
                .places_by_symbol
                .get(&symbol.id)
                .ok_or(BuildError::MissingLocalSymbol)?;
            Ok((place.local, place.projection))
        }
        Expr::Member(member) => {
            let (place, representation) = super::projections::lower_field_place(
                member,
                context.semantic,
                &context.places_by_symbol,
                &mut context.projections,
                &mut context.drop_plans,
            )?;
            if representation != ValueRepresentation::Aggregate {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
            Ok((place.local, place.projection))
        }
        _ => Err(BuildError::UnsupportedClaimedExpression),
    }
}

struct ArrayContract {
    length: u64,
    stride: u32,
    ty: crate::semantic::TyId,
    type_expr: crate::ast::TypeExpr,
    representation: ValueRepresentation,
    ownership: OwnershipKind,
}

fn array_contract(index: &IndexExpr, semantic: SemanticInputs<'_>) -> Option<ArrayContract> {
    let plan = semantic.typed_hir.index_plan(index.span)?;
    let abi =
        abi_value_from_type_expr_with_resolver(&plan.target_ty, semantic.resolved, |source| {
            semantic.resolver_for(source)
        })
        .ok()?;
    let AbiType::Array { element, length } = abi.ty else {
        return None;
    };
    let stride = u32::try_from(array_element_stride(&element).ok()?).ok()?;
    let ty = semantic.typed_hir.type_id(&plan.element_ty)?;
    let representation = super::coverage::scalar_type(ty, semantic.typed_hir)
        .map(ValueRepresentation::Scalar)
        .unwrap_or(ValueRepresentation::Aggregate);
    let ownership = if crate::typecheck::type_expr_is_copy(&plan.element_ty, semantic.resolved)? {
        OwnershipKind::Copy
    } else {
        OwnershipKind::Move
    };
    Some(ArrayContract {
        length,
        stride,
        ty,
        type_expr: plan.element_ty.clone(),
        representation,
        ownership,
    })
}
