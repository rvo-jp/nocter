use super::{Diagnostic, DiagnosticNote, SourceMap, Type};
use crate::ast::TypeExpr;
use crate::resolve::{MethodSignature, TypeSymbol};
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

pub(in crate::typecheck) fn duplicate_generic_bound_diagnostic(
    sources: &SourceMap,
    bound: &TypeExpr,
    actual: &Type,
    first_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0450",
        format!(
            "generic parameter repeats interface bound `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(bound.span()).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "the same specialized interface bound is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("remove the duplicate interface bound".to_string());
    diagnostic
}

pub(in crate::typecheck) fn ambiguous_generic_bound_method_diagnostic(
    sources: &SourceMap,
    member_span: ByteSpan,
    member_name: &str,
    candidates: &[(&TypeSymbol, &MethodSignature)],
) -> Diagnostic {
    let interfaces = candidates
        .iter()
        .map(|(owner, _)| owner.canonical_name.as_str())
        .collect::<Vec<_>>()
        .join("`, `");
    let mut diagnostic = Diagnostic::error(
        "E0449",
        format!(
            "generic method `{member_name}` is ambiguous between interface bounds `{interfaces}`"
        ),
    );
    diagnostic.primary_span = sources.span_to_json(member_span).ok().map(Box::new);
    for (owner, method) in candidates {
        if let Ok(span) = sources.span_to_json(method.name_span) {
            diagnostic.notes.push(DiagnosticNote {
                message: format!("candidate declared by interface `{}`", owner.canonical_name),
                span: Some(span),
            });
        }
    }
    diagnostic.help = Some(
        "use interface bounds whose callable member names do not overlap; Nocter does not choose by bound order"
            .to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn ambiguous_default_method_diagnostic(
    sources: &SourceMap,
    member_span: ByteSpan,
    member_name: &str,
    candidates: &[(&TypeSymbol, &MethodSignature)],
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0451",
        format!("default method `{member_name}` is provided by multiple interfaces"),
    );
    diagnostic.primary_span = sources.span_to_json(member_span).ok().map(Box::new);
    for (owner, method) in candidates {
        if let Ok(span) = sources.span_to_json(method.name_span) {
            diagnostic.notes.push(DiagnosticNote {
                message: format!("default declared by interface `{}`", owner.canonical_name),
                span: Some(span),
            });
        }
    }
    diagnostic.help = Some(
        "define one compatible inherent method on the receiver type to select behavior explicitly"
            .to_string(),
    );
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
