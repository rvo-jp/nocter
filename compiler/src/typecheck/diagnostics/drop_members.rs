use super::{Diagnostic, SourceMap, type_expr_display_lossy};
use crate::ast::DropDecl;

pub(in crate::typecheck) fn drop_binding_type_unsupported_diagnostic(
    sources: &SourceMap,
    drop_: &DropDecl,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0387",
        format!(
            "drop member binding must have type `&+Self`, got `{}`",
            type_expr_display_lossy(&drop_.binding.ty)
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(drop_.binding.ty.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some("write the drop member as `drop name: &+Self { ... }`".to_string());
    diagnostic
}
