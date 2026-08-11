//! Semantic selection for indexing expressions.
//!
//! Indexing is selected once from the directly available representation, a
//! lexical operator requirement, or one visible receiver coercion. Consumers
//! must persist and reuse the selected plan instead of rediscovering a
//! conversion from the source spelling.

use super::coercions::{SelectedCoercion, receiver_coercion_candidates};
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use super::numeric::is_integer_type;
use crate::ast::IndexExpr;
use crate::resolve::ResolveOutput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IndexAccess {
    Readonly,
    Readwrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IndexProjection {
    Array,
    Slice,
    Str,
    Requirement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SelectedIndex {
    pub(super) target_type: Type,
    pub(super) index_type: Type,
    pub(super) element_type: Type,
    pub(super) access: IndexAccess,
    pub(super) projection: IndexProjection,
    pub(super) coercion: Option<SelectedCoercion>,
    pub(super) requirement_span: Option<crate::source::ByteSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IndexRejection {
    UnsupportedTarget,
    InvalidIndex,
    RequiresReadwrite,
    AmbiguousCoercion,
}

pub(super) fn select_index_expression(
    expression: &IndexExpr,
    access: IndexAccess,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Result<SelectedIndex, IndexRejection> {
    let target = expression_type(&expression.object, resolved, environment);
    let index = expression_type(&expression.index, resolved, environment);
    let source_is_writable =
        super::places::expression_is_writable_place(&expression.object, resolved, environment);
    select_index_types(
        &target,
        &index,
        access,
        source_is_writable,
        resolved,
        environment,
    )
}

pub(super) fn select_index_types(
    target: &Type,
    index: &Type,
    access: IndexAccess,
    source_is_writable: bool,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Result<SelectedIndex, IndexRejection> {
    if target.is_unknown_or_unresolved() || index.is_unknown_or_unresolved() {
        return Err(IndexRejection::UnsupportedTarget);
    }

    if let Some((element, projection, writable)) = direct_projection(target) {
        if !is_integer_type(index) {
            return Err(IndexRejection::InvalidIndex);
        }
        if access == IndexAccess::Readwrite && !(writable || source_is_writable) {
            return Err(IndexRejection::RequiresReadwrite);
        }
        return Ok(selected(
            target, index, element, access, projection, None, None,
        ));
    }

    if let Some(requirement) =
        environment.index_requirement(target, index, access == IndexAccess::Readwrite)
    {
        return Ok(selected(
            target,
            index,
            requirement.element.clone(),
            access,
            IndexProjection::Requirement,
            None,
            Some(requirement.span),
        ));
    }

    let (source, source_is_readwrite) = match target {
        Type::Borrow {
            is_readwrite,
            inner,
        } => (inner.as_ref(), *is_readwrite),
        target if target.nominal_name().is_some() => (target, source_is_writable),
        _ => return Err(IndexRejection::UnsupportedTarget),
    };
    if access == IndexAccess::Readwrite && !source_is_readwrite {
        return Err(IndexRejection::RequiresReadwrite);
    }
    if !is_integer_type(index) {
        return Err(IndexRejection::InvalidIndex);
    }

    let mut candidates = receiver_coercion_candidates(source, source_is_readwrite, resolved)
        .into_iter()
        .filter_map(|coercion| {
            let (element, projection, writable) = direct_projection(&coercion.target_type)?;
            if access == IndexAccess::Readwrite && !writable {
                return None;
            }
            Some((coercion, element, projection))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(IndexRejection::UnsupportedTarget);
    }

    if access == IndexAccess::Readonly
        && candidates
            .iter()
            .any(|candidate| !projection_is_readwrite(&candidate.0.target_type))
    {
        candidates.retain(|candidate| !projection_is_readwrite(&candidate.0.target_type));
    }

    // Capability-equivalent declarations of the same projection are ordered
    // by receiver selection. Distinct target projections are ambiguous: the
    // source author must remove the competing coercion instead of depending on
    // declaration order.
    let first_target = candidates[0].0.target_type.clone();
    if candidates
        .iter()
        .skip(1)
        .any(|candidate| candidate.0.target_type != first_target)
    {
        return Err(IndexRejection::AmbiguousCoercion);
    }
    let (coercion, element, projection) = candidates.remove(0);
    Ok(selected(
        target,
        index,
        element,
        access,
        projection,
        Some(coercion),
        None,
    ))
}

fn projection_is_readwrite(ty: &Type) -> bool {
    matches!(
        ty,
        Type::View {
            is_readwrite: true,
            ..
        }
    )
}

fn direct_projection(ty: &Type) -> Option<(Type, IndexProjection, bool)> {
    match ty {
        Type::Array { element, .. } => {
            Some((element.as_ref().clone(), IndexProjection::Array, false))
        }
        Type::View {
            is_readwrite,
            element,
        } => Some((
            element.as_ref().clone(),
            IndexProjection::Slice,
            *is_readwrite,
        )),
        Type::Str => Some((
            Type::Primitive("u8".to_string()),
            IndexProjection::Str,
            false,
        )),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn selected(
    target: &Type,
    index: &Type,
    element: Type,
    access: IndexAccess,
    projection: IndexProjection,
    coercion: Option<SelectedCoercion>,
    requirement_span: Option<crate::source::ByteSpan>,
) -> SelectedIndex {
    SelectedIndex {
        target_type: target.clone(),
        index_type: index.clone(),
        element_type: element,
        access,
        projection,
        coercion,
        requirement_span,
    }
}

pub(crate) fn specialize_index_plan(
    mut plan: super::facts::TypecheckIndexPlan,
    resolved: &ResolveOutput,
) -> Option<super::facts::TypecheckIndexPlan> {
    if plan.conversion.is_some()
        || plan.projection != super::facts::TypecheckIndexProjection::Requirement
    {
        return Some(plan);
    }
    let target = super::type_expr::type_expr_to_type(&plan.target_ty, resolved);
    let index = super::type_expr::type_expr_to_type(&plan.index_ty, resolved);
    let source_is_writable = matches!(
        target,
        Type::Borrow {
            is_readwrite: true,
            ..
        }
    );
    let selected = select_index_types(
        &target,
        &index,
        match plan.access {
            super::facts::TypecheckIndexAccess::Readonly => IndexAccess::Readonly,
            super::facts::TypecheckIndexAccess::Readwrite => IndexAccess::Readwrite,
        },
        source_is_writable,
        resolved,
        &TypeEnvironment::default(),
    )
    .ok()?;
    plan.projection = match selected.projection {
        IndexProjection::Array => super::facts::TypecheckIndexProjection::Array,
        IndexProjection::Slice => super::facts::TypecheckIndexProjection::Slice,
        IndexProjection::Str => super::facts::TypecheckIndexProjection::Str,
        IndexProjection::Requirement => return None,
    };
    plan.requirement_span = None;
    plan.conversion = selected.coercion.and_then(|coercion| {
        super::facts::typecheck_conversion_plan(
            plan.object_span,
            plan.object_span,
            None,
            super::conversions::selected_receiver_coercion(&target, coercion),
        )
    });
    Some(plan)
}
