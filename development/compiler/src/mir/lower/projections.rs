//! Construction of typed MIR projection paths from checked member facts.

use super::{BuildError, SemanticInputs};
use crate::abi::{AbiType, layout_struct};
use crate::ast::{Expr, IdentifierExpr, MemberExpr};
use crate::mir::{
    LocalId, OwnershipKind, Place, ProjectionElement, ProjectionPath, ProjectionPathId,
    ValueRepresentation,
};
use crate::resolve::LocalSymbolId;
use std::collections::HashMap;

pub(super) fn scalar_field_is_supported(member: &MemberExpr, semantic: SemanticInputs<'_>) -> bool {
    field_path_parts(member, semantic, true).is_some_and(|parts| {
        parts
            .segments
            .last()
            .is_some_and(|segment| matches!(segment.representation, ValueRepresentation::Scalar(_)))
    })
}

pub(super) fn scalar_value_field_is_supported(
    member: &MemberExpr,
    semantic: SemanticInputs<'_>,
) -> bool {
    value_root_field_segments(member, semantic).is_some_and(|(_, segments)| {
        segments
            .last()
            .is_some_and(|segment| matches!(segment.representation, ValueRepresentation::Scalar(_)))
    })
}

pub(super) fn error_field_is_supported(member: &MemberExpr, semantic: SemanticInputs<'_>) -> bool {
    let Expr::Identifier(base) = member.object.without_groups() else {
        return false;
    };
    let Some(symbol) = semantic.resolved.local_symbol_for_identifier(base) else {
        return false;
    };
    matches!(
        semantic.typed_hir.binding_type_expr(symbol.id),
        Some(crate::ast::TypeExpr::Reference(reference))
            if crate::builtin_types::BuiltinTypeOwner::from_reference_name(&reference.name)
                == Some(crate::builtin_types::BuiltinTypeOwner::Error)
    ) && crate::builtin_types::BuiltinErrorField::from_source_name(&member.member).is_some()
        && semantic
            .typed_hir
            .expression(member.span)
            .and_then(|expression| match expression.ty {
                crate::typecheck::PartialSemantic::Known(ty) => Some(ty),
                crate::typecheck::PartialSemantic::Error => None,
            })
            .and_then(|ty| super::coverage::value_representation(ty, semantic))
            == Some(ValueRepresentation::View(crate::mir::ViewKind::Str))
}

pub(super) fn lower_error_field_place(
    member: &MemberExpr,
    semantic: SemanticInputs<'_>,
    places: &HashMap<LocalSymbolId, Place>,
    projections: &mut Vec<ProjectionPath>,
) -> Result<Place, BuildError> {
    if !error_field_is_supported(member, semantic) {
        return Err(BuildError::UnsupportedClaimedExpression);
    }
    let Expr::Identifier(base) = member.object.without_groups() else {
        unreachable!("supported error fields have identifier bases")
    };
    let symbol = semantic
        .resolved
        .local_symbol_for_identifier(base)
        .ok_or(BuildError::MissingLocalSymbol)?;
    let base = *places
        .get(&symbol.id)
        .ok_or(BuildError::MissingLocalSymbol)?;
    let field = crate::builtin_types::BuiltinErrorField::from_source_name(&member.member)
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
    let ty = semantic
        .typed_hir
        .expression(member.span)
        .and_then(|expression| match expression.ty {
            crate::typecheck::PartialSemantic::Known(ty) => Some(ty),
            crate::typecheck::PartialSemantic::Error => None,
        })
        .ok_or(BuildError::MissingTypedExpression)?;
    if base.projection.is_some() {
        return Err(BuildError::UnsupportedClaimedExpression);
    }
    Ok(push_error_field_place(base.local, field, ty, projections))
}

pub(super) fn push_error_field_place(
    base: LocalId,
    field: crate::builtin_types::BuiltinErrorField,
    ty: crate::semantic::TyId,
    projections: &mut Vec<ProjectionPath>,
) -> Place {
    let element = ProjectionElement::ErrorField(field);
    let projection = projections
        .iter()
        .find(|projection| {
            projection.base == base
                && projection.parent.is_none()
                && projection.element == element
                && projection.ty == ty
        })
        .map(|projection| projection.id)
        .unwrap_or_else(|| {
            let id = ProjectionPathId::from_index(projections.len());
            projections.push(ProjectionPath {
                id,
                base,
                parent: None,
                element,
                ty,
                representation: ValueRepresentation::View(crate::mir::ViewKind::Str),
                ownership: OwnershipKind::Copy,
                drop_plan: None,
            });
            id
        });
    Place::projected(base, projection)
}

pub(super) fn field_is_supported(member: &MemberExpr, semantic: SemanticInputs<'_>) -> bool {
    field_path_parts(member, semantic, true).is_some()
}

pub(super) fn owned_field_is_supported(member: &MemberExpr, semantic: SemanticInputs<'_>) -> bool {
    field_path_parts(member, semantic, false).is_some()
}

pub(super) fn aggregate_value_field_is_supported(
    member: &MemberExpr,
    semantic: SemanticInputs<'_>,
) -> bool {
    value_root_field_segments(member, semantic).is_some_and(|(_, segments)| {
        segments
            .last()
            .is_some_and(|segment| segment.representation == ValueRepresentation::Aggregate)
    })
}

fn value_root_field_segments<'a>(
    member: &'a MemberExpr,
    semantic: SemanticInputs<'_>,
) -> Option<(&'a Expr, Vec<FieldSegment>)> {
    let mut members = Vec::new();
    let root = collect_member_chain_root(member, &mut members);
    let root_ty = semantic
        .typed_hir
        .expression(root.span())
        .and_then(|expression| match expression.ty {
            crate::typecheck::PartialSemantic::Known(ty) => Some(ty),
            crate::typecheck::PartialSemantic::Error => None,
        })
        .and_then(|ty| semantic.typed_hir.type_expr_by_id(ty))?;
    Some((root, field_segments(root_ty, &members, semantic)?))
}

pub(super) fn member_chain_root(member: &MemberExpr) -> &Expr {
    let mut members = Vec::new();
    collect_member_chain_root(member, &mut members)
}

pub(super) fn lower_field_place(
    member: &MemberExpr,
    semantic: SemanticInputs<'_>,
    places: &HashMap<LocalSymbolId, Place>,
    projections: &mut Vec<ProjectionPath>,
    drop_plans: &mut Vec<crate::mir::DropPlan>,
) -> Result<(Place, ValueRepresentation), BuildError> {
    lower_field_place_with_borrow_base(member, semantic, places, projections, drop_plans, false)
}

pub(super) fn lower_borrow_field_place(
    member: &MemberExpr,
    semantic: SemanticInputs<'_>,
    places: &HashMap<LocalSymbolId, Place>,
    projections: &mut Vec<ProjectionPath>,
    drop_plans: &mut Vec<crate::mir::DropPlan>,
) -> Result<(Place, ValueRepresentation), BuildError> {
    lower_field_place_with_borrow_base(member, semantic, places, projections, drop_plans, true)
}

/// Materializes the complete owned-field projection tree needed by partial
/// move dataflow. A move may mention one field, but cleanup must retain every
/// initialized sibling without reconstructing source member expressions.
pub(super) fn ensure_owned_drop_projections(
    base: LocalId,
    root_ty: crate::semantic::TyId,
    root_plan: crate::mir::DropPlanId,
    semantic: SemanticInputs<'_>,
    projections: &mut Vec<ProjectionPath>,
    drop_plans: &[crate::mir::DropPlan],
) -> Result<(), BuildError> {
    let ty = semantic
        .typed_hir
        .type_expr_by_id(root_ty)
        .ok_or(BuildError::MissingTypedExpression)?;
    let abi = crate::abi::abi_value_from_type_expr_with_resolver(ty, semantic.resolved, |source| {
        semantic.resolver_for(source)
    })
    .map_err(|_| BuildError::UnsupportedClaimedExpression)?
    .ty;
    ensure_owned_drop_projections_inner(
        base,
        None,
        &abi,
        root_plan,
        semantic,
        projections,
        drop_plans,
    )
}

fn ensure_owned_drop_projections_inner(
    base: LocalId,
    parent: Option<ProjectionPathId>,
    abi: &AbiType,
    plan: crate::mir::DropPlanId,
    semantic: SemanticInputs<'_>,
    projections: &mut Vec<ProjectionPath>,
    drop_plans: &[crate::mir::DropPlan],
) -> Result<(), BuildError> {
    let Some(crate::mir::DropPlan::Struct { fields, .. }) = drop_plans.get(plan.index()) else {
        return Ok(());
    };
    let AbiType::Struct(abi_fields) = abi else {
        return Err(BuildError::UnsupportedClaimedExpression);
    };
    let layout = layout_struct(abi_fields).map_err(|_| BuildError::UnsupportedClaimedExpression)?;
    for field in fields {
        let field_ty = semantic
            .typed_hir
            .type_id(&field.ty)
            .ok_or(BuildError::MissingTypedExpression)?;
        let offset = layout
            .fields
            .get(field.index)
            .and_then(|field| u32::try_from(field.offset).ok())
            .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let element = ProjectionElement::Field { offset };
        let id = projections
            .iter()
            .find(|projection| {
                projection.base == base
                    && projection.parent == parent
                    && projection.element == element
                    && projection.ty == field_ty
            })
            .map(|projection| projection.id)
            .unwrap_or_else(|| {
                let id = ProjectionPathId::from_index(projections.len());
                projections.push(ProjectionPath {
                    id,
                    base,
                    parent,
                    element,
                    ty: field_ty,
                    representation: ValueRepresentation::Aggregate,
                    ownership: OwnershipKind::Move,
                    drop_plan: Some(field.plan),
                });
                id
            });
        ensure_owned_drop_projections_inner(
            base,
            Some(id),
            &abi_fields[field.index].ty,
            field.plan,
            semantic,
            projections,
            drop_plans,
        )?;
    }
    Ok(())
}

fn lower_field_place_with_borrow_base(
    member: &MemberExpr,
    semantic: SemanticInputs<'_>,
    places: &HashMap<LocalSymbolId, Place>,
    projections: &mut Vec<ProjectionPath>,
    drop_plans: &mut Vec<crate::mir::DropPlan>,
    allow_borrow_base: bool,
) -> Result<(Place, ValueRepresentation), BuildError> {
    let parts = field_path_parts(member, semantic, allow_borrow_base)
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
    let base_place = *places
        .get(&parts.base_symbol)
        .ok_or(BuildError::MissingLocalSymbol)?;
    push_field_place(
        base_place,
        parts.segments,
        semantic,
        projections,
        drop_plans,
    )
}

pub(super) fn lower_field_place_from_value_root(
    member: &MemberExpr,
    root_ty: crate::semantic::TyId,
    base_place: Place,
    semantic: SemanticInputs<'_>,
    projections: &mut Vec<ProjectionPath>,
    drop_plans: &mut Vec<crate::mir::DropPlan>,
) -> Result<(Place, ValueRepresentation), BuildError> {
    let mut members = Vec::new();
    collect_member_chain_root(member, &mut members);
    let root_ty = semantic
        .typed_hir
        .type_expr_by_id(root_ty)
        .ok_or(BuildError::MissingTypedExpression)?;
    let segments = field_segments(root_ty, &members, semantic)
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
    push_field_place(base_place, segments, semantic, projections, drop_plans)
}

fn push_field_place(
    base_place: Place,
    segments: Vec<FieldSegment>,
    semantic: SemanticInputs<'_>,
    projections: &mut Vec<ProjectionPath>,
    drop_plans: &mut Vec<crate::mir::DropPlan>,
) -> Result<(Place, ValueRepresentation), BuildError> {
    let representation = segments
        .last()
        .ok_or(BuildError::UnsupportedClaimedExpression)?
        .representation;
    let base = base_place.local;
    let mut parent = base_place.projection;
    for segment in segments {
        let element = ProjectionElement::Field {
            offset: segment.offset,
        };
        let id = if let Some(existing) = projections.iter().find(|projection| {
            projection.base == base
                && projection.parent == parent
                && projection.element == element
                && projection.ty == segment.ty
        }) {
            existing.id
        } else {
            let id = ProjectionPathId::from_index(projections.len());
            projections.push(ProjectionPath {
                id,
                base,
                parent,
                element,
                ty: segment.ty,
                representation: segment.representation,
                ownership: segment.ownership,
                drop_plan: if segment.ownership == OwnershipKind::Move {
                    Some(
                        super::super::drop_plans::build(
                            &segment.type_expr,
                            semantic.resolved,
                            semantic.resolved_sources,
                            semantic.typed_hir,
                            drop_plans,
                        )
                        .ok_or(BuildError::UnsupportedClaimedExpression)?,
                    )
                } else {
                    None
                },
            });
            id
        };
        parent = Some(id);
    }
    Ok((
        Place::projected(
            base,
            parent.ok_or(BuildError::UnsupportedClaimedExpression)?,
        ),
        representation,
    ))
}

struct FieldPathParts {
    base_symbol: LocalSymbolId,
    segments: Vec<FieldSegment>,
}

struct FieldSegment {
    offset: u32,
    ty: crate::semantic::TyId,
    representation: ValueRepresentation,
    ownership: OwnershipKind,
    type_expr: crate::ast::TypeExpr,
}

fn field_path_parts(
    member: &MemberExpr,
    semantic: SemanticInputs<'_>,
    allow_borrow_base: bool,
) -> Option<FieldPathParts> {
    let mut members = Vec::new();
    let base = collect_member_chain(member, &mut members)?;
    let base_symbol = semantic.resolved.local_symbol_for_identifier(base)?.id;
    let base_ty = semantic.typed_hir.binding_type_expr(base_symbol)?;
    let layout_ty = match base_ty {
        crate::ast::TypeExpr::Borrow(borrow) if allow_borrow_base => borrow.inner.as_ref(),
        ty => ty,
    };
    let segments = field_segments(layout_ty, &members, semantic)?;
    Some(FieldPathParts {
        base_symbol,
        segments,
    })
}

fn field_segments(
    layout_ty: &crate::ast::TypeExpr,
    members: &[&MemberExpr],
    semantic: SemanticInputs<'_>,
) -> Option<Vec<FieldSegment>> {
    let mut current = crate::abi::abi_value_from_type_expr_with_resolver(
        layout_ty,
        semantic.resolved,
        |source| semantic.resolver_for(source),
    )
    .ok()?
    .ty;
    let mut segments = Vec::with_capacity(members.len());
    for member in members {
        let AbiType::Struct(fields) = &current else {
            return None;
        };
        let field_index = fields
            .iter()
            .position(|field| field.name == member.member)?;
        // A checked field target prevents recovery spelling from becoming a
        // successful projection identity.
        semantic.typed_hir.field_target(member.member_span)?;
        let offset = u32::try_from(layout_struct(fields).ok()?.fields[field_index].offset).ok()?;
        let ty =
            semantic
                .typed_hir
                .expression(member.span)
                .and_then(|expression| match expression.ty {
                    crate::typecheck::PartialSemantic::Known(ty) => Some(ty),
                    crate::typecheck::PartialSemantic::Error => None,
                })?;
        let representation = super::coverage::scalar_type(ty, semantic.typed_hir)
            .map(ValueRepresentation::Scalar)
            .unwrap_or(ValueRepresentation::Aggregate);
        let field_ty = semantic.typed_hir.field_type_expr(member.member_span)?;
        let ownership = if crate::typecheck::type_expr_is_copy(field_ty, semantic.resolved)? {
            OwnershipKind::Copy
        } else {
            OwnershipKind::Move
        };
        current = fields[field_index].ty.clone();
        segments.push(FieldSegment {
            offset,
            ty,
            representation,
            ownership,
            type_expr: field_ty.clone(),
        });
    }
    Some(segments)
}

fn collect_member_chain<'a>(
    member: &'a MemberExpr,
    members: &mut Vec<&'a MemberExpr>,
) -> Option<&'a IdentifierExpr> {
    let Expr::Identifier(base) = collect_member_chain_root(member, members) else {
        return None;
    };
    Some(base)
}

fn collect_member_chain_root<'a>(
    member: &'a MemberExpr,
    members: &mut Vec<&'a MemberExpr>,
) -> &'a Expr {
    let base = match member.object.without_groups() {
        Expr::Member(parent) => collect_member_chain_root(parent, members),
        expression => expression,
    };
    members.push(member);
    base
}
