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
use crate::ast::{CallExpr, Expr, IndexExpr, MemberExpr};
use crate::resolve::ResolveOutput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IndexAccess {
    Readonly,
    Readwrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IndexProjection {
    Array,
    Slice,
    Str,
    Requirement,
    Declared,
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
    pub(super) method: Option<super::facts::TypecheckProtocolMethod>,
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
    let source_is_writable = super::places::expression_supports_readwrite_access(
        &expression.object,
        resolved,
        environment,
    );
    select_index_types_inner(
        &target,
        &index,
        access,
        source_is_writable,
        expression.span,
        Some(expression),
        resolved,
        environment,
    )
}

pub(super) fn select_index_types(
    target: &Type,
    index: &Type,
    access: IndexAccess,
    source_is_writable: bool,
    selection_span: crate::source::ByteSpan,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Result<SelectedIndex, IndexRejection> {
    select_index_types_inner(
        target,
        index,
        access,
        source_is_writable,
        selection_span,
        None,
        resolved,
        environment,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_index_types_inner(
    target: &Type,
    index: &Type,
    access: IndexAccess,
    source_is_writable: bool,
    selection_span: crate::source::ByteSpan,
    expression: Option<&IndexExpr>,
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

    let declared = select_declared_index(
        target,
        index,
        access,
        source_is_writable,
        selection_span,
        expression,
        resolved,
        environment,
    );
    if declared
        .as_ref()
        .is_some_and(|selected| selected.coercion.is_none())
    {
        return Ok(declared.expect("checked direct declared index"));
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
        if let Some(declared) = declared {
            return Ok(declared);
        }
        return Err(IndexRejection::InvalidIndex);
    }

    let mut candidates =
        receiver_coercion_candidates(source, source_is_readwrite, resolved, environment)
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
        if let Some(declared) = declared {
            return Ok(declared);
        }
        return Err(IndexRejection::UnsupportedTarget);
    }

    if access == IndexAccess::Readonly
        && candidates
            .iter()
            .any(|candidate| !projection_is_readwrite(&candidate.0.target_type))
    {
        candidates.retain(|candidate| !projection_is_readwrite(&candidate.0.target_type));
    }

    if let Some(declared) = declared {
        let declared_target = declared
            .coercion
            .as_ref()
            .expect("non-direct declared index has a coercion")
            .target_type
            .clone();
        if candidates
            .iter()
            .any(|candidate| candidate.0.target_type != declared_target)
        {
            return Err(IndexRejection::AmbiguousCoercion);
        }
        return Ok(declared);
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

pub(crate) fn synthetic_index_call(expression: &IndexExpr, access: IndexAccess) -> CallExpr {
    let method_name = match access {
        IndexAccess::Readonly => crate::semantic::OperatorCallableKind::ReadonlyIndex.lookup_name(),
        IndexAccess::Readwrite => {
            crate::semantic::OperatorCallableKind::ReadwriteIndex.lookup_name()
        }
    };
    CallExpr {
        span: expression.span,
        callee: Box::new(Expr::Member(MemberExpr {
            span: expression.span,
            object: expression.object.clone(),
            member: method_name.to_string(),
            member_span: expression.index_span,
        })),
        arguments_span: expression.index.span(),
        arguments: vec![expression.index.as_ref().clone()],
    }
}

fn select_declared_index(
    target: &Type,
    index: &Type,
    access: IndexAccess,
    source_is_writable: bool,
    selection_span: crate::source::ByteSpan,
    source_expression: Option<&IndexExpr>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<SelectedIndex> {
    if access == IndexAccess::Readwrite && !source_is_writable {
        return None;
    }
    let span = selection_span;
    let mut local = environment.clone();
    local.define("__nocter_index_target".to_string(), target.clone());
    local.define("__nocter_index_value".to_string(), index.clone());
    let synthetic_expression;
    let expression = match source_expression {
        Some(expression) => expression,
        None => {
            synthetic_expression = IndexExpr {
                span,
                object: Box::new(Expr::Identifier(crate::ast::IdentifierExpr {
                    span,
                    name: "__nocter_index_target".to_string(),
                })),
                index_span: span,
                index: Box::new(Expr::Identifier(crate::ast::IdentifierExpr {
                    span,
                    name: "__nocter_index_value".to_string(),
                })),
            };
            &synthetic_expression
        }
    };
    let call = synthetic_index_call(expression, access);
    let selected = super::calls::resolved_method_call(resolved, &call, &local);
    let selected = selected?;
    let parameter = selected.method.signature.parameters.first()?;
    let parameters = selected
        .method
        .signature
        .generic_parameters
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    let mut substitutions = std::collections::HashMap::new();
    if let Some(owner_target) = &selected.method.owner_target_ty {
        super::type_expr::infer_type_expr_substitutions(
            owner_target,
            selected.self_type.opaque_lowering_view(),
            resolved,
            None,
            &parameters,
            &mut substitutions,
        );
    }
    let expected_index = super::type_expr::type_expr_to_type_with_substitutions(
        &parameter.ty,
        resolved,
        Some(&selected.self_type),
        &substitutions,
    );
    let index_matches = source_expression.is_some_and(|expression| {
        super::operations::is_expression_assignable(
            &expected_index,
            &expression.index,
            resolved,
            environment,
        )
    }) || (source_expression.is_none()
        && local.types_equal(&expected_index, index));
    if !index_matches {
        return None;
    }
    let result = super::type_expr::type_expr_to_type_with_substitutions(
        &selected.method.signature.return_type,
        resolved,
        Some(&selected.self_type),
        &substitutions,
    );
    let Type::Borrow {
        is_readwrite,
        inner,
    } = result
    else {
        return None;
    };
    if is_readwrite != (access == IndexAccess::Readwrite) {
        return None;
    }
    let method = index_method_fact(&selected, span, resolved)?;
    Some(SelectedIndex {
        target_type: target.clone(),
        index_type: index.clone(),
        element_type: *inner,
        access,
        projection: IndexProjection::Declared,
        coercion: selected.receiver_coercion,
        requirement_span: None,
        method: Some(method),
    })
}

fn index_method_fact(
    selected: &super::calls::ResolvedMethodCall<'_>,
    span: crate::source::ByteSpan,
    resolved: &ResolveOutput,
) -> Option<super::facts::TypecheckProtocolMethod> {
    let mut free_type_parameters = std::collections::HashSet::new();
    let self_ty = super::facts::type_to_type_expr_allowing_parameters(
        selected.self_type.opaque_lowering_view(),
        span,
        &mut free_type_parameters,
    )?;
    Some(super::facts::TypecheckProtocolMethod::new(
        resolved
            .semantic_db
            .definition_at(selected.method.name_span)
            .expect("resolved index operator must have a semantic definition"),
        selected.method.name_span,
        super::facts::method_target_name_from_self_ty(&self_ty, &selected.method.name),
        self_ty,
        selected.method.receiver.mode,
        selected.method.name.clone(),
        free_type_parameters,
    ))
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
        method: None,
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
        plan.expression_span,
        resolved,
        &TypeEnvironment::default(),
    )
    .ok()?;
    plan.projection = match selected.projection {
        IndexProjection::Array => super::facts::TypecheckIndexProjection::Array,
        IndexProjection::Slice => super::facts::TypecheckIndexProjection::Slice,
        IndexProjection::Str => super::facts::TypecheckIndexProjection::Str,
        IndexProjection::Requirement => return None,
        IndexProjection::Declared => super::facts::TypecheckIndexProjection::Declared,
    };
    plan.requirement_span = None;
    plan.method = selected.method;
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

pub(crate) fn specialize_index_plan_across_resolvers<'a>(
    plan: super::facts::TypecheckIndexPlan,
    resolvers: impl IntoIterator<Item = &'a ResolveOutput>,
) -> Option<super::facts::TypecheckIndexPlan> {
    let rank = |plan: &super::facts::TypecheckIndexPlan| match (
        plan.conversion.is_some(),
        plan.projection,
    ) {
        (false, super::facts::TypecheckIndexProjection::Array)
        | (false, super::facts::TypecheckIndexProjection::Slice)
        | (false, super::facts::TypecheckIndexProjection::Str) => 0,
        (false, super::facts::TypecheckIndexProjection::Declared) => 1,
        (true, _) => 2,
        (false, super::facts::TypecheckIndexProjection::Requirement) => 3,
    };
    resolvers
        .into_iter()
        .filter_map(|resolved| specialize_index_plan(plan.clone(), resolved))
        .min_by_key(rank)
}
