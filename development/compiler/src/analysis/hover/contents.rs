use super::*;

pub(in crate::analysis::hover) fn resolved_symbol_hover_contents(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    use_site: &crate::resolve::ResolveOutput,
    symbol: &Symbol,
) -> Option<(String, Option<String>)> {
    let crate::resolve::ResolvedDeclaration::Symbol(declaration) =
        use_site.declaration(symbol.def_id)?
    else {
        return None;
    };
    let label = match &declaration.kind {
        SymbolKind::Function(signature) => {
            crate::analysis::presentation::callable_signature_presentation(
                "func",
                &symbol.name,
                signature,
                use_site,
            )
            .render()
        }
        SymbolKind::Primitive(signature) => {
            crate::analysis::presentation::callable_signature_presentation(
                "primitive",
                &symbol.name,
                signature,
                use_site,
            )
            .render()
        }
        SymbolKind::Type(_) => {
            let mut displayed = declaration.clone();
            displayed.name = symbol.name.clone();
            crate::analysis::presentation::type_declaration_presentation(&displayed, use_site)?
                .render()
        }
        SymbolKind::Imported(_) => return None,
    };

    let construction = match &declaration.kind {
        SymbolKind::Type(type_symbol) => {
            crate::analysis::constructions::construction_surface_markdown(type_symbol, use_site)
        }
        _ => None,
    };
    Some((
        label,
        combine_documentation(
            combine_documentation(
                target_documentation(sources, analysis, declaration.name_span),
                semantic_documentation(sources, analysis, symbol.declaration_span),
            ),
            construction,
        ),
    ))
}
