use super::*;

pub(super) fn type_symbol_kind_name(kind: TypeSymbolKind) -> &'static str {
    match kind {
        TypeSymbolKind::Alias => "type alias",
        TypeSymbolKind::Struct => "struct",
        TypeSymbolKind::Enum => "enum",
        TypeSymbolKind::Trait => "trait",
    }
}
pub(super) fn binding_keyword(kind: BindingKind) -> &'static str {
    match kind {
        BindingKind::Let => "let",
        BindingKind::Var => "var",
    }
}

pub(super) fn add_declared_return_note(
    sources: &SourceMap,
    diagnostic: &mut Diagnostic,
    context: &ReturnContext,
) {
    if let Ok(span) = sources.span_to_json(context.return_type_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!(
                "{} declares return type `{}`",
                context.subject(),
                context.declared_type.display()
            ),
            span: Some(span),
        });
    }
}
