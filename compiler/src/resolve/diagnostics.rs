use crate::ast::Visibility;
use crate::diagnostics::{Diagnostic, DiagnosticNote};
use crate::source::{ByteSpan, SourceMap};

pub(super) fn duplicate_visible_name_diagnostic(
    sources: &SourceMap,
    name: &str,
    first_span: ByteSpan,
    duplicate_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error("E0400", format!("name `{name}` is already visible"));
    diagnostic.primary_span = sources.span_to_json(duplicate_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first visible declaration is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("choose a distinct name; Nocter v0 does not allow shadowing".to_string());
    diagnostic
}

pub(super) fn builtin_name_reuse_diagnostic(
    sources: &SourceMap,
    name: &str,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0401",
        format!("built-in type name `{name}` cannot be reused as a binding"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some("choose a binding name that is not a built-in type name".to_string());
    diagnostic
}

pub(super) fn builtin_type_declaration_name_reuse_diagnostic(
    sources: &SourceMap,
    name: &str,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0401",
        format!("built-in type name `{name}` cannot be reused as a type declaration"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some("choose a type name that is not a built-in type name".to_string());
    diagnostic
}

pub(super) fn duplicate_inherent_member_name_diagnostic(
    sources: &SourceMap,
    target_name: &str,
    member_name: &str,
    first_span: ByteSpan,
    duplicate_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0413",
        format!("type `{target_name}` already has an inherent member named `{member_name}`"),
    );
    diagnostic.primary_span = sources.span_to_json(duplicate_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first inherent member with this name is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("choose a distinct member name; Nocter v0 does not support overloads".to_string());
    diagnostic
}

pub(super) fn invalid_associated_function_owner_diagnostic(
    sources: &SourceMap,
    owner_name: &str,
    owner_span: ByteSpan,
    reason: &str,
    declaration_span: Option<ByteSpan>,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0414",
        format!("associated function owner `{owner_name}` {reason}"),
    );
    diagnostic.primary_span = sources.span_to_json(owner_span).ok().map(Box::new);
    if let Some(declaration_span) = declaration_span
        && let Ok(span) = sources.span_to_json(declaration_span)
    {
        diagnostic.notes.push(DiagnosticNote {
            message: "owner name resolves here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("define associated functions for the nominal type in the same module".to_string());
    diagnostic
}

pub(super) fn unloaded_import_diagnostic(
    sources: &SourceMap,
    import_path: &str,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0411",
        format!("relative import `{import_path}` was not loaded before name resolution"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some("load relative imports before running name resolution".to_string());
    diagnostic
}

pub(super) fn missing_import_diagnostic(
    sources: &SourceMap,
    name: &str,
    import_path: &str,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0411",
        format!("import `{import_path}` does not export `{name}` in v0"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "define a public top-level `func`, `primitive`, `type`, `struct`, or `enum` with that name in the imported file"
            .to_string(),
    );
    diagnostic
}

pub(super) fn restricted_import_diagnostic(
    sources: &SourceMap,
    name: &str,
    import_path: &str,
    visibility: Visibility,
    import_span: ByteSpan,
    declaration_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0412",
        format!(
            "import `{import_path}` cannot access {visibility} name `{name}`",
            visibility = visibility_description(visibility),
        ),
    );
    diagnostic.primary_span = sources.span_to_json(import_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(declaration_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "name is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some(
        match visibility {
            Visibility::Private => "mark the declaration `pub` if it is part of the module API",
            Visibility::Nocter => {
                "`pub(nocter)` names are importable only from files inside the active Nocter home"
            }
            Visibility::Public => {
                "public names should be importable; this diagnostic is unexpected"
            }
        }
        .to_string(),
    );
    diagnostic
}

pub(super) fn unresolved_identifier_diagnostic(
    sources: &SourceMap,
    name: &str,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0416",
        format!("identifier `{name}` is not declared in this scope"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "declare a local binding, parameter, function, type, or import with this name".to_string(),
    );
    diagnostic
}

fn visibility_description(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Private => "private",
        Visibility::Public => "public",
        Visibility::Nocter => "`pub(nocter)`",
    }
}
