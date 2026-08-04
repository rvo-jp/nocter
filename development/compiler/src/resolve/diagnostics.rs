use crate::ast::Visibility;
use crate::diagnostics::{Diagnostic, DiagnosticNote};
use crate::source::{ByteSpan, SourceMap};

pub(super) fn implicit_closure_capture_diagnostic(
    sources: &SourceMap,
    name: &str,
    use_span: ByteSpan,
    declaration_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0452",
        format!("outer binding `{name}` is not captured by this closure"),
    );
    diagnostic.primary_span = sources.span_to_json(use_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(declaration_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "outer binding is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "add `&{name}`, `&+{name}`, or `move {name}` before the closure parameter separator"
    ));
    diagnostic
}

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
    diagnostic.help = Some("choose a distinct name; shadowing is not supported".to_string());
    diagnostic
}

pub(super) fn prelude_name_collision_diagnostic(
    sources: &SourceMap,
    name: &str,
    prelude_span: ByteSpan,
    duplicate_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0400",
        format!("name `{name}` is already visible from the synthetic standard prelude"),
    );
    diagnostic.primary_span = sources.span_to_json(duplicate_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(prelude_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "the synthetic standard prelude introduces this name here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("choose a distinct name; shadowing prelude names is not supported".to_string());
    diagnostic
}

pub(super) fn builtin_name_reuse_diagnostic(
    sources: &SourceMap,
    name: &str,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0401",
        format!("built-in type name `{name}` cannot be reused as a value name"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some("choose a value name that is not a built-in type name".to_string());
    diagnostic
}

pub(super) fn reserved_type_declaration_name_reuse_diagnostic(
    sources: &SourceMap,
    name: &str,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0401",
        format!("reserved type spelling `{name}` cannot be reused as a type declaration"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help =
        Some("choose a type name that is not reserved type-position syntax".to_string());
    diagnostic
}

pub(super) fn reserved_generic_parameter_name_reuse_diagnostic(
    sources: &SourceMap,
    name: &str,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0401",
        format!("reserved type spelling `{name}` cannot be reused as a generic parameter"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "choose a generic parameter name that is not reserved type-position syntax".to_string(),
    );
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
        Some("choose a distinct member name; overloads are not supported".to_string());
    diagnostic
}

pub(super) fn duplicate_interface_method_name_diagnostic(
    sources: &SourceMap,
    interface_name: &str,
    method_name: &str,
    first_span: ByteSpan,
    duplicate_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0428",
        format!("interface `{interface_name}` already has a method named `{method_name}`"),
    );
    diagnostic.primary_span = sources.span_to_json(duplicate_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first interface method with this name is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("choose a distinct method name; overloads are not supported".to_string());
    diagnostic
}

pub(super) fn duplicate_struct_field_name_diagnostic(
    sources: &SourceMap,
    struct_name: &str,
    field_name: &str,
    first_span: ByteSpan,
    duplicate_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0417",
        format!("struct `{struct_name}` already has a field named `{field_name}`"),
    );
    diagnostic.primary_span = sources.span_to_json(duplicate_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first field with this name is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("choose a distinct struct field name".to_string());
    diagnostic
}

pub(super) fn duplicate_enum_variant_name_diagnostic(
    sources: &SourceMap,
    enum_name: &str,
    variant_name: &str,
    first_span: ByteSpan,
    duplicate_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0418",
        format!("enum `{enum_name}` already has a variant named `{variant_name}`"),
    );
    diagnostic.primary_span = sources.span_to_json(duplicate_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first variant with this name is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("choose a distinct enum variant name".to_string());
    diagnostic
}

pub(super) fn duplicate_enum_variant_payload_name_diagnostic(
    sources: &SourceMap,
    enum_name: &str,
    variant_name: &str,
    payload_name: &str,
    first_span: ByteSpan,
    duplicate_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0419",
        format!(
            "enum variant `{enum_name}.{variant_name}` already has a payload named `{payload_name}`"
        ),
    );
    diagnostic.primary_span = sources.span_to_json(duplicate_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first payload with this name is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("choose a distinct enum payload name".to_string());
    diagnostic
}

pub(super) fn duplicate_generic_parameter_name_diagnostic(
    sources: &SourceMap,
    subject: &str,
    parameter_name: &str,
    first_span: ByteSpan,
    duplicate_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0420",
        format!("{subject} already has a generic parameter named `{parameter_name}`"),
    );
    diagnostic.primary_span = sources.span_to_json(duplicate_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first generic parameter with this name is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("choose a distinct generic parameter name".to_string());
    diagnostic
}

pub(super) fn duplicate_parameter_name_diagnostic(
    sources: &SourceMap,
    subject: &str,
    parameter_name: &str,
    first_span: ByteSpan,
    duplicate_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0421",
        format!("{subject} already has a parameter named `{parameter_name}`"),
    );
    diagnostic.primary_span = sources.span_to_json(duplicate_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first parameter with this name is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("choose a distinct parameter name".to_string());
    diagnostic
}

pub(super) fn unqualified_enum_variant_constructor_diagnostic(
    sources: &SourceMap,
    variant_name: &str,
    variant_span: ByteSpan,
    enum_name: &str,
    reference_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0431",
        format!("enum variant `{variant_name}` cannot be used unqualified in v0"),
    );
    diagnostic.primary_span = sources.span_to_json(reference_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(variant_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("variant is declared as `{enum_name}.{variant_name}` here"),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!("write `{enum_name}.{variant_name}`"));
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
