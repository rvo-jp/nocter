use super::*;

pub(in crate::analysis::hover) fn call_hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let (span, label) = file.typecheck_facts.call_hover_at_offset(offset)?;
    if let Some(signature) =
        crate::analysis::signature_help::call_signature_at_offset(sources, analysis, file, offset)
        && signature.is_specialized
    {
        let target = call_target(file, span);
        return Some(HoverInfo {
            span,
            label: signature.label,
            documentation: combine_documentation(
                signature.documentation,
                target.and_then(|target| semantic_documentation(sources, analysis, target)),
            ),
        });
    }
    Some(HoverInfo {
        span,
        label: label.to_string(),
        documentation: call_documentation(sources, analysis, file, span),
    })
}

pub(in crate::analysis::hover) fn call_documentation(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    call_span: ByteSpan,
) -> Option<String> {
    let target_span = call_target(file, call_span)?;
    combine_documentation(
        target_documentation(sources, analysis, target_span),
        semantic_documentation(sources, analysis, target_span),
    )
}

pub(in crate::analysis::hover) fn call_target(
    file: &FileAnalysis,
    call_span: ByteSpan,
) -> Option<ByteSpan> {
    file.typecheck_facts
        .function_call_target(call_span)
        .or_else(|| file.typecheck_facts.method_call_target(call_span))
        .or_else(|| file.typecheck_facts.associated_function_target(call_span))
}

pub(in crate::analysis::hover) fn semantic_documentation(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    target_span: ByteSpan,
) -> Option<String> {
    combine_documentation(
        crate::analysis::allocation::allocation_effect_markdown(analysis, target_span),
        crate::analysis::provenance::result_provenance_markdown(sources, analysis, target_span),
    )
}

pub(in crate::analysis::hover) fn combine_documentation(
    first: Option<String>,
    second: Option<String>,
) -> Option<String> {
    match (first, second) {
        (Some(first), Some(second)) => Some(format!("{first}\n\n{second}")),
        (Some(documentation), None) | (None, Some(documentation)) => Some(documentation),
        (None, None) => None,
    }
}

pub(crate) fn target_documentation(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    target_span: ByteSpan,
) -> Option<String> {
    let target_file = analysis.file_by_source(target_span.source)?;
    let target_source = sources.get(target_file.ast.span.source)?;
    let text = target_source.text();
    let symbols = hover_symbols_for_file_analysis(text, target_file);
    let documentation =
        documentation_for_hover_symbols(target_file.ast.span.source, text, &symbols);
    documentation_for_target_span(&documentation, &symbols, target_span)
}

pub(in crate::analysis::hover) fn type_reference_hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let reference = file.typecheck_facts.type_reference_at_offset(offset)?;
    let declaration_span = reference.symbol_declaration_span?;
    let symbol = type_symbol_for_declaration_span(analysis, declaration_span)?;
    let (label, documentation) = resolved_symbol_hover_contents(sources, analysis, symbol)
        .unwrap_or_else(|| {
            (
                symbol_hover_label_for_sources(sources, symbol),
                None::<String>,
            )
        });

    Some(HoverInfo {
        span: reference.span,
        label,
        documentation,
    })
}

pub(in crate::analysis::hover) fn type_reference_hover_for_ast(
    text: &str,
    resolved: &ResolveOutput,
    facts: &TypecheckFacts,
    symbols: &[HoverSymbol],
    documentation: &crate::comments::AttachedDocumentation,
    offset: usize,
) -> Option<HoverInfo> {
    let reference = facts.type_reference_at_offset(offset)?;
    let declaration_span = reference.symbol_declaration_span?;
    let symbol = resolved
        .symbols
        .symbols()
        .find(|candidate| is_type_symbol_at_declaration_span(candidate, declaration_span))?;
    let (label, documentation) =
        single_file_symbol_hover_contents(text, symbols, documentation, symbol);

    Some(HoverInfo {
        span: reference.span,
        label,
        documentation,
    })
}

pub(in crate::analysis::hover) fn type_symbol_for_declaration_span(
    analysis: &CompileUnitAnalysis,
    declaration_span: ByteSpan,
) -> Option<&Symbol> {
    let file = analysis.file_by_source(declaration_span.source)?;
    file.resolved
        .symbols
        .symbols()
        .find(|candidate| is_type_symbol_at_declaration_span(candidate, declaration_span))
}

pub(in crate::analysis::hover) fn is_type_symbol_at_declaration_span(
    symbol: &Symbol,
    declaration_span: ByteSpan,
) -> bool {
    matches!(symbol.kind, SymbolKind::Type(_)) && symbol.declaration_span == declaration_span
}

pub(in crate::analysis::hover) fn documentation_for_hover_symbols(
    source: SourceId,
    text: &str,
    symbols: &[HoverSymbol],
) -> crate::comments::AttachedDocumentation {
    let targets = symbols
        .iter()
        .map(|symbol| DocumentationTarget::new(symbol.attach_start, symbol.name_span.start))
        .collect::<Vec<_>>();
    attach_documentation(source, text, &targets)
}

pub(in crate::analysis::hover) fn documentation_for_target_span(
    documentation: &crate::comments::AttachedDocumentation,
    symbols: &[HoverSymbol],
    target_span: ByteSpan,
) -> Option<String> {
    documentation
        .get(target_span.start)
        .map(str::to_string)
        .or_else(|| {
            symbols
                .iter()
                .find(|symbol| span_contains(symbol.name_span, target_span.start))
                .and_then(|symbol| documentation.get(symbol.name_span.start))
                .map(str::to_string)
        })
}

pub(in crate::analysis::hover) fn resolved_reference_at_offset(
    resolved: &ResolveOutput,
    offset: usize,
) -> Option<(ByteSpan, ResolvedReference)> {
    let mut candidates = Vec::new();
    if let Some((span, symbol)) = resolved.local_symbol_reference_at_offset(offset) {
        candidates.push((span, ResolvedReference::Local(symbol.clone())));
    }
    if let Some((span, symbol)) = resolved.symbol_reference_at_offset(offset) {
        candidates.push((span, ResolvedReference::TopLevel(Box::new(symbol.clone()))));
    }
    candidates.sort_by_key(|(span, _)| (span.len(), span.start));
    candidates.into_iter().next()
}

pub(in crate::analysis::hover) fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}
