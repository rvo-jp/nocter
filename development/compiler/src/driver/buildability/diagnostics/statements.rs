use super::*;

pub(in crate::driver::buildability) fn unsupported_expression_statement_diagnostic(
    sources: &SourceMap,
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if expression_statement_is_supported(
        expression,
        resolved,
        resolved_sources,
        typed_hir,
        generic_substitutions,
    ) {
        return None;
    }

    Some(unsupported_native_build_diagnostic(
        sources,
        expression.span(),
        "value-producing expression statements",
        "call a void, never, or discardable scalar/view/aggregate function, handle a discardable scalar/view/aggregate fallible call with `?`, `!`, or `catch`, or bind/return the value explicitly",
    ))
}

pub(in crate::driver::buildability) fn otherwise_optional_value_call_is_buildable(
    value: &Expr,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let stored = match unwrap_group_expr(value) {
        Expr::Identifier(identifier) => Some((
            identifier.span,
            &[crate::outcomes::OutcomeLayer::Optional][..],
            true,
        )),
        Expr::Propagate(propagation) => match unwrap_group_expr(&propagation.expression) {
            Expr::Identifier(identifier) => Some((
                identifier.span,
                &[
                    crate::outcomes::OutcomeLayer::Fallible,
                    crate::outcomes::OutcomeLayer::Optional,
                ][..],
                false,
            )),
            _ => None,
        },
        Expr::Catch(catch) => match unwrap_group_expr(&catch.expression) {
            Expr::Identifier(identifier) => Some((
                identifier.span,
                &[
                    crate::outcomes::OutcomeLayer::Fallible,
                    crate::outcomes::OutcomeLayer::Optional,
                ][..],
                false,
            )),
            _ => None,
        },
        _ => None,
    };
    if let Some((span, expected_layers, allow_trailing)) = stored
        && let Some(ty) = typed_hir.expression_type_expr(span)
    {
        let ty = substitute_type_expr_parameters(ty, generic_substitutions);
        let shape = outcome_shape_with_resolver(&ty, resolved, |source| {
            resolved_sources.get(&source).copied()
        });
        return shape.is_supported_callable_shape()
            && shape.layers.starts_with(expected_layers)
            && (allow_trailing || shape.layers.len() == expected_layers.len());
    }
    let (call, expected_layers) = match unwrap_group_expr(value) {
        Expr::Call(call) => (call, &[crate::outcomes::OutcomeLayer::Optional][..]),
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group_expr(&propagation.expression) else {
                return false;
            };
            (
                call,
                &[
                    crate::outcomes::OutcomeLayer::Fallible,
                    crate::outcomes::OutcomeLayer::Optional,
                ][..],
            )
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group_expr(&catch.expression) else {
                return false;
            };
            (
                call,
                &[
                    crate::outcomes::OutcomeLayer::Fallible,
                    crate::outcomes::OutcomeLayer::Optional,
                ][..],
            )
        }
        _ => return false,
    };
    let Some(return_type) =
        call_return_type_expr_with_substitutions(call, resolved, typed_hir, generic_substitutions)
    else {
        return false;
    };
    let source_resolver = |source| resolved_sources.get(&source).copied();
    outcome_shape_with_resolver(&return_type, resolved, source_resolver).layers == expected_layers
}

pub(in crate::driver::buildability) fn expression_is_never_runtime_shape_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => matches!(
            call_return_shape(call, resolved, typed_hir, generic_substitutions),
            Some(ReturnShape::Never)
        ),
        _ => false,
    }
}

pub(in crate::driver::buildability) fn aggregate_literal_statement_is_supported(
    literal: &crate::ast::StructLiteralExpr,
    resolved: &ResolveOutput,
) -> bool {
    abi_value_from_type_expr(&literal.ty, resolved)
        .map(|value| matches!(value.ty, AbiType::Struct(_)))
        .unwrap_or(false)
}

pub(in crate::driver::buildability) fn unsupported_index_assignment_target_diagnostic(
    sources: &SourceMap,
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if statement.operator != AssignmentOperator::Assign {
        return None;
    }
    let Expr::Index(index) = unwrap_group_expr(&statement.target) else {
        return None;
    };
    if let Some(is_buildable) = fixed_array_index_assignment_target_is_buildable(
        index,
        resolved,
        resolved_sources,
        typed_hir,
        generic_substitutions,
    ) {
        if is_buildable {
            return None;
        }
        return Some(unsupported_native_build_diagnostic(
            sources,
            index.span,
            "fixed array index assignment targets outside scalar/view element locals or aggregate fields",
            "assign through an index into a local or aggregate-field fixed array with builtin integer, `bool`, or `&str` elements",
        ));
    }
    if matches!(
        slice_index_assignment_target_is_buildable(
            &index.object,
            resolved,
            resolved_sources,
            typed_hir,
            generic_substitutions,
        ),
        Some(true) | None
    ) {
        return None;
    }

    Some(unsupported_native_build_diagnostic(
        sources,
        index.object.span(),
        "index assignment targets outside supported slice values",
        "assign through a slice binding, supported slice-returning call result, or slice aggregate field until broader index assignment lowering is promoted",
    ))
}
