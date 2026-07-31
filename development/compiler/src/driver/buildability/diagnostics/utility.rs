use super::*;

pub(in crate::driver::buildability) fn unwrap_group_expr(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group_expr(&group.expression),
        _ => expression,
    }
}

pub(in crate::driver::buildability) fn unsupported_v0_build_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    construct: &str,
    help: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0435",
        format!("Nocter v0 build cannot lower {construct} yet"),
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
    unsupported_v0_build_diagnostic(
        sources,
        span,
        &format!(
            "payload bindings outside runtime scalar/view and copy aggregate types in {control}"
        ),
        "bind an `i32`, `u8`, `usize`, `bool`, `&str`, slice view, or copy aggregate payload; use `_` to discard other payloads until ownership-aware payload extraction is promoted",
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
    format!("{}.drop", type_expr_display_lossy(self_ty))
}

pub(in crate::driver::buildability) fn nested_fallible_return_issue(
    function: &FunctionDecl,
    substitutions: &HashMap<String, TypeExpr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<BuildabilityIssue> {
    let return_type = substitute_type_expr_parameters(&function.return_type, substitutions);
    nested_fallible_return_type_issue(
        &return_type,
        function.return_type.span(),
        resolved,
        resolved_sources,
    )
}

pub(in crate::driver::buildability) fn nested_fallible_return_type_issue(
    return_type: &TypeExpr,
    diagnostic_span: ByteSpan,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<BuildabilityIssue> {
    if type_expr_fallible_depth(return_type, resolved, resolved_sources) <= 1 {
        return None;
    }

    Some(BuildabilityIssue {
        span: diagnostic_span,
        construct: "nested fallible or optional return types",
        help: "flatten the return boundary to a single optional or fallible layer until nested fallible lowering is promoted",
    })
}

pub(in crate::driver::buildability) fn impl_target_type_name(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Generic(generic) => Some(&generic.name),
        _ => None,
    }
}

pub(in crate::driver::buildability) fn drop_name_span(span: ByteSpan) -> ByteSpan {
    ByteSpan::new(span.source, span.start, span.start + "drop".len())
}
