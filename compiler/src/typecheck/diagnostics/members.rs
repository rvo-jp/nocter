use super::{
    Diagnostic, DiagnosticNote, MemberExpr, MethodSignature, SourceMap, Type, TypeSymbol,
    type_expr_display_lossy,
};

pub(in crate::typecheck) fn error_member_unknown_diagnostic(
    sources: &SourceMap,
    member: &MemberExpr,
) -> Diagnostic {
    let mut diagnostic =
        Diagnostic::error("E0369", format!("`error` has no field `{}`", member.member));
    diagnostic.primary_span = sources.span_to_json(member.member_span).ok().map(Box::new);
    diagnostic.help = Some("use `error.code` or `error.message`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn struct_field_unknown_diagnostic(
    sources: &SourceMap,
    member: &MemberExpr,
    struct_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0370",
        format!(
            "struct `{}` has no field `{}`",
            struct_symbol.canonical_name, member.member
        ),
    );
    diagnostic.primary_span = sources.span_to_json(member.member_span).ok().map(Box::new);
    diagnostic.help = Some("use a field declared by the struct".to_string());
    diagnostic
}

pub(in crate::typecheck) fn method_receiver_unsupported_diagnostic(
    sources: &SourceMap,
    member: &MemberExpr,
    owner: &TypeSymbol,
    method: &MethodSignature,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0377",
        format!(
            "method `{}.{}` uses unsupported receiver type `{}`",
            owner.canonical_name,
            method.name,
            type_expr_display_lossy(&method.receiver.ty)
        ),
    );
    diagnostic.primary_span = sources.span_to_json(member.member_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(method.receiver.ty.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: "receiver type is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("v0 method calls require receiver type `Self`, `&Self`, or `&+Self`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn method_readwrite_receiver_requires_var_diagnostic(
    sources: &SourceMap,
    member: &MemberExpr,
    owner: &TypeSymbol,
    method: &MethodSignature,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0378",
        format!(
            "method `{}.{}` requires a mutable `var` receiver",
            owner.canonical_name, method.name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(member.object.span())
        .ok()
        .map(Box::new);
    if let Ok(span) = sources.span_to_json(method.receiver.ty.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: "readwrite receiver is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("bind the receiver with `var` before calling this method".to_string());
    diagnostic
}

pub(in crate::typecheck) fn member_target_type_mismatch_diagnostic(
    sources: &SourceMap,
    member: &MemberExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0371",
        format!(
            "field access target has type `{}`, but fields require a struct value",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(member.object.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some("access fields on a struct value".to_string());
    diagnostic
}
