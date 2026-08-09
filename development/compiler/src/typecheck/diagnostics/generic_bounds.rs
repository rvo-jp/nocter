use super::{Diagnostic, DiagnosticNote, SourceMap, Type};
use crate::ast::TypeExpr;
use crate::resolve::{AssociatedTypeBindingSignature, MethodSignature, TypeSymbol};
use crate::source::ByteSpan;

pub(in crate::typecheck) fn generic_bound_not_interface_diagnostic(
    sources: &SourceMap,
    bound: &TypeExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0446",
        format!(
            "generic parameter bounds must name an interface or callable contract, found `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(bound.span()).ok().map(Box::new);
    diagnostic.help =
        Some("replace the bound with an interface type or built-in callable contract".to_string());
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
        format!("generic parameter repeats bound `{}`", actual.display()),
    );
    diagnostic.primary_span = sources.span_to_json(bound.span()).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "the same specialized interface bound is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("remove the duplicate bound".to_string());
    diagnostic
}

pub(in crate::typecheck) fn unknown_where_parameter_diagnostic(
    sources: &SourceMap,
    name: &str,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0452",
        format!("where clause names unknown generic parameter `{name}`"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "declare the parameter in the callable or its enclosing type, or remove the requirement"
            .to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn duplicate_copy_requirement_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    first_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error("E0453", "generic parameter repeats `copy`");
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "the first `copy` requirement is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("remove the duplicate `copy` requirement".to_string());
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

pub(in crate::typecheck) fn ambiguous_concrete_method_diagnostic(
    sources: &SourceMap,
    member_span: ByteSpan,
    member_name: &str,
    candidates: &[(&TypeSymbol, &MethodSignature)],
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0451",
        format!("method `{member_name}` is ambiguous for this concrete receiver"),
    );
    diagnostic.primary_span = sources.span_to_json(member_span).ok().map(Box::new);
    for (owner, method) in candidates {
        if let Ok(span) = sources.span_to_json(method.name_span) {
            diagnostic.notes.push(DiagnosticNote {
                message: format!("candidate declared by `{}`", owner.canonical_name),
                span: Some(span),
            });
        }
    }
    diagnostic.help = Some(
        "use non-overlapping inherent and interface member names; qualified method calls are not yet available"
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

pub(in crate::typecheck) fn copy_requirement_not_satisfied_diagnostic(
    sources: &SourceMap,
    argument_span: ByteSpan,
    actual: &Type,
    requirement_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0458",
        format!(
            "type `{}` is not copyable but this call requires `copy`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(argument_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(requirement_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "the `copy` requirement is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some(
        "pass a copyable type, or use an operation that transfers or borrows the value".to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn invalid_type_equality_diagnostic(
    sources: &SourceMap,
    left: &TypeExpr,
    right: &TypeExpr,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0466",
        "a type equality must constrain at least one associated type projection",
    );
    diagnostic.primary_span = sources
        .span_to_json(ByteSpan::new(
            left.span().source,
            left.span().start,
            right.span().end,
        ))
        .ok()
        .map(Box::new);
    diagnostic.help = Some(
        "write an equality such as `R.Item = L.Item`; ordinary generic identity is already inferred"
            .to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn duplicate_type_equality_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    first_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error("E0467", "where clause repeats a type equality");
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "the equivalent equality is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("remove the duplicate equality".to_string());
    diagnostic
}

pub(in crate::typecheck) fn associated_type_bound_not_satisfied_diagnostic(
    sources: &SourceMap,
    interface: &TypeSymbol,
    binding: &AssociatedTypeBindingSignature,
    actual: &Type,
    bound: &Type,
    bound_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0468",
        format!(
            "associated type `{}.{}` is `{}`, which does not satisfy `{}`",
            interface.canonical_name,
            binding.name,
            actual.display(),
            bound.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(binding.declaration_span)
        .ok()
        .map(Box::new);
    if let Ok(span) = sources.span_to_json(bound_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "the associated type bound is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some(
        "bind the associated type to a type that implements the required interface".to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn type_equality_not_satisfied_diagnostic(
    sources: &SourceMap,
    call_span: ByteSpan,
    left: &Type,
    right: &Type,
    equality_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0469",
        format!(
            "call requires `{}` and `{}` to be the same type",
            left.display(),
            right.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(call_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(equality_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "the type equality is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("use arguments whose associated types satisfy the equality".to_string());
    diagnostic
}
