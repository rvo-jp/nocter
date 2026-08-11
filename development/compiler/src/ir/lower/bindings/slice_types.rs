use super::*;

pub(super) fn slice_index_element_type_expr(
    index: &IndexExpr,
    context: &LoweringContext,
) -> Option<TypeExpr> {
    if let Some(plan) = context.index_plan(index.span) {
        return Some(plan.element_ty);
    }
    slice_target_element_type_expr(&index.object, context)
}

pub(super) fn slice_target_element_type_expr(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<TypeExpr> {
    if let Some(ty) = context.expression_type_expr(expression.span())
        && let Some(element) = slice_element_type_expr_from_type_expr(&ty, context)
    {
        return Some(element);
    }

    match unwrap_group(expression) {
        Expr::Identifier(identifier) => context.slice_element_type_expr(&identifier.name).cloned(),
        Expr::Member(member) => match aggregate_member_field_kind_from_member(member, context)
            .ok()
            .flatten()
        {
            Some(AggregateFieldKind::Slice(info)) => info.element_type,
            _ => None,
        },
        Expr::Call(call) => {
            let return_type = context.call_return_type_expr(call)?;
            slice_element_type_expr_from_type_expr(&return_type, context)
        }
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return None;
            };
            let TypeExpr::Fallible(fallible) = context.call_return_type_expr(call)? else {
                return None;
            };
            slice_element_type_expr_from_type_expr(&fallible.success, context)
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return None;
            };
            let TypeExpr::Fallible(fallible) = context.call_return_type_expr(call)? else {
                return None;
            };
            slice_element_type_expr_from_type_expr(&fallible.success, context)
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return None;
            };
            let TypeExpr::Fallible(fallible) = context.call_return_type_expr(call)? else {
                return None;
            };
            slice_element_type_expr_from_type_expr(&fallible.success, context)
        }
        Expr::Group(group) => slice_target_element_type_expr(&group.expression, context),
        _ => None,
    }
}

pub(super) fn slice_element_type_expr_from_type_expr(
    ty: &TypeExpr,
    context: &LoweringContext,
) -> Option<TypeExpr> {
    let (_root_source, resolved) = context.resolved_calls()?;
    slice_element_type_expr_from_type_expr_with_resolved(ty, resolved, context)
}

pub(super) fn slice_element_type_expr_from_type_expr_with_resolved(
    ty: &TypeExpr,
    resolved: &crate::resolve::ResolveOutput,
    context: &LoweringContext,
) -> Option<TypeExpr> {
    match ty {
        TypeExpr::Borrow(borrow) => {
            let TypeExpr::View(view) = borrow.inner.as_ref() else {
                return None;
            };
            Some(*view.element.clone())
        }
        TypeExpr::Reference(reference) => {
            let symbol = resolved.type_symbol_by_reference_name(&reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            let target_resolved = context
                .resolved_source(target.span().source)
                .unwrap_or(resolved);
            slice_element_type_expr_from_type_expr_with_resolved(target, target_resolved, context)
        }
        _ => None,
    }
}

pub(super) fn slice_type_info_from_type_expr(
    ty: &TypeExpr,
    context: &LoweringContext,
) -> SliceTypeInfo {
    let element_type = slice_element_type_expr_from_type_expr(ty, context);
    let element_kind = element_type
        .as_ref()
        .map(|element_type| slice_element_kind_from_element_type_expr(element_type, context))
        .unwrap_or_else(|| slice_element_kind_from_type_expr(ty, context));
    SliceTypeInfo {
        element_kind,
        element_type,
    }
}

pub(super) fn slice_type_info_from_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<SliceTypeInfo> {
    let element_type = slice_target_element_type_expr(expression, context)?;
    Some(SliceTypeInfo {
        element_kind: slice_element_kind_from_element_type_expr(&element_type, context),
        element_type: Some(element_type),
    })
}

pub(super) fn slice_type_info_from_call_return(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<SliceTypeInfo> {
    let return_type = context.call_return_type_expr(call)?;
    Some(slice_type_info_from_type_expr(&return_type, context))
}

pub(super) fn slice_type_info_from_call_success(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<SliceTypeInfo> {
    let TypeExpr::Fallible(fallible) = context.call_return_type_expr(call)? else {
        return None;
    };
    Some(slice_type_info_from_type_expr(&fallible.success, context))
}

pub(super) fn slice_type_info_from_kind(element_kind: TypecheckSliceElementKind) -> SliceTypeInfo {
    SliceTypeInfo {
        element_kind,
        element_type: None,
    }
}

pub(super) fn slice_element_kind_from_type_expr(
    ty: &TypeExpr,
    context: &LoweringContext,
) -> TypecheckSliceElementKind {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return TypecheckSliceElementKind::Other;
    };
    slice_element_kind_from_type(view_element_type_from_type_expr(ty, resolved))
}

pub(super) fn slice_element_kind_from_element_type_expr(
    ty: &TypeExpr,
    context: &LoweringContext,
) -> TypecheckSliceElementKind {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return TypecheckSliceElementKind::Other;
    };
    slice_element_kind_from_type(scalar_or_view_type_from_type_expr(ty, resolved))
}

pub(super) fn call_return_slice_element_kind(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<TypecheckSliceElementKind> {
    let (_root_source, resolved) = context.resolved_calls()?;
    let return_type = context.call_return_type_expr(call)?;
    Some(slice_element_kind_from_type(
        view_element_type_from_type_expr(&return_type, resolved),
    ))
}

pub(super) fn call_success_slice_element_kind(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<TypecheckSliceElementKind> {
    let (_root_source, resolved) = context.resolved_calls()?;
    let return_type = context.call_return_type_expr(call)?;
    let TypeExpr::Fallible(fallible) = return_type else {
        return None;
    };
    Some(slice_element_kind_from_type(
        view_element_type_from_type_expr(&fallible.success, resolved),
    ))
}

pub(super) fn slice_element_kind_from_type(ty: Option<Type>) -> TypecheckSliceElementKind {
    match ty {
        Some(Type::I32) => TypecheckSliceElementKind::I32,
        Some(Type::U8) => TypecheckSliceElementKind::U8,
        Some(Type::Usize) => TypecheckSliceElementKind::Usize,
        Some(Type::Integer(kind)) => TypecheckSliceElementKind::Integer(kind),
        Some(Type::Bool) => TypecheckSliceElementKind::Bool,
        Some(Type::Str) => TypecheckSliceElementKind::Str,
        _ => TypecheckSliceElementKind::Other,
    }
}
