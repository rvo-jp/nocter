use super::{
    AssociatedTypeBindingSignature, AssociatedTypeSignature, Diagnostic, DiagnosticNote, ImplDecl,
    MethodSignature, SourceMap, Type, TypeExpr, TypeSymbol, type_symbol_kind_name,
};

pub(in crate::typecheck) fn associated_type_missing_diagnostic(
    sources: &SourceMap,
    impl_: &ImplDecl,
    interface_symbol: &TypeSymbol,
    target_symbol: &TypeSymbol,
    required: &AssociatedTypeSignature,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0433",
        format!(
            "interface implementation for `{}` does not bind associated type `{}` from `{}`",
            target_symbol.canonical_name, required.name, interface_symbol.canonical_name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(impl_.target_ty.span())
        .ok()
        .map(Box::new);
    if let Ok(span) = sources.span_to_json(required.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "associated type is required here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "add `type {} = ConcreteType` to this interface implementation",
        required.name
    ));
    diagnostic
}

pub(in crate::typecheck) fn associated_type_extra_diagnostic(
    sources: &SourceMap,
    interface_symbol: &TypeSymbol,
    target_symbol: &TypeSymbol,
    binding: &AssociatedTypeBindingSignature,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0434",
        format!(
            "associated type `{}.{}` is not declared by interface `{}`",
            target_symbol.canonical_name, binding.name, interface_symbol.canonical_name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(binding.name_span).ok().map(Box::new);
    diagnostic.help =
        Some("remove the binding or declare the associated type in the interface".to_string());
    diagnostic
}

pub(in crate::typecheck) fn duplicate_associated_type_binding_diagnostic(
    sources: &SourceMap,
    target_symbol: &TypeSymbol,
    binding: &AssociatedTypeBindingSignature,
    first_span: crate::source::ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0435",
        format!(
            "interface implementation for `{}` binds associated type `{}` more than once",
            target_symbol.canonical_name, binding.name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(binding.name_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first binding is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("keep exactly one binding for each associated type".to_string());
    diagnostic
}

pub(in crate::typecheck) fn interface_impl_contract_not_interface_diagnostic(
    sources: &SourceMap,
    interface_ty: &TypeExpr,
    actual: &Type,
    symbol: Option<&TypeSymbol>,
) -> Diagnostic {
    let actual_label = symbol.map_or_else(
        || actual.display(),
        |symbol| {
            format!(
                "{} `{}`",
                type_symbol_kind_name(symbol.kind),
                symbol.canonical_name
            )
        },
    );
    let mut diagnostic = Diagnostic::error(
        "E0422",
        format!("interface conformance impl must name an interface, got {actual_label}"),
    );
    diagnostic.primary_span = sources.span_to_json(interface_ty.span()).ok().map(Box::new);
    diagnostic.help = Some("write `impl Interface for Type` with an interface name".to_string());
    diagnostic
}

pub(in crate::typecheck) fn interface_impl_target_not_nominal_diagnostic(
    sources: &SourceMap,
    target_ty: &TypeExpr,
    actual: &Type,
    symbol: Option<&TypeSymbol>,
) -> Diagnostic {
    let actual_label = symbol.map_or_else(
        || actual.display(),
        |symbol| {
            format!(
                "{} `{}`",
                type_symbol_kind_name(symbol.kind),
                symbol.canonical_name
            )
        },
    );
    let mut diagnostic = Diagnostic::error(
        "E0423",
        format!("interface conformance target must be a struct or enum, got {actual_label}"),
    );
    diagnostic.primary_span = sources.span_to_json(target_ty.span()).ok().map(Box::new);
    diagnostic.help = Some("implement interfaces for nominal struct or enum types".to_string());
    diagnostic
}

pub(in crate::typecheck) fn duplicate_interface_impl_diagnostic(
    sources: &SourceMap,
    impl_: &ImplDecl,
    interface_symbol: &TypeSymbol,
    target_symbol: &TypeSymbol,
    first_span: crate::source::ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0424",
        format!(
            "`{}` already declares conformance to interface `{}`",
            target_symbol.canonical_name, interface_symbol.canonical_name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(impl_.span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first conformance declaration is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("keep a single `impl Interface for Type` declaration".to_string());
    diagnostic
}

pub(in crate::typecheck) fn interface_method_missing_diagnostic(
    sources: &SourceMap,
    impl_: &ImplDecl,
    interface_symbol: &TypeSymbol,
    target_symbol: &TypeSymbol,
    required: &MethodSignature,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0425",
        format!(
            "interface implementation for `{}` does not define required method `{}` from `{}`",
            target_symbol.canonical_name, required.name, interface_symbol.canonical_name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(impl_.target_ty.span())
        .ok()
        .map(Box::new);
    if let Ok(span) = sources.span_to_json(required.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "interface method is required here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "define `method self.{}(...)` inside this interface implementation block",
        required.name
    ));
    diagnostic
}

pub(in crate::typecheck) fn interface_impl_extra_method_diagnostic(
    sources: &SourceMap,
    interface_symbol: &TypeSymbol,
    target_symbol: &TypeSymbol,
    actual: &MethodSignature,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0425",
        format!(
            "method `{}.{}` is not declared by interface `{}`",
            target_symbol.canonical_name, actual.name, interface_symbol.canonical_name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(actual.name_span).ok().map(Box::new);
    diagnostic.help =
        Some("move unrelated methods to an inherent `impl Type { ... }` block".to_string());
    diagnostic
}

pub(in crate::typecheck) fn duplicate_interface_impl_method_diagnostic(
    sources: &SourceMap,
    interface_symbol: &TypeSymbol,
    target_symbol: &TypeSymbol,
    actual: &MethodSignature,
    first_span: crate::source::ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0425",
        format!(
            "interface implementation for `{}` defines method `{}` more than once",
            target_symbol.canonical_name, actual.name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(actual.name_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!(
                "first implementation of `{}.{}` is here",
                interface_symbol.canonical_name, actual.name
            ),
            span: Some(span),
        });
    }
    diagnostic.help = Some("keep exactly one implementation for each interface method".to_string());
    diagnostic
}

pub(in crate::typecheck) fn interface_method_signature_mismatch_diagnostic(
    sources: &SourceMap,
    interface_symbol: &TypeSymbol,
    target_symbol: &TypeSymbol,
    required: &MethodSignature,
    actual: &MethodSignature,
    expected: String,
    found: String,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0426",
        format!(
            "`{}.{}` does not match the signature required by interface `{}`",
            target_symbol.canonical_name, actual.name, interface_symbol.canonical_name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(actual.name_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(required.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("required signature is `{expected}`"),
            span: Some(span),
        });
    }
    if let Ok(span) = sources.span_to_json(actual.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("implementing signature is `{found}`"),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("make the interface implementation member signature match the contract".to_string());
    diagnostic
}
