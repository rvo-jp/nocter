use super::{ByteSpan, Diagnostic, DiagnosticNote, SourceMap, Type};

pub(in crate::typecheck) fn uninitialized_binding_diagnostic(
    sources: &SourceMap,
    name: &str,
    use_span: ByteSpan,
    action: &str,
    previous_action: &str,
    previous_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0385",
        format!("cannot {action} `{name}` because it was {previous_action}"),
    );
    diagnostic.primary_span = sources.span_to_json(use_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(previous_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("`{name}` was {previous_action} here"),
            span: Some(span),
        });
    }
    diagnostic.help = Some(
        "move or drop each owned value at most once, or reinitialize a `var` binding before using it again"
            .to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn invalid_drop_target_diagnostic(
    sources: &SourceMap,
    name: &str,
    name_span: ByteSpan,
    ty: Option<&Type>,
) -> Diagnostic {
    let message = match ty {
        Some(ty) => format!(
            "cannot explicitly drop `{name}` with type `{}` because it is not a move-only owned binding",
            ty.display()
        ),
        None => format!("cannot explicitly drop unknown binding `{name}`"),
    };
    let mut diagnostic = Diagnostic::error("E0386", message);
    diagnostic.primary_span = sources.span_to_json(name_span).ok().map(Box::new);
    diagnostic.help = Some(
        "`drop name` accepts initialized non-copy struct bindings; copy values and borrowed values are not explicitly dropped in v0"
            .to_string(),
    );
    diagnostic
}
