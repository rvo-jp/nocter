use super::{BindingStmt, Diagnostic, DiagnosticNote, SourceMap, Type, binding_keyword};

pub(in crate::typecheck) fn binding_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &BindingStmt,
    expected: &Type,
    actual: &Type,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0342",
        format!(
            "`{keyword}` binding `{}` is annotated as `{}`, but the initializer has type `{}`",
            statement.name,
            expected.display(),
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    if let Some(annotation) = &statement.ty
        && let Ok(span) = sources.span_to_json(annotation.span())
    {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("binding `{}` is annotated here", statement.name),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "change the initializer or annotate `{}` as `{}`",
        statement.name,
        actual.display()
    ));
    diagnostic
}
