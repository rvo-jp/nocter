use super::{Diagnostic, DiagnosticNote, SourceMap, Type, TypeExpr, canonical_type_expr};
use crate::source::ByteSpan;

pub(in crate::typecheck) fn unsized_value_type_diagnostic(
    sources: &SourceMap,
    ty: &TypeExpr,
    subject: &str,
    unsized_part: &Type,
) -> Diagnostic {
    let display = canonical_type_expr(ty);
    let mut diagnostic = Diagnostic::error(
        "E0380",
        format!(
            "{} has unsized type `{display}`, which cannot be used by value",
            subject
        ),
    );
    diagnostic.primary_span = sources.span_to_json(ty.span()).ok().map(Box::new);
    diagnostic.help = Some(match unsized_part {
        Type::StrData => "use `&str` for a string slice or `String` for owned text".to_string(),
        Type::ArrayData { element } => format!(
            "use `&[{}]` for a readonly slice, `&+[{}]` for a mutable slice, or `Vec<{}>` for owned variable-length storage",
            element.display(),
            element.display(),
            element.display()
        ),
        Type::Named(_) | Type::Generic { .. } => {
            "use a concrete struct or enum that explicitly implements the interface; interfaces are contracts only and have no runtime dispatch representation".to_string()
        }
        _ => "put the unsized type behind a borrow or use an owning sized type".to_string(),
    });
    diagnostic
}

pub(in crate::typecheck) fn generic_type_argument_count_diagnostic(
    sources: &SourceMap,
    name: &str,
    name_span: ByteSpan,
    declaration_span: Option<ByteSpan>,
    expected: usize,
    found: usize,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0427",
        format!(
            "type `{name}` expects {} type {}, got {found}",
            expected,
            if expected == 1 {
                "argument"
            } else {
                "arguments"
            }
        ),
    );
    diagnostic.primary_span = sources.span_to_json(name_span).ok().map(Box::new);
    if let Some(declaration_span) = declaration_span
        && let Ok(span) = sources.span_to_json(declaration_span)
    {
        diagnostic.notes.push(DiagnosticNote {
            message: "type is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some(if expected == 0 {
        "remove the type argument list".to_string()
    } else {
        format!(
            "write `{name}<...>` with exactly {} type {}",
            expected,
            if expected == 1 {
                "argument"
            } else {
                "arguments"
            }
        )
    });
    diagnostic
}

pub(in crate::typecheck) fn unresolved_type_reference_diagnostic(
    sources: &SourceMap,
    name: &str,
    name_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0436",
        format!("type `{name}` is not declared in this scope"),
    );
    diagnostic.primary_span = sources.span_to_json(name_span).ok().map(Box::new);
    diagnostic.help =
        Some("import or define the type, or use a built-in type such as `i32`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn self_type_outside_context_diagnostic(
    sources: &SourceMap,
    name_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0436",
        "`Self` has no meaning outside inherent member or interface method type positions",
    );
    diagnostic.primary_span = sources.span_to_json(name_span).ok().map(Box::new);
    diagnostic.help = Some(
        "use `Self` only inside an inherent `impl`, a qualified associated function declaration, or an interface method signature".to_string(),
    );
    diagnostic
}
