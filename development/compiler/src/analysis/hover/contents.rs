use super::*;

pub(in crate::analysis::hover) fn binding_kind_label(
    kind: crate::ast::BindingKind,
) -> &'static str {
    match kind {
        crate::ast::BindingKind::Let => "let",
        crate::ast::BindingKind::Var => "var",
    }
}

pub(in crate::analysis::hover) fn resolved_reference_hover_contents(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    reference: &ResolvedReference,
) -> (String, Option<String>) {
    match reference {
        ResolvedReference::TopLevel(symbol) => {
            resolved_symbol_hover_contents(sources, analysis, symbol).unwrap_or_else(|| {
                (
                    crate::analysis::presentation::symbol_presentation_without_resolution(symbol),
                    None::<String>,
                )
            })
        }
        ResolvedReference::Local(symbol) => {
            resolved_local_symbol_hover_contents(sources, analysis, symbol)
                .unwrap_or_else(|| (local_symbol_hover_label(symbol), None))
        }
    }
}

pub(in crate::analysis::hover) fn resolved_local_symbol_hover_contents(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    symbol: &LocalSymbol,
) -> Option<(String, Option<String>)> {
    let file = analysis.file_by_source(symbol.name_span.source)?;
    let label = crate::analysis::presentation::local_presentation(
        symbol,
        file.typecheck_facts.binding_type_expr(symbol.name_span),
        &file.resolved,
    )
    .render();
    let documentation = target_documentation(sources, analysis, symbol.name_span);
    let region = matches!(symbol.kind, LocalSymbolKind::Region)
        .then(|| crate::analysis::regions::region_markdown(sources, file, symbol.name_span))
        .flatten();

    Some((label, combine_documentation(documentation, region)))
}

pub(in crate::analysis::hover) fn local_symbol_hover_label(symbol: &LocalSymbol) -> String {
    match symbol.kind {
        LocalSymbolKind::Parameter => format!("parameter {}", symbol.name),
        LocalSymbolKind::Binding(kind) => format!("{} {}", binding_kind_label(kind), symbol.name),
        LocalSymbolKind::Region => format!("region {}", symbol.name),
        LocalSymbolKind::PatternPayload => format!("payload {}", symbol.name),
        LocalSymbolKind::CatchError => format!("catch {}", symbol.name),
        LocalSymbolKind::ForRange
        | LocalSymbolKind::CollectionFor
        | LocalSymbolKind::LiteralPackFor => {
            format!("for {}", symbol.name)
        }
        LocalSymbolKind::LiteralCapture => format!("literal pack {}", symbol.name),
        LocalSymbolKind::ClosureCapture(mode) => {
            format!("capture {}{}", mode.source_prefix(), symbol.name)
        }
    }
}

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

pub(in crate::analysis::hover) fn source_fragment(text: &str, span: ByteSpan) -> &str {
    text.get(span.start.min(text.len())..span.end.min(text.len()))
        .unwrap_or_default()
        .trim()
}

pub(in crate::analysis::hover) fn declaration_line_start(text: &str, node_start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut line_start = node_start.min(bytes.len());
    while line_start > 0 && bytes[line_start - 1] != b'\n' {
        line_start -= 1;
    }

    let mut start = line_start;
    while start < node_start && matches!(bytes[start], b' ' | b'\t') {
        start += 1;
    }

    start
}
