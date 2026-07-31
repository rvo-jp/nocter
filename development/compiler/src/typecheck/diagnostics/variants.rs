use super::{
    ByteSpan, CallExpr, Diagnostic, DiagnosticNote, EnumVariantSignature, Expr, MemberExpr,
    SourceMap, Type, TypeSymbol,
};

pub(in crate::typecheck) fn non_copy_payload_binding_target_diagnostic(
    sources: &SourceMap,
    target: &Expr,
    binding_name: &str,
    payload_type: &Type,
    source_name: &str,
    help: String,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0438",
        format!(
            "payload binding `{binding_name}` has move-only type `{}` and cannot be extracted from `{source_name}` without consuming the matched enum",
            payload_type.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(target.span()).ok().map(Box::new);
    diagnostic.help = Some(help);
    diagnostic
}

pub(in crate::typecheck) fn enum_variant_unknown_diagnostic(
    sources: &SourceMap,
    member: &MemberExpr,
    enum_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0366",
        format!(
            "enum `{}` has no variant `{}`",
            enum_symbol.canonical_name, member.member
        ),
    );
    diagnostic.primary_span = sources.span_to_json(member.member_span).ok().map(Box::new);
    diagnostic.help = Some("use a variant declared by the enum".to_string());
    diagnostic
}

pub(in crate::typecheck) fn enum_variant_payload_count_mismatch_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    enum_symbol: &TypeSymbol,
    variant: &EnumVariantSignature,
    expected: usize,
    actual: usize,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0367",
        format!(
            "`{}.{}` expects {} payload value(s), but construction provides {}",
            enum_symbol.canonical_name, variant.name, expected, actual
        ),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(variant.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "variant is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("construct the variant with the payload values declared by the enum".to_string());
    diagnostic
}

pub(in crate::typecheck) fn enum_variant_payloadless_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    enum_symbol: &TypeSymbol,
    variant: &EnumVariantSignature,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0367",
        format!(
            "`{}.{}` has no payload and must be constructed without `()`",
            enum_symbol.canonical_name, variant.name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(call.arguments_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(variant.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "payloadless variant is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "write `{}.{}` instead",
        enum_symbol.canonical_name, variant.name
    ));
    diagnostic
}

pub(in crate::typecheck) fn enum_variant_payload_type_mismatch_diagnostic(
    sources: &SourceMap,
    argument: &Expr,
    enum_symbol: &TypeSymbol,
    variant: &EnumVariantSignature,
    index: usize,
    expected: &Type,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0368",
        format!(
            "`{}.{}` payload {} has type `{}`, but the variant expects `{}`",
            enum_symbol.canonical_name,
            variant.name,
            index + 1,
            actual.display(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(argument.span()).ok().map(Box::new);
    if let Some(parameter) = variant.payload.get(index)
        && let Ok(span) = sources.span_to_json(parameter.ty.span())
    {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("payload `{}` is declared here", parameter.name),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "pass a payload value of type `{}`",
        expected.display()
    ));
    diagnostic
}
