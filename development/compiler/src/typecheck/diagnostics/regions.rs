use super::{Diagnostic, DiagnosticNote, Expr, ReturnContext, SourceMap};
use crate::source::ByteSpan;

pub(in crate::typecheck) fn region_allocator_not_place_diagnostic(
    sources: &SourceMap,
    allocator_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0438",
        "a region parent must be an established allocator place",
    );
    diagnostic.primary_span = sources.span_to_json(allocator_span).ok().map(Box::new);
    diagnostic.help = Some(
        "bind the allocator/context value first, then use that binding or one of its fields"
            .to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn region_binding_escape_diagnostic(
    sources: &SourceMap,
    statement_span: ByteSpan,
    target_span: ByteSpan,
    region_name: &str,
    region_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0437",
        format!(
            "a value carrying storage from {region_name} escapes the region through an outer binding"
        ),
    );
    diagnostic.primary_span = sources.span_to_json(statement_span).ok().map(Box::new);
    for (message, span) in [
        ("the outer binding is declared here", target_span),
        ("the storage region is declared here", region_span),
    ] {
        if let Ok(span) = sources.span_to_json(span) {
            diagnostic.notes.push(DiagnosticNote {
                message: message.to_string(),
                span: Some(span),
            });
        }
    }
    diagnostic.help = Some(
        "keep the value inside the region, or assign only region-independent data to the outer binding"
            .to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn region_return_escape_diagnostic(
    sources: &SourceMap,
    expression: &Expr,
    region_name: &str,
    region_span: ByteSpan,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0436",
        format!(
            "cannot return a value carrying storage from {region_name} because the region ends before {}",
            context.subject()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(expression.span()).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(region_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "the storage region is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some(
        "consume the value inside the region, or copy region-independent data out before leaving it"
            .to_string(),
    );
    diagnostic
}
