use super::{
    CallExpr, CheckedCallSignature, Diagnostic, DiagnosticNote, Expr, NonCopyOwnedValueKind,
    ParameterSignature, SourceMap, Type,
};

pub(in crate::typecheck) fn argument_count_mismatch_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    signature: &CheckedCallSignature<'_>,
    expected: usize,
    actual: usize,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0320",
        format!(
            "{} `{}` expects {expected} argument(s), but call provides {actual}",
            signature.kind.noun(),
            signature.name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(call.arguments_span).ok().map(Box::new);
    if let Some(declaration_span) = signature.declaration_span
        && let Ok(span) = sources.span_to_json(declaration_span)
    {
        diagnostic.notes.push(DiagnosticNote {
            message: format!(
                "{} `{}` is declared here",
                signature.kind.noun(),
                signature.name
            ),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "pass exactly the parameters declared by the {}",
        signature.kind.noun()
    ));
    diagnostic
}

pub(in crate::typecheck) fn argument_type_mismatch_diagnostic(
    sources: &SourceMap,
    index: usize,
    argument: &Expr,
    parameter: &ParameterSignature,
    expected: &Type,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0321",
        format!(
            "argument {} has type `{}`, but parameter `{}` expects `{}`",
            index + 1,
            actual.display(),
            parameter.name,
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(argument.span()).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(parameter.ty.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("parameter `{}` is declared here", parameter.name),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!("pass a value of type `{}`", expected.display()));
    diagnostic
}

pub(in crate::typecheck) fn non_copy_struct_argument_diagnostic(
    sources: &SourceMap,
    index: usize,
    argument: &Expr,
    parameter: &ParameterSignature,
    source_name: &str,
    type_name: &str,
    kind: NonCopyOwnedValueKind,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0392",
        format!(
            "cannot implicitly copy {} `{type_name}` from `{source_name}` into argument {}",
            kind.noun(),
            index + 1
        ),
    );
    diagnostic.primary_span = sources.span_to_json(argument.span()).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(parameter.ty.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!(
                "parameter `{}` takes `{type_name}` by value",
                parameter.name
            ),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!("write `move {source_name}` to transfer ownership"));
    diagnostic
}
