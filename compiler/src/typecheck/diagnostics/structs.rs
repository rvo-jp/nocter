use super::{
    ByteSpan, Diagnostic, DiagnosticNote, ResolveOutput, SourceMap, StructFieldSignature,
    StructLiteralExpr, StructLiteralField, Type, TypeSymbol, type_symbol_kind_name,
};

pub(in crate::typecheck) fn struct_literal_target_type_mismatch_diagnostic(
    sources: &SourceMap,
    literal: &StructLiteralExpr,
    actual: &Type,
    resolved: &ResolveOutput,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0372",
        format!(
            "struct literal target has type `{}`, but struct literals require a struct type",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(literal.ty.span()).ok().map(Box::new);
    if let Type::Named(name) = actual
        && let Some(symbol) = resolved.type_symbol_by_canonical_name(name)
    {
        diagnostic.help = Some(format!(
            "`{}` is a {}; use a struct type in the literal",
            symbol.canonical_name,
            type_symbol_kind_name(symbol.kind)
        ));
    } else {
        diagnostic.help = Some("use a struct type before `{ ... }`".to_string());
    }
    diagnostic
}

pub(in crate::typecheck) fn struct_literal_unknown_field_diagnostic(
    sources: &SourceMap,
    field: &StructLiteralField,
    struct_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0373",
        format!(
            "struct `{}` has no field `{}`",
            struct_symbol.canonical_name, field.name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(field.name_span).ok().map(Box::new);
    diagnostic.help = Some("initialize a field declared by the struct".to_string());
    diagnostic
}

pub(in crate::typecheck) fn struct_literal_duplicate_field_diagnostic(
    sources: &SourceMap,
    field: &StructLiteralField,
    first: &StructLiteralField,
    struct_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0374",
        format!(
            "struct `{}` field `{}` is initialized more than once",
            struct_symbol.canonical_name, field.name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(field.name_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first initialization is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("initialize each struct field exactly once".to_string());
    diagnostic
}

pub(in crate::typecheck) fn struct_literal_missing_field_diagnostic(
    sources: &SourceMap,
    literal: &StructLiteralExpr,
    struct_symbol: &TypeSymbol,
    field: &StructFieldSignature,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0375",
        format!(
            "struct `{}` literal does not initialize field `{}`",
            struct_symbol.canonical_name, field.name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(literal.fields_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(field.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("field `{}` is declared here", field.name),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!("add `{}` to the struct literal", field.name));
    diagnostic
}

pub(in crate::typecheck) fn struct_literal_field_type_mismatch_diagnostic(
    sources: &SourceMap,
    field: &StructLiteralField,
    expected_field: &StructFieldSignature,
    expected: &Type,
    actual: &Type,
    struct_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0376",
        format!(
            "struct `{}` field `{}` is initialized with `{}`, but the field expects `{}`",
            struct_symbol.canonical_name,
            field.name,
            actual.display(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(field.value.span()).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(expected_field.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("field `{}` is declared here", expected_field.name),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "initialize `{}` with a value of type `{}`",
        field.name,
        expected.display()
    ));
    diagnostic
}

pub(in crate::typecheck) fn struct_literal_inaccessible_field_diagnostic(
    sources: &SourceMap,
    field_span: ByteSpan,
    struct_symbol: &TypeSymbol,
    field: &StructFieldSignature,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0377",
        format!(
            "field `{}` of struct `{}` is not visible here",
            field.name, struct_symbol.canonical_name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(field_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(field.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("field `{}` is declared here", field.name),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("construct this value through a visible API from its defining module".to_string());
    diagnostic
}

pub(in crate::typecheck) fn struct_literal_inaccessible_missing_field_diagnostic(
    sources: &SourceMap,
    literal: &StructLiteralExpr,
    struct_symbol: &TypeSymbol,
    field: &StructFieldSignature,
) -> Diagnostic {
    let mut diagnostic = struct_literal_inaccessible_field_diagnostic(
        sources,
        literal.ty.span(),
        struct_symbol,
        field,
    );
    diagnostic.message = format!(
        "struct `{}` literal cannot initialize hidden field `{}`",
        struct_symbol.canonical_name, field.name
    );
    diagnostic
}
