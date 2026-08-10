use super::{Diagnostic, SourceMap, canonical_type_expr};
use crate::ast::{DropDecl, InstanceDecl};

pub(in crate::typecheck) fn drop_binding_type_unsupported_diagnostic(
    sources: &SourceMap,
    drop_: &DropDecl,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0387",
        format!(
            "drop member binding must have type `&+Self`, got `{}`",
            canonical_type_expr(&drop_.binding.ty)
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(drop_.binding.ty.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some("write the drop member as `drop &+self { ... }`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn conditional_drop_pattern_diagnostic(
    sources: &SourceMap,
    instance: &InstanceDecl,
    drop_: &DropDecl,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0388",
        "drop behavior must cover every specialization of its nominal type",
    );
    diagnostic.primary_span = sources.span_to_json(drop_.name_span).ok().map(Box::new);
    diagnostic.help = Some(format!(
        "move `drop` to an unconstrained pattern such as `instance {}<T>`, and keep specialized behavior in separate method-only instances",
        canonical_type_expr(&instance.target_ty)
            .split('<')
            .next()
            .unwrap_or("Type")
    ));
    diagnostic
}
