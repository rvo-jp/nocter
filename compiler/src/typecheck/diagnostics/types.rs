use super::{Diagnostic, SourceMap, Type, TypeExpr, type_expr_display_lossy};

pub(in crate::typecheck) fn unsized_value_type_diagnostic(
    sources: &SourceMap,
    ty: &TypeExpr,
    subject: &str,
    unsized_part: &Type,
) -> Diagnostic {
    let display = type_expr_display_lossy(ty);
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
        _ => "put the unsized type behind a borrow or use an owning sized type".to_string(),
    });
    diagnostic
}
