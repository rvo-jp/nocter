use super::*;

pub(in crate::driver::buildability) fn unwrap_group_expr(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group_expr(&group.expression),
        _ => expression,
    }
}

pub(in crate::driver::buildability) fn unsupported_native_build_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    construct: &str,
    help: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0435",
        format!("the native compiler cannot lower {construct} yet"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(help.to_string());
    diagnostic
}

pub(in crate::driver::buildability) fn unsupported_payload_binding_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    control: &str,
) -> Diagnostic {
    unsupported_native_build_diagnostic(
        sources,
        span,
        &format!(
            "payload bindings outside runtime scalar/view, copy aggregate, and owned recursively droppable aggregate types in {control}"
        ),
        "bind an `i32`, `u8`, `usize`, `bool`, `&str`, slice view, or copy aggregate payload; move an owned aggregate with runtime-supported recursive drop glue; or use `_` to discard other payloads",
    )
}

pub(in crate::driver::buildability) fn call_target_for_source(
    source: SourceId,
    root_source: SourceId,
    name: String,
) -> CallTarget {
    if source == root_source {
        CallTarget::same_file(name)
    } else {
        CallTarget::imported(source, name)
    }
}

pub(in crate::driver::buildability) fn span_contains(outer: ByteSpan, inner: ByteSpan) -> bool {
    outer.source == inner.source && outer.start <= inner.start && inner.end <= outer.end
}

pub(in crate::driver::buildability) fn method_target_name(
    type_name: &str,
    method_name: &str,
) -> String {
    format!("{type_name}.{method_name}")
}

pub(in crate::driver::buildability) fn drop_target_name(self_ty: &TypeExpr) -> String {
    format!("{}.drop", canonical_type_expr(self_ty))
}

pub(in crate::driver::buildability) fn unsupported_outcome_return_issue(
    function: &FunctionDecl,
    substitutions: &HashMap<String, TypeExpr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<BuildabilityIssue> {
    let return_type = substitute_type_expr_parameters(&function.return_type, substitutions);
    unsupported_outcome_return_type_issue(
        &return_type,
        function.return_type.span(),
        resolved,
        resolved_sources,
    )
}

pub(in crate::driver::buildability) fn unsupported_outcome_return_type_issue(
    return_type: &TypeExpr,
    diagnostic_span: ByteSpan,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<BuildabilityIssue> {
    let shape = outcome_shape_with_resolver(return_type, resolved, |source| {
        resolved_sources.get(&source).copied()
    });
    if shape.is_supported_callable_shape() {
        return None;
    }

    Some(BuildabilityIssue {
        span: diagnostic_span,
        construct: "unsupported recursive optional or fallible return shapes",
        help: "use a value, one optional or fallible layer, or one explicitly composed optional and fallible layer",
    })
}

pub(in crate::driver::buildability) fn impl_target_type_name(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Generic(generic) => Some(&generic.name),
        _ => None,
    }
}
