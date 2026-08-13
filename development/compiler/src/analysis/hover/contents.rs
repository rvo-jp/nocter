use super::*;

pub(in crate::analysis::hover) fn resolved_symbol_hover_contents(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    symbol: &Symbol,
) -> Option<(String, Option<String>)> {
    let file = analysis.file_by_source(symbol.declaration_span.source)?;
    let declaration = file
        .resolved
        .symbols
        .symbols()
        .find(|candidate| candidate.declaration_span == symbol.declaration_span)?;
    let label = match &declaration.kind {
        SymbolKind::Function(signature) => {
            crate::analysis::presentation::callable_signature_presentation(
                "func",
                &symbol.name,
                signature,
                &file.resolved,
            )
            .render()
        }
        SymbolKind::Primitive(signature) => {
            crate::analysis::presentation::callable_signature_presentation(
                "primitive",
                &symbol.name,
                signature,
                &file.resolved,
            )
            .render()
        }
        SymbolKind::Type(_) => {
            let mut displayed = declaration.clone();
            displayed.name = symbol.name.clone();
            crate::analysis::presentation::type_declaration_presentation(
                &displayed,
                &file.resolved,
            )?
            .render()
        }
        SymbolKind::Imported(_) => return None,
    };

    let construction = match &symbol.kind {
        SymbolKind::Type(type_symbol) => {
            crate::analysis::constructions::construction_surface_markdown(
                type_symbol,
                &file.resolved,
            )
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
