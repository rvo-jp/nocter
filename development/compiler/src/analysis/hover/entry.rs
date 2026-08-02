use super::*;

pub(crate) fn hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let source_file = sources.get(file.ast.span.source)?;
    let text = source_file.text();
    let symbols = hover_symbols_for_file_analysis(text, file);
    let documentation = documentation_for_hover_symbols(file.ast.span.source, text, &symbols);

    if let Some(hover) = module_path_hover_for_ast(sources, analysis, file, offset) {
        return Some(hover);
    }

    if let Some(literal) = crate::analysis::literals::literal_editor_info_at_offset(
        analysis,
        file,
        offset,
        crate::analysis::literals::LiteralCursorRegion::Hover,
    ) {
        return Some(HoverInfo {
            span: literal.focus_span,
            label: literal.label,
            documentation: combine_documentation(
                target_documentation(sources, analysis, literal.declaration_shape_span),
                semantic_documentation(sources, analysis, literal.declaration_span),
            ),
        });
    }

    if let Some(symbol) = symbols
        .iter()
        .find(|symbol| span_contains(symbol.name_span, offset))
    {
        let attached = documentation
            .get(symbol.name_span.start)
            .map(str::to_string);
        let semantic = semantic_documentation(sources, analysis, symbol.name_span);
        let region = crate::analysis::regions::region_markdown(sources, file, symbol.name_span);
        return Some(HoverInfo {
            span: symbol.name_span,
            label: symbol.label.clone(),
            documentation: combine_documentation(combine_documentation(attached, semantic), region),
        });
    }

    if let Some(hover) = call_hover_for_file_analysis(sources, analysis, file, offset) {
        return Some(hover);
    }

    if let Some((span, label)) = file.typecheck_facts.field_hover_at_offset(offset) {
        return Some(HoverInfo {
            span,
            label: label.to_string(),
            documentation: None,
        });
    }

    if let Some((span, label)) = file.typecheck_facts.enum_variant_hover_at_offset(offset) {
        return Some(HoverInfo {
            span,
            label: label.to_string(),
            documentation: file
                .typecheck_facts
                .enum_variant_target(span)
                .and_then(|target| target_documentation(sources, analysis, target)),
        });
    }

    if let Some(hover) = type_reference_hover_for_file_analysis(sources, analysis, file, offset) {
        return Some(hover);
    }

    resolved_reference_at_offset(&file.resolved, offset).map(|(span, reference)| {
        let (label, documentation) =
            resolved_reference_hover_contents(sources, analysis, &reference);
        HoverInfo {
            span,
            label,
            documentation,
        }
    })
}

pub(crate) fn hover_for_text(text: &str, offset: usize) -> Option<HoverInfo> {
    let parsed = parse_single_file_text("hover.nct", text)?;

    hover_for_ast(text, parsed.source, &parsed.ast, offset)
}

pub(crate) fn hover_for_ast(
    text: &str,
    source: SourceId,
    ast: &AstFile,
    offset: usize,
) -> Option<HoverInfo> {
    let resolved = resolve_single_file_ast("hover.nct", text, source, ast);
    let facts = collect_typecheck_facts(ast, &resolved);
    let mut symbols = hover_symbols_for_ast(text, ast);
    apply_typecheck_hover_facts(text, &facts, &mut symbols);
    let documentation = documentation_for_hover_symbols(source, text, &symbols);

    if let Some(symbol) = symbols
        .iter()
        .find(|symbol| span_contains(symbol.name_span, offset))
    {
        return Some(HoverInfo {
            span: symbol.name_span,
            label: symbol.label.clone(),
            documentation: documentation
                .get(symbol.name_span.start)
                .map(str::to_string),
        });
    }

    if let Some((span, label)) = facts.call_hover_at_offset(offset) {
        return Some(HoverInfo {
            span,
            label: label.to_string(),
            documentation: facts
                .function_call_target(span)
                .or_else(|| facts.method_call_target(span))
                .or_else(|| facts.associated_function_target(span))
                .and_then(|target| documentation_for_target_span(&documentation, &symbols, target)),
        });
    }

    if let Some((span, label)) = facts.field_hover_at_offset(offset) {
        return Some(HoverInfo {
            span,
            label: label.to_string(),
            documentation: None,
        });
    }

    if let Some((span, label)) = facts.enum_variant_hover_at_offset(offset) {
        return Some(HoverInfo {
            span,
            label: label.to_string(),
            documentation: facts
                .enum_variant_target(span)
                .and_then(|target| documentation_for_target_span(&documentation, &symbols, target)),
        });
    }

    if let Some(hover) =
        type_reference_hover_for_ast(text, &resolved, &facts, &symbols, &documentation, offset)
    {
        return Some(hover);
    }

    resolved_reference_at_offset(&resolved, offset).map(|(span, reference)| {
        let (label, documentation) = single_file_resolved_reference_hover_contents(
            text,
            &symbols,
            &documentation,
            &reference,
        );
        HoverInfo {
            span,
            label,
            documentation,
        }
    })
}

pub(crate) fn definition_span_for_ast(
    text: &str,
    ast: &AstFile,
    resolved: &ResolveOutput,
    offset: usize,
) -> Option<ByteSpan> {
    let symbols = hover_symbols_for_ast(text, ast);
    if let Some(symbol) = symbols
        .iter()
        .find(|symbol| span_contains(symbol.name_span, offset))
    {
        return Some(symbol.name_span);
    }

    resolved_reference_at_offset(resolved, offset)
        .map(|(_, reference)| reference.declaration_span())
}
