use super::{Diagnostic, DiagnosticNote, SourceMap, Type};
use crate::ast::TypeExpr;
use crate::source::ByteSpan;

pub(in crate::typecheck) fn generic_bound_not_interface_diagnostic(
    sources: &SourceMap,
    bound: &TypeExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0446",
        format!(
            "generic parameter bounds must name an interface, found `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(bound.span()).ok().map(Box::new);
    diagnostic.help = Some("replace the bound with an interface type".to_string());
    diagnostic
}

pub(in crate::typecheck) fn generic_bound_not_satisfied_diagnostic(
    sources: &SourceMap,
    argument_span: ByteSpan,
    actual: &Type,
    bound: &Type,
    bound_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0447",
        format!(
            "type `{}` does not implement interface `{}` required by this call",
            actual.display(),
            bound.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(argument_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(bound_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "the generic interface bound is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "add `impl {} for {}` with the required methods, or pass a conforming type",
        bound.display(),
        actual.display()
    ));
    diagnostic
}
