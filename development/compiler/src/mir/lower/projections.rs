//! Construction of typed MIR projection paths from checked member facts.

use super::{BuildError, SemanticInputs};
use crate::abi::{AbiType, layout_struct};
use crate::ast::{Expr, IdentifierExpr, MemberExpr};
use crate::mir::{
    LocalId, OwnershipKind, Place, ProjectionElement, ProjectionPath, ProjectionPathId, ScalarType,
    ValueRepresentation,
};
use crate::resolve::LocalSymbolId;
use std::collections::HashMap;

pub(super) fn scalar_field_is_supported(member: &MemberExpr, semantic: SemanticInputs<'_>) -> bool {
    field_path_parts(member, semantic).is_some_and(|parts| {
        parts
            .segments
            .last()
            .is_some_and(|segment| matches!(segment.representation, ValueRepresentation::Scalar(_)))
    })
}

pub(super) fn lower_scalar_field_place(
    member: &MemberExpr,
    semantic: SemanticInputs<'_>,
    locals: &HashMap<LocalSymbolId, LocalId>,
    projections: &mut Vec<ProjectionPath>,
) -> Result<(Place, ScalarType), BuildError> {
    let parts =
        field_path_parts(member, semantic).ok_or(BuildError::UnsupportedClaimedExpression)?;
    let ValueRepresentation::Scalar(scalar) = parts
        .segments
        .last()
        .ok_or(BuildError::UnsupportedClaimedExpression)?
        .representation
    else {
        return Err(BuildError::UnsupportedClaimedExpression);
    };
    let base = *locals
        .get(&parts.base_symbol)
        .ok_or(BuildError::MissingLocalSymbol)?;
    let mut parent = None;
    for segment in parts.segments {
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
        scalar,
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
}

fn field_path_parts(member: &MemberExpr, semantic: SemanticInputs<'_>) -> Option<FieldPathParts> {
    let mut members = Vec::new();
    let base = collect_member_chain(member, &mut members)?;
    let base_symbol = semantic.resolved.local_symbol_for_identifier(base)?.id;
    let base_ty = semantic.typed_hir.binding_type_expr(base_symbol)?;
    let mut current =
        crate::abi::abi_value_from_type_expr_with_resolver(base_ty, semantic.resolved, |source| {
            semantic.resolver_for(source)
        })
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
        });
    }
    Some(FieldPathParts {
        base_symbol,
        segments,
    })
}

fn collect_member_chain<'a>(
    member: &'a MemberExpr,
    members: &mut Vec<&'a MemberExpr>,
) -> Option<&'a IdentifierExpr> {
    let base = match member.object.without_groups() {
        Expr::Identifier(identifier) => identifier,
        Expr::Member(parent) => collect_member_chain(parent, members)?,
        _ => return None,
    };
    members.push(member);
    Some(base)
}
