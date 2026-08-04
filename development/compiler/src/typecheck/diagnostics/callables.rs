use super::*;

pub(in crate::typecheck) fn callable_readwrite_requires_writable_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0454",
        "mutable callable invocation requires a writable callable place",
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "bind the callable with `var`, or invoke it through a writable aggregate field".to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn multiple_callable_bounds_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    first_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0455",
        "a generic parameter cannot declare multiple callable contracts",
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "the first callable contract is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("keep one unambiguous callable signature for the parameter".to_string());
    diagnostic
}

pub(in crate::typecheck) fn duplicate_callable_parameter_name_diagnostic(
    sources: &SourceMap,
    name: &str,
    span: ByteSpan,
    first_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0456",
        format!("callable contract repeats parameter name `{name}`"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "the first parameter is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("give each named callable parameter a unique name".to_string());
    diagnostic
}

pub(in crate::typecheck) fn invalid_callable_provenance_origin_diagnostic(
    sources: &SourceMap,
    origin: &crate::ast::ResultProvenanceOrigin,
) -> Diagnostic {
    let label = origin.kind.source_label();
    let mut diagnostic = Diagnostic::error(
        "E0457",
        format!("callable result provenance cannot originate from `{label}`"),
    );
    diagnostic.primary_span = sources.span_to_json(origin.span).ok().map(Box::new);
    diagnostic.help =
        Some("name a declared callable parameter, `static`, or `current` after `from`".to_string());
    diagnostic
}
