use super::{Diagnostic, SourceMap, canonical_type_expr};
use crate::ast::DestructDecl;

pub(in crate::typecheck) fn destruct_binding_type_unsupported_diagnostic(
    sources: &SourceMap,
    destruct: &DestructDecl,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0387",
        format!(
            "destruct binding must have type `&+Self`, got `{}`",
            canonical_type_expr(&destruct.binding.ty)
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(destruct.binding.ty.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some("write the declaration as `destruct Type(&+self) { ... }`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn non_uniform_destruct_pattern_diagnostic(
    sources: &SourceMap,
    destruct: &DestructDecl,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0388",
        "destruction must cover every specialization of its nominal type",
    );
    diagnostic.primary_span = sources
        .span_to_json(destruct.keyword_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some(format!(
        "use a uniform declaration such as `destruct {}<T>(&+self) {{ ... }}`",
        canonical_type_expr(&destruct.target_ty)
            .split('<')
            .next()
            .unwrap_or("Type")
    ));
    diagnostic
}

pub(in crate::typecheck) fn invalid_destruct_target_diagnostic(
    sources: &SourceMap,
    destruct: &DestructDecl,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0399",
        format!(
            "destruct target `{}` must be a struct or enum",
            canonical_type_expr(&destruct.target_ty)
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(destruct.target_ty.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(
        "declare destruction on the nominal struct or enum rather than an alias or view"
            .to_string(),
    );
    diagnostic
}
