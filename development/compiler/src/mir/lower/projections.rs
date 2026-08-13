//! Construction of typed MIR projection paths from checked member facts.

use super::{BuildError, SemanticInputs};
use crate::abi::{AbiType, layout_struct};
use crate::ast::{Expr, MemberExpr};
use crate::mir::{
    LocalId, OwnershipKind, Place, ProjectionElement, ProjectionPath, ProjectionPathId, ScalarType,
    ValueRepresentation,
};
use crate::resolve::LocalSymbolId;
use std::collections::HashMap;

pub(super) fn scalar_field_is_supported(member: &MemberExpr, semantic: SemanticInputs<'_>) -> bool {
    field_parts(member, semantic)
        .is_some_and(|parts| matches!(parts.representation, ValueRepresentation::Scalar(_)))
}

pub(super) fn lower_scalar_field_place(
    member: &MemberExpr,
    semantic: SemanticInputs<'_>,
    locals: &HashMap<LocalSymbolId, LocalId>,
    projections: &mut Vec<ProjectionPath>,
) -> Result<(Place, ScalarType), BuildError> {
    let parts = field_parts(member, semantic).ok_or(BuildError::UnsupportedClaimedExpression)?;
    let ValueRepresentation::Scalar(scalar) = parts.representation else {
        return Err(BuildError::UnsupportedClaimedExpression);
    };
    let base = *locals
        .get(&parts.base_symbol)
        .ok_or(BuildError::MissingLocalSymbol)?;
    if let Some(existing) = projections.iter().find(|projection| {
        projection.base == base
            && projection.parent.is_none()
            && projection.element
                == (ProjectionElement::Field {
                    offset: parts.offset,
                })
            && projection.ty == parts.ty
    }) {
        return Ok((Place::projected(base, existing.id), scalar));
    }
    let id = ProjectionPathId::from_index(projections.len());
    projections.push(ProjectionPath {
        id,
        base,
        parent: None,
        element: ProjectionElement::Field {
            offset: parts.offset,
        },
        ty: parts.ty,
        representation: parts.representation,
        ownership: parts.ownership,
    });
    Ok((Place::projected(base, id), scalar))
}

struct FieldParts {
    base_symbol: LocalSymbolId,
    offset: u32,
    ty: crate::semantic::TyId,
    representation: ValueRepresentation,
    ownership: OwnershipKind,
}

fn field_parts(member: &MemberExpr, semantic: SemanticInputs<'_>) -> Option<FieldParts> {
    let Expr::Identifier(base) = member.object.without_groups() else {
        return None;
    };
    let base_symbol = semantic.resolved.local_symbol_for_identifier(base)?.id;
    let base_ty = semantic.typed_hir.binding_type_expr(base_symbol)?;
    let value =
        crate::abi::abi_value_from_type_expr_with_resolver(base_ty, semantic.resolved, |source| {
            semantic.resolver_for(source)
        })
        .ok()?;
    let AbiType::Struct(fields) = &value.ty else {
        return None;
    };
    let field_index = fields
        .iter()
        .position(|field| field.name == member.member)?;
    // The selected semantic field identity prevents a same-spelling member
    // from being accepted when recovery facts are incomplete.
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
    Some(FieldParts {
        base_symbol,
        offset,
        ty,
        representation,
        ownership,
    })
}
